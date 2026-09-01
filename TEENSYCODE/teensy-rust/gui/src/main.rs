use eframe::egui;
use egui_plot::{Line, Plot, PlotBounds, PlotPoints};
use serialport::SerialPort;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Batas jumlah sample yang disimpan agar memory tidak terus membesar
// pada sesi monitoring yang sangat lama.
const MAX_POINTS: usize = 10_000;

struct GuiApp {
    ports: Vec<String>,
    selected_port: String,
    serial: Option<Box<dyn SerialPort>>,
    adc_val: Arc<Mutex<String>>,
    mv_val: Arc<Mutex<String>>,
    // (waktu_detik, nilai_mV) — dipakai untuk grafik trend
    history: Arc<Mutex<VecDeque<[f64; 2]>>>,
    start_time: Instant,
    error_msg: String,
}

impl Default for GuiApp {
    fn default() -> Self {
        let ports = serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.port_name)
            .collect::<Vec<_>>();
        let selected_port = ports.first().cloned().unwrap_or_default();
        Self {
            ports,
            selected_port,
            serial: None,
            adc_val: Arc::new(Mutex::new("0".to_string())),
            mv_val: Arc::new(Mutex::new("0.0".to_string())),
            history: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_POINTS))),
            start_time: Instant::now(),
            error_msg: String::new(),
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint every frame for real-time updates
        ctx.request_repaint();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Select Port:");
                egui::ComboBox::from_id_source("port_combo")
                    .selected_text(&self.selected_port)
                    .show_ui(ui, |ui| {
                        for port in &self.ports {
                            ui.selectable_value(&mut self.selected_port, port.clone(), port);
                        }
                    });

                if ui.button("Refresh").clicked() {
                    self.ports = serialport::available_ports()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|p| p.port_name)
                        .collect();
                }

                if self.serial.is_none() {
                    if ui.button("Connect").clicked() {
                        match serialport::new(&self.selected_port, 115200)
                            .timeout(Duration::from_millis(100))
                            .open()
                        {
                            Ok(port) => {
                                self.serial = Some(port.try_clone().unwrap());
                                self.error_msg.clear();

                                // Reset titik waktu nol dan histori grafik setiap kali konek ulang
                                self.start_time = Instant::now();
                                self.history.lock().unwrap().clear();

                                let mut reader = BufReader::new(port);
                                let adc_val = Arc::clone(&self.adc_val);
                                let mv_val = Arc::clone(&self.mv_val);
                                let history = Arc::clone(&self.history);
                                let start_time = self.start_time;

                                thread::spawn(move || {
                                    let mut line = String::new();
                                    loop {
                                        line.clear();
                                        if let Ok(bytes) = reader.read_line(&mut line) {
                                            if bytes > 0 {
                                                // Expected format: "... MQ135:1234,mV:994.3"
                                                if let Some(mq_idx) = line.find("MQ135:") {
                                                    let rest = &line[mq_idx + 6..];
                                                    if let Some(comma_idx) = rest.find(',') {
                                                        let adc = &rest[..comma_idx];
                                                        if let Ok(mut val) = adc_val.lock() {
                                                            *val = adc.to_string();
                                                        }
                                                    }
                                                }
                                                if let Some(mv_idx) = line.find("mV:") {
                                                    let rest = &line[mv_idx + 3..];
                                                    let end_idx = rest
                                                        .find('\r')
                                                        .unwrap_or(rest.find('\n').unwrap_or(rest.len()));
                                                    let mv_str = rest[..end_idx].trim();
                                                    if let Ok(mut val) = mv_val.lock() {
                                                        *val = mv_str.to_string();
                                                    }

                                                    // Catat titik data baru untuk grafik trend
                                                    if let Ok(mv_num) = mv_str.parse::<f64>() {
                                                        let t = start_time.elapsed().as_secs_f64();
                                                        if let Ok(mut hist) = history.lock() {
                                                            hist.push_back([t, mv_num]);
                                                            if hist.len() > MAX_POINTS {
                                                                hist.pop_front();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // Handle disconnection or error if needed
                                            // break;
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                self.error_msg = format!("Error connecting: {}", e);
                            }
                        }
                    }
                } else {
                    if ui.button("Disconnect").clicked() {
                        self.serial = None;
                    }
                    if ui.button("Clear Graph").clicked() {
                        self.history.lock().unwrap().clear();
                        self.start_time = Instant::now();
                    }
                }
            });
            if !self.error_msg.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.error_msg);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.heading("Teensy 4.1 MQ135 Sensor");
                ui.add_space(10.0);

                let adc = self.adc_val.lock().unwrap().clone();
                let mv = self.mv_val.lock().unwrap().clone();

                ui.label(
                    egui::RichText::new(format!("ADC Value: {}", adc))
                        .size(22.0)
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("Voltage: {} mV", mv))
                        .size(30.0)
                        .color(egui::Color32::LIGHT_GREEN),
                );
                ui.add_space(12.0);
            });

            // Ambil snapshot histori untuk digambar (lepas lock secepatnya)
            let points: Vec<[f64; 2]> = {
                let hist = self.history.lock().unwrap();
                hist.iter().copied().collect()
            };
            let latest_t = points.last().map(|p| p[0]).unwrap_or(0.0);

            Plot::new("mv_trend_plot")
                .height(480.0)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .x_axis_label("Waktu (s)")
                .y_axis_label("Tegangan (mV)")
                .show(ui, |plot_ui| {
                    // Sumbu Y dikunci 0-3300 mV, sumbu X mengikuti waktu berjalan
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                        [0.0, 0.0],
                        [latest_t.max(10.0), 3300.0],
                    ));
                    let line = Line::new(PlotPoints::from(points)).name("mV");
                    plot_ui.line(line);
                });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1440.0, 950.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Teensy MQ135 Monitor",
        options,
        Box::new(|_cc| Box::new(GuiApp::default())),
    )
}