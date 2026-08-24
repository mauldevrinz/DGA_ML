//! CNN Cross Day Assessment GUI
//! Train: full CNN model (all 12 origins, loo_cnn_full_model.json).
//! Metrik per-file (accuracy/precision/recall/F1/AUC + confusion matrix) + ringkasan


use coffee_classifier::ml::*;
use coffee_classifier::report::{generate_independent_pdf, IndepReport, IndepSample};
use eframe::egui;
use egui::{Color32, RichText};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Instant;

const INDEP_BASE: &str = "data/Sample Ulang dan Baru";

#[derive(Clone)]
struct EvalDone {
    samples: Vec<IndepSample>,
    confusion: [[usize; 2]; 2],
    scores: Vec<f32>,
    labels: Vec<i64>,
    secs: f64,
}

enum State {
    Idle,
    Running(Instant),
    Done(EvalDone),
    Error(String),
}

struct App {
    state: State,
    rx: Option<Receiver<Result<EvalDone, String>>>,
    model_status: String,
    last_pdf: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self { state: State::Idle, rx: None, model_status: String::new(), last_pdf: None }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut app = Self::default();
        let mut parts = Vec::new();
        if Path::new("models/loo_cnn_full_model.json").exists() { parts.push("model ready"); }
        else { parts.push("model not ready (will be trained on START)"); }
        app.model_status = parts.join("  |  ");
        app
    }

    fn start_eval(&mut self) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.state = State::Running(Instant::now());
        thread::spawn(move || {
            let res = run_independent_eval();
            let _ = tx.send(res);
        });
    }
}

/// Pastikan model full (12 origin) ada. Kalau belum, latih & simpan.
fn ensure_full_model() -> Result<(), String> {
    if Path::new("models/loo_cnn_full_model.json").exists()
        && Path::new("models/loo_cnn_full_norm.json").exists() {
        return Ok(());
    }
    println!("🔧 Model full belum ada — melatih CNN FULL (12 origin)...");
    let dataset = FeatureLoader::new("data/features").load_raw()
        .map_err(|e| format!("Failed to load 12-origin dataset: {}", e))?;
    let nf = dataset.features.ncols();
    let full_norm = FeatureNormStats::fit(&dataset.features);
    let full_feat = full_norm.transform(&dataset.features);
    let mut full_cnn = ndarray::Array3::<f32>::zeros((full_feat.nrows(), 1, nf));
    for i in 0..full_feat.nrows() { for j in 0..nf { full_cnn[[i,0,j]] = full_feat[[i,j]]; } }

    let conv = |l: usize, k: usize| (l - k) + 1;
    let pool = |l: usize| (l - 2) / 2 + 1;
    let mut len = nf;
    len = conv(len, 7); len = pool(len);
    len = conv(len, 5); len = pool(len);
    let flatten_size = 64 * len;

    let training_config = TrainingConfig {
        batch_size: 8,
        learning_rate: 0.0042,
        num_epochs: 100,
        weight_decay: 0.0001,
        patience: 100000,
        min_delta: 0.0,
        max_overfitting_gap: 1.0,
        stop_on_perfect: false,
    };
    let mut full_model = CoffeeCNN::new_with_input(1, flatten_size);
    let trainer = Trainer::new(training_config);
    let _ = trainer.train_with_callback(
        &mut full_model,
        &full_cnn, &dataset.labels,
        &full_cnn, &dataset.labels,
        None,
    );
    std::fs::create_dir_all("models").ok();
    full_model.save("models/loo_cnn_full_model.json")
        .map_err(|e| format!("Failed to save model: {}", e))?;
    full_norm.save("models/loo_cnn_full_norm.json")
        .map_err(|e| format!("Failed to save normalization: {}", e))?;
    println!("✅ Model CNN FULL + norm tersimpan.");
    Ok(())
}

/// Evaluasi independent: load model full, prediksi semua file di 5 folder, hitung metrik.
fn run_independent_eval() -> Result<EvalDone, String> {
    let start = Instant::now();
    ensure_full_model()?;
    let norm = load_norm("models/loo_cnn_full_norm.json")
        .map_err(|e| format!("Failed to load normalization: {}", e))?;
    let mut model = CoffeeCNN::load("models/loo_cnn_full_model.json")
        .map_err(|_| "Failed to load full model.".to_string())?;

    // scan folder independen: tiap subfolder prefix HIGH_/LOW_
    let base = Path::new(INDEP_BASE);
    if !base.exists() {
        return Err(format!("Folder not found: {}", INDEP_BASE));
    }
    let mut sample_dirs: Vec<(String, i64, PathBuf)> = Vec::new();
    for e in std::fs::read_dir(base).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        if !p.is_dir() { continue; }
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let up = name.to_uppercase();
        let label = if up.starts_with("HIGH") { 1 } else if up.starts_with("LOW") { 0 } else { continue };
        sample_dirs.push((name, label, p));
    }
    sample_dirs.sort_by(|a, b| a.0.cmp(&b.0));
    if sample_dirs.is_empty() {
        return Err("No HIGH_/LOW_ subfolders found in independent folder.".to_string());
    }

    let mut samples: Vec<IndepSample> = Vec::new();
    let mut confusion = [[0usize; 2]; 2];
    let mut all_scores: Vec<f32> = Vec::new();
    let mut all_labels: Vec<i64> = Vec::new();

    for (name, true_label, dir) in &sample_dirs {
        let sample_start = Instant::now();
        // cari semua CSV (boleh dalam subfolder 1 level)
        let mut csvs: Vec<PathBuf> = Vec::new();
        collect_csvs(dir, &mut csvs);
        csvs.sort();
        if csvs.is_empty() { continue; }

        let mut n_high = 0usize; let mut n_low = 0usize;
        let mut conf_high_sum = 0.0f32; let mut conf_low_sum = 0.0f32;
        for path in &csvs {
            let pstr = path.to_string_lossy().to_string();
            let extracted = match extract_features_via_python(&pstr) { Ok(e) => e, Err(_) => continue };
            let feat_norm = norm.transform(&extracted.features);
            let nf = feat_norm.ncols();
            let mut input_cnn = ndarray::Array3::<f32>::zeros((1, 1, nf));
            for j in 0..nf { input_cnn[[0, 0, j]] = feat_norm[[0, j]]; }
            let probs = model.predict(&input_cnn);
            let p_high = probs[[0, 1]];
            let p_low = probs[[0, 0]];
            let pred = if p_high >= p_low { 1i64 } else { 0i64 };
            // confusion per-file: [true][pred]
            confusion[*true_label as usize][pred as usize] += 1;
            all_scores.push(p_high);
            all_labels.push(*true_label);
            if pred == 1 { n_high += 1; conf_high_sum += p_high; }
            else { n_low += 1; conf_low_sum += p_low; }
        }
        let n_files = n_high + n_low;
        if n_files == 0 { continue; }
        let (pred_label, confidence) = if n_high >= n_low {
            (1i64, if n_high > 0 { conf_high_sum / n_high as f32 } else { 0.0 })
        } else {
            (0i64, if n_low > 0 { conf_low_sum / n_low as f32 } else { 0.0 })
        };
        samples.push(IndepSample {
            name: name.clone(),
            true_label: *true_label,
            n_files, n_high, n_low, pred_label, confidence,
            secs: sample_start.elapsed().as_secs_f64(),
        });
        println!("📊 {}: {} file, vote H={} L={} -> {}", name, n_files, n_high, n_low,
            if pred_label == 1 { "HIGH" } else { "LOW" });
    }

    if samples.is_empty() {
        return Err("All samples failed to extract (check Python/tsfresh).".to_string());
    }

    Ok(EvalDone {
        samples, confusion,
        scores: all_scores, labels: all_labels,
        secs: start.elapsed().as_secs_f64(),
    })
}

fn collect_csvs(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_csvs(&p, out);
            } else if p.extension().map(|x| x.eq_ignore_ascii_case("csv")).unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

fn load_norm(path: &str) -> Result<FeatureNormStats, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read norm stats: {}", e))?;
    serde_json::from_str(&json).map_err(|e| format!("Failed to parse norm stats: {}", e))
}

fn metrics(cm: &[[usize; 2]; 2]) -> (f32, f32, f32, f32) {
    let tp = cm[1][1] as f32; let fp = cm[0][1] as f32;
    let fn_ = cm[1][0] as f32; let tn = cm[0][0] as f32;
    let total = tp + fp + fn_ + tn;
    let acc = if total > 0.0 { (tp + tn) / total } else { 0.0 };
    let prec = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    let rec = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
    let f1 = if prec + rec > 0.0 { 2.0 * prec * rec / (prec + rec) } else { 0.0 };
    (acc, prec, rec, f1)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.rx {
            if let Ok(res) = rx.try_recv() {
                match res {
                    Ok(d) => self.state = State::Done(d),
                    Err(e) => self.state = State::Error(e),
                }
                self.rx = None;
            }
        }

        // Header
        egui::TopBottomPanel::top("hdr").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("☕  CNN Cross-Day Validation Batch Assessment")
                    .size(22.0).strong().color(Color32::from_rgb(200, 165, 30)));
                ui.label(RichText::new("Train: full model (12 origins)  •  Test: cross-day batches")
                    .size(12.0).color(Color32::from_rgb(120, 120, 120)));
            });
            ui.add_space(8.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new(&self.model_status).size(11.0).color(Color32::from_rgb(90, 90, 90)));
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                // START (orange)
                let running = matches!(self.state, State::Running(_));
                let start_btn = egui::Button::new(RichText::new("▶  START").size(15.0).color(Color32::WHITE))
                    .fill(Color32::from_rgb(200, 165, 30)).min_size(egui::vec2(150.0, 40.0));
                if ui.add_enabled(!running, start_btn).clicked() {
                    self.start_eval();
                }
                ui.add_space(12.0);
                // ASSESSMENT (PDF)
                let can_assess = matches!(self.state, State::Done(_));
                let assess_btn = egui::Button::new(RichText::new("📄  ASSESSMENT").size(15.0).color(Color32::WHITE))
                    .fill(Color32::from_rgb(160, 130, 20)).min_size(egui::vec2(170.0, 40.0));
                if ui.add_enabled(can_assess, assess_btn).clicked() {
                    if let State::Done(d) = &self.state {
                        let rep = IndepReport {
                            model_name: "CNN".to_string(),
                            samples: d.samples.clone(),
                            confusion: d.confusion,
                            scores: d.scores.clone(),
                            labels: d.labels.clone(),
                            total_secs: d.secs,
                        };
                        match generate_independent_pdf(&rep, "testing_results") {
                            Ok(p) => { println!("✅ PDF: {}", p); self.last_pdf = Some(p); }
                            Err(e) => { self.last_pdf = Some(format!("PDF failed: {}", e)); }
                        }
                    }
                }
            });
            ui.add_space(14.0);

            match &self.state {
                State::Idle => {
                    ui.label(RichText::new("Click START to evaluate independent samples.")
                        .size(13.0).color(Color32::GRAY));
                }
                State::Running(t) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(format!("Evaluating... {:.0}s (TSFRESH extraction per file)",
                            t.elapsed().as_secs_f64())).size(13.0));
                    });
                    ctx.request_repaint();
                }
                State::Error(e) => {
                    ui.label(RichText::new(format!("❌ {}", e)).size(13.0).color(Color32::RED));
                }
                State::Done(d) => {
                    let (acc, prec, rec, f1) = metrics(&d.confusion);
                    let n_files: usize = d.samples.iter().map(|s| s.n_files).sum();
                    ui.label(RichText::new("Per-sample results").size(15.0).strong());
                    ui.add_space(4.0);
                    egui::Grid::new("samples").striped(true).show(ui, |ui| {
                        ui.label(RichText::new("Sample").strong());
                        ui.label(RichText::new("True Label").strong());
                        ui.label(RichText::new("Correct").strong());
                        ui.label(RichText::new("Wrong").strong());
                        ui.label(RichText::new("Total").strong());
                        ui.label(RichText::new("Accuracy").strong());
                        ui.label(RichText::new("Time").strong());
                        ui.end_row();
                        for s in &d.samples {
                            let tl = if s.true_label == 1 { "HIGH" } else { "LOW" };
                            let (correct, wrong) = if s.true_label == 1 {
                                (s.n_high, s.n_low)
                            } else {
                                (s.n_low, s.n_high)
                            };
                            let total = correct + wrong;
                            let acc = if total > 0 { correct as f32 / total as f32 * 100.0 } else { 0.0 };
                            let allok = wrong == 0;
                            ui.label(&s.name);
                            ui.label(tl);
                            ui.label(RichText::new(format!("{}", correct)).color(Color32::from_rgb(0, 140, 0)));
                            ui.label(RichText::new(format!("{}", wrong)).color(if wrong > 0 { Color32::RED } else { Color32::GRAY }));
                            ui.label(format!("{}", total));
                            ui.label(RichText::new(format!("{:.1}%", acc))
                                .color(if allok { Color32::from_rgb(0, 140, 0) } else { Color32::RED }));
                            ui.label(format!("{:.1}s", s.secs));
                            ui.end_row();
                        }
                    });
                    ui.add_space(12.0);
                    ui.label(RichText::new(format!("Overall (per file, n={})", n_files)).size(15.0).strong());
                    ui.add_space(4.0);
                    ui.label(format!("Accuracy: {:.4}   Precision: {:.4}   Recall: {:.4}   F1: {:.4}",
                        acc, prec, rec, f1));
                    ui.label(format!("Confusion  [HIGH: pred H={} L={}]  [LOW: pred H={} L={}]",
                        d.confusion[1][1], d.confusion[1][0], d.confusion[0][1], d.confusion[0][0]));
                    ui.add_space(6.0);
                    ui.label(RichText::new("Click ASSESSMENT to generate the PDF report.").size(12.0).color(Color32::GRAY));
                    if let Some(p) = &self.last_pdf {
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("PDF: {}", p)).size(11.0).color(Color32::from_rgb(0, 110, 0)));
                    }
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    println!("=== Cross-Day CNN (train full 12 origin, batches test) ===");
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 680.0])
            .with_title("CNN Cross-Day Batch Assessment"),
        ..Default::default()
    };
    eframe::run_native("CNN Cross-Day", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
