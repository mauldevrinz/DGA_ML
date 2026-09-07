#!/usr/bin/env python3
"""Parses Teensy serial lines and dual-writes each aggregated sample to
SQLite and InfluxDB. Mirrors the regex parsing done client-side in
src/components/Acquisition.jsx so both places stay in sync.
"""
import re
import sqlite3
import time
from pathlib import Path

import requests

ROOT = Path(__file__).resolve().parent

MOS_FIELDS = [f"mos{i}" for i in range(16)]
NUMERIC_FIELDS = MOS_FIELDS + [
    "temp0", "temp1", "hum0", "hum1",
    "ina1_v", "ina1_i", "ina1_p",
    "ina2_v", "ina2_i", "ina2_p",
    "setpoint_c",
]

RE_ADC = re.compile(r"ADC(\d+)\s*=\s*(-?\d+)")
RE_TEMP0 = re.compile(r"SHT30 Temp\s*=\s*(-?\d+\.\d+)")
RE_HUM0 = re.compile(r"SHT30 Humi\s*=\s*(\d+\.\d+)")
RE_TEMP1 = re.compile(r"SHT31 Temp\s*=\s*(-?\d+\.\d+)")
RE_HUM1 = re.compile(r"SHT31 Humi\s*=\s*(\d+\.\d+)")
RE_SETPOINT = re.compile(r"^(?:ACK )?SETPOINT\s*=\s*(-?\d+\.\d+)")
RE_PELTIER_MODE = re.compile(r"PELTIER_MODE\s*=\s*(\w+)")
RE_INA = re.compile(
    r"INA226_(\d+)\s+Vbus\s*=\s*(-?\d+\.?\d*)\s*V,\s*Vshunt\s*=\s*(-?\d+\.?\d*)\s*V,"
    r"\s*I\s*=\s*(-?\d+\.?\d*)\s*A,\s*P\s*=\s*(-?\d+\.?\d*)\s*W"
)


def _load_env(path=ROOT / ".env"):
    env = {}
    if path.exists():
        for raw_line in path.read_text().splitlines():
            stripped = raw_line.strip()
            if not stripped or stripped.startswith("#") or "=" not in stripped:
                continue
            key, value = stripped.split("=", 1)
            env[key.strip()] = value.strip()
    return env


class SensorPersistence:
    """Aggregates parsed fields into one sample per second (same throttle
    as the browser) and writes it to SQLite + InfluxDB."""

    def __init__(self, min_interval_sec=1.0):
        env = _load_env()

        self.sqlite_path = str(ROOT / env.get("SQLITE_PATH", "sensor_stream.db"))
        self.influx_url = env.get("INFLUXDB_URL", "http://127.0.0.1:8086")
        self.influx_org = env.get("INFLUXDB_ORG", "dga-enose")
        self.influx_bucket = env.get("INFLUXDB_BUCKET", "sensor_data")
        self.influx_token = env.get("INFLUXDB_TOKEN", "")

        self.min_interval_sec = min_interval_sec
        self._current = self._blank_sample()
        self._last_write = 0.0
        self._influx_warned = False

        self._db = sqlite3.connect(self.sqlite_path, check_same_thread=False)
        self._init_sqlite()

    def _blank_sample(self):
        sample = {field: None for field in NUMERIC_FIELDS}
        sample["peltier_mode"] = None
        return sample

    def _init_sqlite(self):
        columns = ",\n".join(f"{field} REAL" for field in NUMERIC_FIELDS)
        self._db.execute(
            f"""
            CREATE TABLE IF NOT EXISTS sensor_readings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts_unix REAL NOT NULL,
                ts_iso TEXT NOT NULL,
                {columns},
                peltier_mode TEXT
            )
            """
        )
        self._db.commit()

    def process_line(self, line):
        sample = self._current
        updated = False

        if m := RE_ADC.search(line):
            idx = int(m.group(1))
            if 0 <= idx <= 15:
                adc_value = int(m.group(2))
                sample[f"mos{idx}"] = adc_value * (4.096 / 32768.0) * 1000
                updated = True

        if m := RE_TEMP0.search(line):
            sample["temp0"] = float(m.group(1))
            updated = True

        if m := RE_HUM0.search(line):
            sample["hum0"] = float(m.group(1))
            updated = True

        if m := RE_TEMP1.search(line):
            sample["temp1"] = float(m.group(1))
            updated = True

        if m := RE_HUM1.search(line):
            sample["hum1"] = float(m.group(1))
            updated = True

        if m := RE_SETPOINT.match(line):
            sample["setpoint_c"] = float(m.group(1))
            updated = True

        if m := RE_PELTIER_MODE.search(line):
            sample["peltier_mode"] = m.group(1)
            updated = True

        if m := RE_INA.search(line):
            ina_index = int(m.group(1))
            vbus = float(m.group(2))
            current_ma = float(m.group(4)) * 1000  # firmware sends A, we store mA
            power = float(m.group(5))
            if ina_index == 1:
                sample["ina1_v"] = vbus
                sample["ina1_i"] = current_ma
                sample["ina1_p"] = power
                updated = True
            elif ina_index == 2:
                sample["ina2_v"] = vbus
                sample["ina2_i"] = current_ma
                sample["ina2_p"] = power
                updated = True

        if not updated or sample["mos15"] is None:
            return

        now = time.time()
        if now - self._last_write < self.min_interval_sec:
            return
        self._last_write = now

        self._write(sample, now)
        # Carry every field forward so a sensor that doesn't get a fresh
        # reading this cycle keeps its last known value.
        self._current = dict(sample)

    def _write(self, sample, ts_unix):
        self._write_sqlite(sample, ts_unix)
        self._write_influx(sample, ts_unix)

    def _write_sqlite(self, sample, ts_unix):
        try:
            fields = NUMERIC_FIELDS + ["peltier_mode"]
            placeholders = ", ".join("?" for _ in fields)
            columns = ", ".join(fields)
            values = [sample[f] for f in fields]
            self._db.execute(
                f"INSERT INTO sensor_readings (ts_unix, ts_iso, {columns}) "
                f"VALUES (?, ?, {placeholders})",
                [ts_unix, time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(ts_unix))] + values,
            )
            self._db.commit()
        except Exception as e:
            print(f"SQLite write error: {e}")

    def _write_influx(self, sample, ts_unix):
        if not self.influx_token:
            return
        try:
            fields = []
            for f in NUMERIC_FIELDS:
                if sample[f] is not None:
                    fields.append(f"{f}={sample[f]}")
            if sample["peltier_mode"] is not None:
                mode = sample["peltier_mode"].replace('"', "")
                fields.append(f'peltier_mode="{mode}"')
            if not fields:
                return

            line_protocol = f"sensor_readings {','.join(fields)} {int(ts_unix * 1e9)}"
            resp = requests.post(
                f"{self.influx_url}/api/v2/write",
                params={"org": self.influx_org, "bucket": self.influx_bucket, "precision": "ns"},
                headers={"Authorization": f"Token {self.influx_token}"},
                data=line_protocol,
                timeout=1.5,
            )
            if resp.status_code >= 300:
                if not self._influx_warned:
                    print(f"InfluxDB write failed ({resp.status_code}): {resp.text}")
                    self._influx_warned = True
            else:
                self._influx_warned = False
        except Exception as e:
            if not self._influx_warned:
                print(f"InfluxDB write error: {e}")
                self._influx_warned = True
