import re

with open("src/infrastructure/serial/teensy.rs", "r") as f:
    content = f.read()

# Add tx_cmd to TeensySerial struct
content = content.replace(
    "readings_buffer: Arc<Mutex<Vec<SensorReading>>>,",
    "readings_buffer: Arc<Mutex<Vec<SensorReading>>>,\n    tx_cmd: Arc<Mutex<Option<std::sync::mpsc::Sender<Vec<u8>>>>>,",
)

# Initialize tx_cmd in new()
content = content.replace(
    "readings_buffer: Arc::new(Mutex::new(Vec::new())),",
    "readings_buffer: Arc::new(Mutex::new(Vec::new())),\n            tx_cmd: Arc::new(Mutex::new(None)),",
)

# Connect: Create channel, pass rx to loop, set tx_cmd
old_connect = """        // Spawn background reader thread
        let port_name = port_name.to_string();
        let baud = self.baud_rate;
        let is_running = Arc::clone(&self.is_running);
        let latest = Arc::clone(&self.latest_reading);
        let buffer = Arc::clone(&self.readings_buffer);

        std::thread::spawn(move || {
            if let Err(e) = read_serial_loop(&port_name, baud, &is_running, &latest, &buffer) {"""

new_connect = """        // Setup command channel
        let (tx, rx) = std::sync::mpsc::channel();
        *self.tx_cmd.lock() = Some(tx);

        // Spawn background reader thread
        let port_name = port_name.to_string();
        let baud = self.baud_rate;
        let is_running = Arc::clone(&self.is_running);
        let latest = Arc::clone(&self.latest_reading);
        let buffer = Arc::clone(&self.readings_buffer);

        std::thread::spawn(move || {
            if let Err(e) = read_serial_loop(&port_name, baud, &is_running, &latest, &buffer, rx) {"""
content = content.replace(old_connect, new_connect)


# Add write method to TeensySerial
write_method = """    /// Drain buffered readings (moves them out)"""
new_write = """    /// Write data to serial port
    pub fn write(&self, data: &[u8]) -> Result<()> {
        if let Some(tx) = &*self.tx_cmd.lock() {
            tx.send(data.to_vec()).context("Failed to send command to serial thread")?;
        }
        Ok(())
    }

    /// Drain buffered readings (moves them out)"""
content = content.replace(write_method, new_write)


# Modify read_serial_loop signature
old_loop_sig = """    latest: &Mutex<Option<SensorReading>>,
    buffer: &Mutex<Vec<SensorReading>>,
) -> Result<()> {"""

new_loop_sig = """    latest: &Mutex<Option<SensorReading>>,
    buffer: &Mutex<Vec<SensorReading>>,
    rx_cmd: std::sync::mpsc::Receiver<Vec<u8>>,
) -> Result<()> {"""
content = content.replace(old_loop_sig, new_loop_sig)

# Modify read_serial_loop timeout and add write logic
old_timeout = """.timeout(Duration::from_millis(500))"""
new_timeout = """.timeout(Duration::from_millis(100))"""
content = content.replace(old_timeout, new_timeout)

old_loop_body = """    while is_running.load(Ordering::Relaxed) {
        match port.read(&mut byte_buf) {"""

new_loop_body = """    while is_running.load(Ordering::Relaxed) {
        // Check for pending commands
        while let Ok(cmd) = rx_cmd.try_recv() {
            if let Err(e) = port.write_all(&cmd) {
                warn!("Failed to write to serial: {}", e);
            }
        }

        match port.read(&mut byte_buf) {"""
content = content.replace(old_loop_body, new_loop_body)

with open("src/infrastructure/serial/teensy.rs", "w") as f:
    f.write(content)
