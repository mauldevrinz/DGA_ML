//! MLP LOOO Coffee Grade Predictor GUI
//! Prediction interface using pre-trained MLP model (mlp_model.json)

use coffee_classifier::ml::{CoffeeMLP, extract_features_via_python};
use eframe::egui;
use egui::{Color32, RichText};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
enum PredictionState {
    Idle,
    Loading(Instant),
    Success(PredictionResult),
    Error(String),
}

#[derive(Debug, Clone)]
struct PredictionResult {
    predicted_class: String,
    high_grade_prob: f32,
    low_grade_prob: f32,
    confidence: f32,
    prediction_time: Duration,
}

// ═══════════════════════════════════════════════════════
// MAIN GUI APPLICATION
// ═══════════════════════════════════════════════════════

struct MLPPredictorApp {
    file_path: String,
    state: PredictionState,
    model: Option<CoffeeMLP>,
    rx: Option<Receiver<Result<PredictionResult, String>>>,
    model_status: String,
}

impl Default for MLPPredictorApp {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            state: PredictionState::Idle,
            model: None,
            rx: None,
            model_status: String::new(),
        }
    }
}

impl MLPPredictorApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        setup_fonts(&cc.egui_ctx);

        let mut app = Self::default();
        let mut status_parts = Vec::new();


        if let Ok(model) = CoffeeMLP::load("models/loo_mlp_full_model.json") {
            println!("✅ MLP model loaded");
            status_parts.push("✅ MLP model");
            app.model = Some(model);
        } else {
            println!("⚠️ Failed to load MLP model");
            status_parts.push("❌ MLP model missing");
        }

        app.model_status = status_parts.join("  |  ");
        app
    }

    fn predict(&mut self) {
        if self.file_path.is_empty() {
            self.state = PredictionState::Error("Please select a CSV file first!".to_string());
            return;
        }

        let path = PathBuf::from(&self.file_path);
        if !path.exists() {
            self.state = PredictionState::Error(format!("Folder/file tidak ada: {}", self.file_path));
            return;
        }

        if self.model.is_none() {
            self.state =
                PredictionState::Error("Model or normalization stats not loaded!".to_string());
            return;
        }

        let start_time = Instant::now();
        self.state = PredictionState::Loading(start_time);

        let file_path = self.file_path.clone();
        let model = self.model.clone().unwrap();

        let (tx, rx) = channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let result = run_prediction(&file_path, model, start_time);
            let _ = tx.send(result);
        });
    }
}

impl eframe::App for MLPPredictorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(pred) => self.state = PredictionState::Success(pred),
                    Err(e) => self.state = PredictionState::Error(e),
                }
                self.rx = None;
            }
        }

        // Header panel
        egui::TopBottomPanel::top("mlp_predict_header")
            .exact_height(70.0)
            .frame(egui::Frame::none().fill(Color32::from_rgb(0, 130, 110)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("☕  MLP LOOO Coffee Grade Predictor")
                            .size(22.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Predict coffee quality: High Grade or Low Grade")
                            .size(12.0)
                            .color(Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );
                });
            });

        // Status bar
        egui::TopBottomPanel::top("mlp_predict_status")
            .exact_height(28.0)
            .frame(egui::Frame::none().fill(Color32::from_rgb(240, 250, 248)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(&self.model_status)
                            .size(11.0)
                            .color(Color32::DARK_GRAY),
                    );
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(20.0);

            // ── File Picker ─────────────────────────────────────
            egui::Frame::none()
                .fill(Color32::from_rgb(245, 252, 250))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(16.0))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(170, 220, 210)))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("📂  Select Folder (20 files)")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(0, 130, 110)),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let te = egui::TextEdit::singleline(&mut self.file_path)
                            .desired_width(ui.available_width() - 120.0)
                            .hint_text("Select sensor CSV file...");
                        ui.add(te);
                        ui.add_space(8.0);
                        if ui
                            .add(
                                egui::Button::new(RichText::new("📁 Browse").size(13.0))
                                    .fill(Color32::from_rgb(0, 130, 110))
                                    .min_size(egui::vec2(90.0, 30.0)),
                            )
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .pick_folder()
                            {
                                self.file_path = path.display().to_string();
                                self.state = PredictionState::Idle;
                            }
                        }
                    });
                });

            ui.add_space(16.0);

            // ── Predict Button ───────────────────────────────────
            ui.vertical_centered(|ui| {
                let btn_size = egui::vec2(200.0, 46.0);
                let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                let is_loading = matches!(self.state, PredictionState::Loading(_));
                let is_hover = response.hovered();

                let bg = if is_loading {
                    Color32::from_rgb(100, 100, 100)
                } else if is_hover {
                    Color32::from_rgb(0, 110, 90)
                } else {
                    Color32::from_rgb(0, 130, 110)
                };

                ui.painter().rect_filled(rect, 10.0, bg);

                let label = if is_loading {
                    "⏳  Predicting...".to_string()
                } else {
                    "🔍  Predict".to_string()
                };

                let galley = ui.painter().layout_no_wrap(
                    label,
                    egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                    Color32::WHITE,
                );
                let text_pos = rect.center() - galley.size() / 2.0;
                ui.painter().galley(text_pos, galley, Color32::WHITE);

                if response.clicked() && !is_loading {
                    self.predict();
                }
            });

            ui.add_space(24.0);
            ui.separator();
            ui.add_space(16.0);

            // ── Results ─────────────────────────────────────────
            match &self.state.clone() {
                PredictionState::Idle => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("📊  Pilih folder berisi 20 file and click Predict")
                                .size(15.0)
                                .color(Color32::GRAY),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Model: MLP  •  Classes: High Grade / Low Grade")
                                .size(12.0)
                                .color(Color32::from_rgb(100, 160, 150)),
                        );
                    });
                }
                PredictionState::Loading(start_time) => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        let elapsed = start_time.elapsed();
                        let s = elapsed.as_secs();
                        let ms = elapsed.subsec_millis();
                        ui.label(
                            RichText::new(format!("Analyzing... {}.{:03} seconds", s, ms))
                                .size(15.0)
                                .color(Color32::from_rgb(0, 130, 110)),
                        );
                    });
                    ctx.request_repaint();
                }
                PredictionState::Error(msg) => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.label(
                            RichText::new(format!("❌  Error: {}", msg))
                                .size(14.0)
                                .color(Color32::RED),
                        );
                    });
                }
                PredictionState::Success(result) => {
                    display_results(ui, result);
                }
            }
        });

        if matches!(self.state, PredictionState::Loading(_)) {
            ctx.request_repaint();
        }
    }
}

// ═══════════════════════════════════════════════════════
// RESULTS DISPLAY
// ═══════════════════════════════════════════════════════

fn display_results(ui: &mut egui::Ui, result: &PredictionResult) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("⏱️  Prediction Time:").size(13.0).strong());
        let s = result.prediction_time.as_secs();
        let ms = result.prediction_time.subsec_millis();
        let time_text = if s > 0 {
            format!("{}.{:03} seconds", s, ms)
        } else {
            format!("{} ms", ms)
        };
        ui.label(
            RichText::new(time_text)
                .size(13.0)
                .color(Color32::from_rgb(0, 150, 255)),
        );
    });

    ui.add_space(12.0);

    let is_high = result.predicted_class == "High Grade";
    let (box_color, text_color, icon) = if is_high {
        (
            Color32::from_rgb(220, 250, 245),
            Color32::from_rgb(0, 100, 85),
            "🏆",
        )
    } else {
        (
            Color32::from_rgb(255, 225, 225),
            Color32::from_rgb(180, 0, 0),
            "📉",
        )
    };

    egui::Frame::none()
        .fill(box_color)
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::same(20.0))
        .stroke(egui::Stroke::new(2.0, text_color))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!("{}  {}", icon, result.predicted_class))
                        .size(34.0)
                        .strong()
                        .color(text_color),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("Confidence: {:.1}%", result.confidence * 100.0))
                        .size(15.0)
                        .color(text_color),
                );
            });
        });

    ui.add_space(20.0);

    ui.label(
        RichText::new("📊  Confidence Score")
            .size(14.0)
            .strong()
            .color(Color32::DARK_GRAY),
    );
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("High Grade:").size(13.0).strong());
        ui.add_space(4.0);
        let p = result.high_grade_prob;
        let bar_color = if p > 0.5 {
            Color32::from_rgb(0, 130, 110)
        } else {
            Color32::from_rgb(120, 190, 180)
        };
        ui.add(egui::ProgressBar::new(p).fill(bar_color).desired_width(280.0));
        ui.label(RichText::new(format!("{:.1}%", p * 100.0)).size(13.0).strong());
    });

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Low Grade:").size(13.0).strong());
        ui.add_space(4.0);
        let p = result.low_grade_prob;
        let bar_color = if p > 0.5 {
            Color32::from_rgb(200, 0, 0)
        } else {
            Color32::from_rgb(200, 150, 150)
        };
        ui.add(egui::ProgressBar::new(p).fill(bar_color).desired_width(280.0));
        ui.label(RichText::new(format!("{:.1}%", p * 100.0)).size(13.0).strong());
    });

    ui.add_space(16.0);
}

// ═══════════════════════════════════════════════════════
// PREDICTION LOGIC
// ═══════════════════════════════════════════════════════

fn run_prediction(
    folder_path: &str,
    mut model: CoffeeMLP,
    start_time: Instant,
) -> Result<PredictionResult, String> {
    let mut csvs: Vec<std::path::PathBuf> = Vec::new();
    let rd = std::fs::read_dir(folder_path).map_err(|e| format!("Gagal baca folder: {}", e))?;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x.eq_ignore_ascii_case("csv")).unwrap_or(false) { csvs.push(p); }
    }
    csvs.sort();
    if csvs.is_empty() { return Err("Tidak ada file CSV di folder itu.".to_string()); }

    let mut n_high = 0usize; let mut n_low = 0usize;
    let mut conf_high_sum = 0.0f32; let mut conf_low_sum = 0.0f32; let mut n_ok = 0usize;
    for path in &csvs {
        let pstr = path.to_string_lossy().to_string();
        let extracted = match extract_features_via_python(&pstr) { Ok(e) => e, Err(_) => continue };
        let (p_high, p_low) = model.predict_proba_features(&extracted.features);
        if p_high >= p_low { n_high += 1; conf_high_sum += p_high; }
        else { n_low += 1; conf_low_sum += p_low; }
        n_ok += 1;
    }
    if n_ok == 0 { return Err("Semua file gagal diekstrak (cek Python/tsfresh).".to_string()); }

    let (predicted_class, high_grade_prob, low_grade_prob, confidence) = if n_high >= n_low {
        let c = if n_high > 0 { conf_high_sum / n_high as f32 } else { 0.0 };
        ("High Grade".to_string(), c, 1.0 - c, c)
    } else {
        let c = if n_low > 0 { conf_low_sum / n_low as f32 } else { 0.0 };
        ("Low Grade".to_string(), 1.0 - c, c, c)
    };

    println!("📊 Folder: {} file OK, vote High={} Low={}", n_ok, n_high, n_low);

    Ok(PredictionResult {
        predicted_class,
        high_grade_prob,
        low_grade_prob,
        confidence,
        prediction_time: start_time.elapsed(),
    })
}

// ═══════════════════════════════════════════════════════
// FONT SETUP
// ═══════════════════════════════════════════════════════

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Simpan daftar font emoji default egui (NotoEmoji, emoji-icon-font) agar
    // tetap jadi fallback setelah Poppins — supaya emoji tidak jadi "?".
    let default_proportional = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    if let Ok(font_data) = std::fs::read("assets/fonts/Poppins-Regular.ttf") {
        fonts.font_data.insert("Poppins".to_owned(), egui::FontData::from_owned(font_data));
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
            .insert(0, "Poppins".to_owned());
        // Poppins-only family (untuk teks yang sengaja pakai Poppins) + emoji fallback
        let mut poppins_chain = vec!["Poppins".to_owned()];
        poppins_chain.extend(default_proportional.iter().cloned());
        fonts.families.insert(egui::FontFamily::Name("Poppins".into()), poppins_chain);
    }

    if let Ok(font_data) = std::fs::read("assets/fonts/Poppins-Bold.ttf") {
        fonts.font_data.insert("PoppinsBold".to_owned(), egui::FontData::from_owned(font_data));
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
            .insert(0, "PoppinsBold".to_owned());
        let mut bold_chain = vec!["PoppinsBold".to_owned()];
        bold_chain.extend(default_proportional.iter().cloned());
        fonts.families.insert(egui::FontFamily::Name("PoppinsBold".into()), bold_chain);
    }

    ctx.set_fonts(fonts);
}

// ═══════════════════════════════════════════════════════
// MAIN
// ═══════════════════════════════════════════════════════

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 580.0])
            .with_title("MLP LOOO Coffee Grade Predictor"),
        ..Default::default()
    };

    eframe::run_native(
        "MLP LOOO LOOO Coffee Predictor",
        options,
        Box::new(|cc| Ok(Box::new(MLPPredictorApp::new(cc)))),
    )
}