use rusqlite::{params, Connection};
use regex::Regex;
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::blocking::Client;
use chrono::prelude::*;
use lazy_static::lazy_static;
use std::path::Path;

lazy_static! {
    static ref RE_ADC: Regex = Regex::new(r"ADC(\d+)\s*=\s*(-?\d+)").unwrap();
    static ref RE_TEMP0: Regex = Regex::new(r"SHT30 Temp\s*=\s*(-?\d+\.\d+)").unwrap();
    static ref RE_HUM0: Regex = Regex::new(r"SHT30 Humi\s*=\s*(\d+\.\d+)").unwrap();
    static ref RE_TEMP1: Regex = Regex::new(r"SHT31 Temp\s*=\s*(-?\d+\.\d+)").unwrap();
    static ref RE_HUM1: Regex = Regex::new(r"SHT31 Humi\s*=\s*(\d+\.\d+)").unwrap();
    static ref RE_SETPOINT: Regex = Regex::new(r"^(?:ACK )?SETPOINT\s*=\s*(-?\d+\.\d+)").unwrap();
    static ref RE_PELTIER_MODE: Regex = Regex::new(r"PELTIER_MODE\s*=\s*(\w+)").unwrap();
    static ref RE_INA: Regex = Regex::new(r"INA226_(\d+)\s+Vbus\s*=\s*(-?\d+\.?\d*)\s*V,\s*Vshunt\s*=\s*(-?\d+\.?\d*)\s*V,\s*I\s*=\s*(-?\d+\.?\d*)\s*A,\s*P\s*=\s*(-?\d+\.?\d*)\s*W").unwrap();
}

const ADC_SENSOR_NAMES: [&str; 16] = [
    "tgs2600", "tgs2611", "tgs2610", "tgs822", "tgs813",
    "mq2", "mq6", "mq8", "mq4", "mq3",
    "mq135", "mq9", "mq7", "mq5",
    "ir12em_act", "ir12em_ref",
];

const NUMERIC_FIELDS: [&str; 30] = [
    "tgs2600", "tgs2611", "tgs2610", "tgs822", "tgs813",
    "mq2", "mq6", "mq8", "mq4", "mq3",
    "mq135", "mq9", "mq7", "mq5",
    "ir12em_act", "ir12em_ref",
    "temp_chamber", "hum_chamber",
    "temp_sample", "hum_sample",
    "ina_actuator_v", "ina_actuator_i", "ina_actuator_p",
    "ina_sensor_mcu_v", "ina_sensor_mcu_i", "ina_sensor_mcu_p",
    "kria_v", "kria_i", "kria_p",
    "setpoint_c",
];

#[derive(Default, Clone, Debug)]
struct Sample {
    values: [Option<f64>; 30],
    peltier_mode: Option<String>,
}

impl Sample {
    fn set(&mut self, name: &str, val: f64) {
        if let Some(pos) = NUMERIC_FIELDS.iter().position(|&x| x == name) {
            self.values[pos] = Some(val);
        }
    }
    
    fn get(&self, name: &str) -> Option<f64> {
        if let Some(pos) = NUMERIC_FIELDS.iter().position(|&x| x == name) {
            self.values[pos]
        } else {
            None
        }
    }
}

struct SensorPersistence {
    db: Connection,
    influx_url: String,
    influx_org: String,
    influx_bucket: String,
    influx_token: String,
    min_interval_sec: f64,
    current: Sample,
    last_write: f64,
    http_client: Client,
    influx_warned: bool,
}

impl SensorPersistence {
    fn new(min_interval_sec: f64) -> Self {
        // Load env via dotenvy from ../.env (since we'll run from DGA_ML)
        let _ = dotenvy::from_filename(".env");
        
        let sqlite_path = env::var("SQLITE_PATH").unwrap_or_else(|_| "sensor_stream.db".to_string());
        let influx_url = env::var("INFLUXDB_URL").unwrap_or_else(|_| "http://127.0.0.1:8086".to_string());
        let influx_org = env::var("INFLUXDB_ORG").unwrap_or_else(|_| "dga-enose".to_string());
        let influx_bucket = env::var("INFLUXDB_BUCKET").unwrap_or_else(|_| "sensor_data".to_string());
        let influx_token = env::var("INFLUXDB_TOKEN").unwrap_or_default();
        
        let db = Connection::open(&sqlite_path).expect("Failed to open SQLite db");
        
        let mut sp = Self {
            db,
            influx_url,
            influx_org,
            influx_bucket,
            influx_token,
            min_interval_sec,
            current: Sample::default(),
            last_write: 0.0,
            http_client: Client::builder().timeout(std::time::Duration::from_secs(1)).build().unwrap(),
            influx_warned: false,
        };
        
        sp.init_sqlite();
        sp
    }

    fn init_sqlite(&mut self) {
        let cols = NUMERIC_FIELDS.iter().map(|f| format!("{} REAL", f)).collect::<Vec<_>>().join(",\n");
        let create_sql = format!("
            CREATE TABLE IF NOT EXISTS sensor_readings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts_unix REAL NOT NULL,
                ts_iso TEXT NOT NULL,
                {},
                peltier_mode TEXT
            )
        ", cols);
        self.db.execute(&create_sql, []).expect("Failed to create table");
        
        // Add kria columns just in case
        for col in ["kria_v", "kria_i", "kria_p"] {
            let _ = self.db.execute(&format!("ALTER TABLE sensor_readings ADD COLUMN {} REAL", col), []);
        }
    }

    fn process_line(&mut self, line: &str) {
        let mut updated = false;
        
        if let Some(caps) = RE_ADC.captures(line) {
            if let (Ok(idx), Ok(val)) = (caps[1].parse::<usize>(), caps[2].parse::<i32>()) {
                if idx < ADC_SENSOR_NAMES.len() {
                    let v = (val as f64) * (4.096 / 32768.0) * 1000.0;
                    self.current.set(ADC_SENSOR_NAMES[idx], v);
                    updated = true;
                }
            }
        }
        
        if let Some(caps) = RE_TEMP0.captures(line) {
            if let Ok(v) = caps[1].parse::<f64>() { self.current.set("temp_chamber", v); updated = true; }
        }
        if let Some(caps) = RE_HUM0.captures(line) {
            if let Ok(v) = caps[1].parse::<f64>() { self.current.set("hum_chamber", v); updated = true; }
        }
        if let Some(caps) = RE_TEMP1.captures(line) {
            if let Ok(v) = caps[1].parse::<f64>() { self.current.set("temp_sample", v); updated = true; }
        }
        if let Some(caps) = RE_HUM1.captures(line) {
            if let Ok(v) = caps[1].parse::<f64>() { self.current.set("hum_sample", v); updated = true; }
        }
        if let Some(caps) = RE_SETPOINT.captures(line) {
            if let Ok(v) = caps[1].parse::<f64>() { self.current.set("setpoint_c", v); updated = true; }
        }
        if let Some(caps) = RE_PELTIER_MODE.captures(line) {
            self.current.peltier_mode = Some(caps[1].to_string());
            updated = true;
        }
        if let Some(caps) = RE_INA.captures(line) {
            if let (Ok(idx), Ok(vbus), Ok(curr), Ok(pwr)) = (caps[1].parse::<i32>(), caps[2].parse::<f64>(), caps[4].parse::<f64>(), caps[5].parse::<f64>()) {
                if idx == 1 {
                    self.current.set("ina_actuator_v", vbus);
                    self.current.set("ina_actuator_i", curr * 1000.0);
                    self.current.set("ina_actuator_p", pwr);
                    updated = true;
                } else if idx == 2 {
                    self.current.set("ina_sensor_mcu_v", vbus);
                    self.current.set("ina_sensor_mcu_i", curr * 1000.0);
                    self.current.set("ina_sensor_mcu_p", pwr);
                    updated = true;
                }
            }
        }
        
        if !updated || self.current.get("ir12em_ref").is_none() {
            return;
        }
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
        if now - self.last_write < self.min_interval_sec {
            return;
        }
        self.last_write = now;
        
        self.read_kria_power();
        let sample_to_write = self.current.clone();
        self.write_sqlite(&sample_to_write, now);
        self.write_influx(&sample_to_write, now);
    }
    
    fn read_kria_power(&mut self) {
        let hwmon_dir = Path::new("/sys/class/hwmon");
        let mut base_path = None;
        
        if let Ok(entries) = fs::read_dir(hwmon_dir) {
            for entry in entries.flatten() {
                let name_path = entry.path().join("name");
                if let Ok(name_str) = fs::read_to_string(&name_path) {
                    if name_str.contains("ina260") {
                        base_path = Some(entry.path());
                        break;
                    }
                }
            }
        }
        
        if let Some(bp) = base_path {
            if let Ok(v_str) = fs::read_to_string(bp.join("in1_input")) {
                if let Ok(v) = v_str.trim().parse::<f64>() { self.current.set("kria_v", v / 1000.0); }
            }
            if let Ok(i_str) = fs::read_to_string(bp.join("curr1_input")) {
                if let Ok(i) = i_str.trim().parse::<f64>() { self.current.set("kria_i", i); }
            }
            if let Ok(p_str) = fs::read_to_string(bp.join("power1_input")) {
                if let Ok(p) = p_str.trim().parse::<f64>() { self.current.set("kria_p", p / 1_000_000.0); }
            }
        }
    }
    
    fn write_sqlite(&mut self, sample: &Sample, ts_unix: f64) {
        let ts_iso = Utc.timestamp_nanos((ts_unix * 1e9) as i64).format("%Y-%m-%dT%H:%M:%S").to_string();
        let cols = NUMERIC_FIELDS.join(", ");
        let placeholders = vec!["?"; NUMERIC_FIELDS.len() + 3].join(", "); // ts_unix, ts_iso, [fields...], peltier
        
        let sql = format!("INSERT INTO sensor_readings (ts_unix, ts_iso, {}, peltier_mode) VALUES ({})", cols, placeholders);
        
        // Build params dynamically
        // Rusqlite expects a sequence of &dyn ToSql. 
        // We will build a Statement and pass a slice of &dyn ToSql.
        let mut stmt = match self.db.prepare(&sql) {
            Ok(s) => s,
            Err(e) => { eprintln!("SQLite prepare error: {}", e); return; }
        };
        
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        params.push(&ts_unix);
        params.push(&ts_iso);
        for i in 0..NUMERIC_FIELDS.len() {
            params.push(&sample.values[i]);
        }
        params.push(&sample.peltier_mode);
        
        if let Err(e) = stmt.execute(&*params) {
            eprintln!("SQLite write error: {}", e);
        }
    }
    
    fn write_influx(&mut self, sample: &Sample, ts_unix: f64) {
        if self.influx_token.is_empty() { return; }
        
        let mut line = String::from("sensor_readings ");
        let mut first = true;
        
        for (i, &field) in NUMERIC_FIELDS.iter().enumerate() {
            if let Some(v) = sample.values[i] {
                if !first { line.push(','); }
                line.push_str(&format!("{}={}", field, v));
                first = false;
            }
        }
        
        if let Some(ref mode) = sample.peltier_mode {
            let clean_mode = mode.replace('"', "");
            if !first { line.push(','); }
            line.push_str(&format!("peltier_mode=\"{}\"", clean_mode));
        }
        
        if first { return; } // nothing to write
        
        line.push_str(&format!(" {}", (ts_unix * 1e9) as u64));
        
        let url = format!("{}/api/v2/write?org={}&bucket={}&precision=ns", self.influx_url, self.influx_org, self.influx_bucket);
        
        let res = self.http_client.post(&url)
            .header("Authorization", format!("Token {}", self.influx_token))
            .body(line)
            .send();
            
        match res {
            Ok(r) if !r.status().is_success() => {
                if !self.influx_warned {
                    eprintln!("InfluxDB write failed ({}): {}", r.status(), r.text().unwrap_or_default());
                    self.influx_warned = true;
                }
            }
            Ok(_) => { self.influx_warned = false; }
            Err(e) => {
                if !self.influx_warned {
                    eprintln!("InfluxDB write error: {}", e);
                    self.influx_warned = true;
                }
            }
        }
    }
}

fn main() {
    let mut persister = SensorPersistence::new(1.0);
    let stdin = io::stdin();
    
    for line in stdin.lock().lines() {
        if let Ok(l) = line {
            persister.process_line(&l);
        }
    }
}
