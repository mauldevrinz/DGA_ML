#![allow(dead_code)]
use anyhow::{Result, Context};
use crate::domain::models::{SensorReading, NUM_SENSORS};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use parking_lot::Mutex;
use tracing::{info, warn, error};

/// Teensy 4.1 USB Vendor/Product IDs
const TEENSY_VID: u16 = 0x16C0;
const TEENSY_PID: u16 = 0x0483;

/// Serial port manager for Teensy 4.1 communication
pub struct TeensySerial {
    port_name: Arc<Mutex<Option<String>>>,
    baud_rate: u32,
    is_running: Arc<AtomicBool>,
    latest_reading: Arc<Mutex<Option<SensorReading>>>,
    readings_buffer: Arc<Mutex<Vec<SensorReading>>>,
}

impl TeensySerial {
    pub fn new(baud_rate: u32) -> Self {
        Self {
            port_name: Arc::new(Mutex::new(None)),
            baud_rate,
            is_running: Arc::new(AtomicBool::new(false)),
            latest_reading: Arc::new(Mutex::new(None)),
            readings_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// List all available serial ports
    pub fn list_ports() -> Result<Vec<PortInfo>> {
        let ports = serialport::available_ports()
            .context("Failed to enumerate serial ports")?;
        Ok(ports.iter().map(|p| {
            let is_teensy = match &p.port_type {
                serialport::SerialPortType::UsbPort(usb) => {
                    usb.vid == TEENSY_VID && usb.pid == TEENSY_PID
                }
                _ => false,
            };
            let description = match &p.port_type {
                serialport::SerialPortType::UsbPort(usb) => {
                    format!("USB {} (VID:{:04X} PID:{:04X})",
                        usb.product.as_deref().unwrap_or("Unknown"),
                        usb.vid, usb.pid)
                }
                _ => "Serial Port".to_string(),
            };
            PortInfo {
                name: p.port_name.clone(),
                description,
                is_teensy,
            }
        }).collect())
    }

    /// Auto-detect Teensy 4.1 port
    pub fn auto_detect() -> Option<String> {
        Self::list_ports().ok()?.into_iter()
            .find(|p| p.is_teensy)
            .map(|p| p.name)
    }

    /// Connect to serial port and start reading in background
    pub fn connect(&self, port_name: &str) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            self.disconnect();
        }

        info!("Connecting to serial port: {} @ {} baud", port_name, self.baud_rate);

        // Test that port can be opened
        let _test = serialport::new(port_name, self.baud_rate)
            .timeout(Duration::from_millis(1000))
            .open()
            .context(format!("Failed to open port {}", port_name))?;
        drop(_test);

        *self.port_name.lock() = Some(port_name.to_string());
        self.is_running.store(true, Ordering::Relaxed);

        // Spawn background reader thread
        let port_name = port_name.to_string();
        let baud = self.baud_rate;
        let is_running = Arc::clone(&self.is_running);
        let latest = Arc::clone(&self.latest_reading);
        let buffer = Arc::clone(&self.readings_buffer);

        std::thread::spawn(move || {
            if let Err(e) = read_serial_loop(&port_name, baud, &is_running, &latest, &buffer) {
                error!("Serial reader error: {}", e);
                is_running.store(false, Ordering::Relaxed);
            }
        });

        info!("Serial connection established");
        Ok(())
    }

    /// Disconnect from serial port
    pub fn disconnect(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        *self.port_name.lock() = None;
        info!("Serial disconnected");
    }

    /// Check if currently connected and reading
    pub fn is_connected(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Get the latest sensor reading
    pub fn get_latest(&self) -> Option<SensorReading> {
        self.latest_reading.lock().clone()
    }

    /// Drain buffered readings (moves them out)
    pub fn drain_buffer(&self) -> Vec<SensorReading> {
        let mut buf = self.readings_buffer.lock();
        std::mem::take(&mut *buf)
    }

    /// Get current port name
    pub fn port_name(&self) -> Option<String> {
        self.port_name.lock().clone()
    }
}

/// Background serial reading loop
fn read_serial_loop(
    port_name: &str,
    baud_rate: u32,
    is_running: &AtomicBool,
    latest: &Mutex<Option<SensorReading>>,
    buffer: &Mutex<Vec<SensorReading>>,
) -> Result<()> {
    let mut port = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(500))
        .open()
        .context("Failed to open serial port")?;

    let mut line_buf = String::new();
    let mut byte_buf = [0u8; 1024];

    while is_running.load(Ordering::Relaxed) {
        match port.read(&mut byte_buf) {
            Ok(n) => {
                let text = String::from_utf8_lossy(&byte_buf[..n]);
                line_buf.push_str(&text);

                // Process complete lines
                while let Some(newline_pos) = line_buf.find('\n') {
                    let line = line_buf[..newline_pos].trim().to_string();
                    line_buf = line_buf[newline_pos + 1..].to_string();

                    if let Some(reading) = parse_csv_line(&line) {
                        *latest.lock() = Some(reading.clone());
                        buffer.lock().push(reading);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                continue;
            }
            Err(e) => {
                warn!("Serial read error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    Ok(())
}

/// Parse a CSV line of 17 comma-separated float values
fn parse_csv_line(line: &str) -> Option<SensorReading> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < NUM_SENSORS {
        return None;
    }

    let mut values = [0.0f64; NUM_SENSORS];
    for (i, part) in parts.iter().take(NUM_SENSORS).enumerate() {
        values[i] = part.trim().parse().ok()?;
    }

    let timestamp = chrono::Utc::now().timestamp_millis();
    Some(SensorReading::new(timestamp, values))
}

/// Information about an available serial port
#[derive(Debug, Clone)]
pub struct PortInfo {
    pub name: String,
    pub description: String,
    pub is_teensy: bool,
}
