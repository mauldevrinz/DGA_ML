//! CNN Training GUI - Pure Rust Implementation (No Gnuplot)
//! File: src/bin/train_gui.rs

use coffee_classifier::ml::*;
use coffee_classifier::ml::evaluation::EvaluationResults;
use coffee_classifier::power_monitor::PowerMonitor;
use coffee_classifier::report::{TrainingReport, generate_training_pdf};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, Corner};
use ndarray::Array1;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use colored::*;
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use serde::Serialize;
use std::process::Command;

//  visualization imports


#[allow(dead_code)]

#[derive(Serialize)]
struct MetricsLog {
    epochs: Vec<usize>,
    train_loss: Vec<f32>,
    train_accuracy: Vec<f32>,
    val_loss: Vec<f32>,
    val_accuracy: Vec<f32>,
}

fn save_metrics_json(metrics: &[TrainingMetrics], path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let log = MetricsLog {
        epochs: (1..=metrics.len()).collect(),
        train_loss: metrics.iter().map(|m| m.train_loss).collect(),
        train_accuracy: metrics.iter().map(|m| m.train_accuracy).collect(),
        val_loss: metrics.iter().map(|m| m.val_loss).collect(),
        val_accuracy: metrics.iter().map(|m| m.val_accuracy).collect(),
    };
    let json = serde_json::to_string_pretty(&log)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[derive(Clone, Debug)]
struct FolderNode {
    name: String,
    files: Vec<String>,
    expanded: bool,
}

struct TrainingGUI {
    is_training: bool,
    training_complete: bool,
    metrics_history: Arc<Mutex<Vec<TrainingMetrics>>>,
    live_preview: Option<TrainingMetrics>,
    start_time: Option<Instant>,
    final_time: Option<f64>,
    metrics_receiver: Option<Receiver<TrainingMetrics>>,
    eval_receiver: Option<Receiver<(EvaluationResults, EvaluationResults)>>,
    current_epoch: usize,
    #[allow(dead_code)]
    total_epochs: usize,
    train_eval: Option<EvaluationResults>,
    val_eval: Option<EvaluationResults>,
    high_grade_folders: Vec<FolderNode>,
    low_grade_folders: Vec<FolderNode>,
    high_grade_expanded: bool,
    low_grade_expanded: bool,
    last_refresh: Instant,
    refresh_interval: Duration,
    file_scan_receiver: Option<Receiver<(Vec<FolderNode>, Vec<FolderNode>)>>,
    power_monitor: Option<PowerMonitor>,
    training_start_sample: usize,
    pdf_saved: Option<String>,
}

impl Default for TrainingGUI {
    fn default() -> Self {
        Self {
            is_training: false,
            training_complete: false,
            metrics_history: Arc::new(Mutex::new(Vec::new())),
            live_preview: None,
            start_time: None,
            final_time: None,
            metrics_receiver: None,
            eval_receiver: None,
            current_epoch: 0,
            total_epochs: 100,
            train_eval: None,
            val_eval: None,
            high_grade_folders: Vec::new(),
            low_grade_folders: Vec::new(),
            high_grade_expanded: false,
            low_grade_expanded: false,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(2),
            file_scan_receiver: None,
            power_monitor: Some(PowerMonitor::start()),
            training_start_sample: 0,
            pdf_saved: None,
        }
    }
}

impl TrainingGUI {
    fn new() -> Self {
        let mut gui = Self::default();
        // Initial load: spawn background thread so GUI starts responsive
        let (tx, rx) = channel();
        gui.file_scan_receiver = Some(rx);
        thread::spawn(move || {
            let _ = tx.send(Self::scan_data_files());
        });
        gui
    }
    
    /// Scan data files from disk — runs in background thread, NOT on UI thread.
    fn scan_data_files() -> (Vec<FolderNode>, Vec<FolderNode>) {
        fn scan_grade_path(path: &PathBuf) -> Vec<FolderNode> {
            let mut result = Vec::new();
            if let Ok(entries) = fs::read_dir(path) {
                let mut folders: Vec<_> = entries.flatten()
                    .filter(|e| e.path().is_dir())
                    .collect();
                folders.sort_by_key(|e| e.file_name());
                for entry in folders {
                    let folder_path = entry.path();
                    let folder_name = folder_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let mut files = Vec::new();
                    if let Ok(file_entries) = fs::read_dir(&folder_path) {
                        let mut csv_files: Vec<_> = file_entries.flatten()
                            .filter(|f| {
                                let p = f.path();
                                p.is_file() && p.extension()
                                    .and_then(|ext| ext.to_str())
                                    .map_or(false, |ext| ext == "csv")
                            })
                            .collect();
                        csv_files.sort_by_key(|f| f.file_name());
                        for file in csv_files {
                            files.push(file.file_name().to_string_lossy().to_string());
                        }
                    }
                    result.push(FolderNode { name: folder_name, files, expanded: false });
                }
            }
            result
        }

        let high = scan_grade_path(&PathBuf::from("data/raw/high_grade"));
        let low  = scan_grade_path(&PathBuf::from("data/raw/low_grade"));
        (high, low)
    }

    fn auto_refresh_data(&mut self) {
        // Check if a previous background scan finished
        let mut scan_done = false;
        if let Some(ref rx) = self.file_scan_receiver {
            if let Ok((high, low)) = rx.try_recv() {
                // Preserve expanded states when updating
                let high_states: HashMap<String, bool> = self.high_grade_folders.iter()
                    .map(|f| (f.name.clone(), f.expanded)).collect();
                let low_states: HashMap<String, bool> = self.low_grade_folders.iter()
                    .map(|f| (f.name.clone(), f.expanded)).collect();

                self.high_grade_folders = high;
                self.low_grade_folders = low;

                for f in &mut self.high_grade_folders {
                    if let Some(&exp) = high_states.get(&f.name) { f.expanded = exp; }
                }
                for f in &mut self.low_grade_folders {
                    if let Some(&exp) = low_states.get(&f.name) { f.expanded = exp; }
                }
                scan_done = true;
            }
        }
        if scan_done {
            self.file_scan_receiver = None;
        }

        // Schedule next background scan when interval elapses (only if no scan in flight)
        if self.file_scan_receiver.is_none() && self.last_refresh.elapsed() >= self.refresh_interval {
            let (tx, rx) = channel();
            self.file_scan_receiver = Some(rx);
            self.last_refresh = Instant::now();
            thread::spawn(move || {
                let _ = tx.send(Self::scan_data_files());
            });
        }
    }
    
    fn open_file_explorer() {
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg("data/raw")
                .spawn()
                .ok();
        }
        
        #[cfg(target_os = "windows")]
        {
            Command::new("explorer")
                .arg("data\\raw")
                .spawn()
                .ok();
        }
        
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg("data/raw")
                .spawn()
                .ok();
        }
    }
    
    
    

    
    
    
    fn start_training(&mut self) {
        if self.is_training {
            return;
        }
        
        self.is_training = true;
        self.training_complete = false;
        self.start_time = Some(Instant::now());
        self.final_time = None;
        self.current_epoch = 0;
        self.train_eval = None;
        self.val_eval = None;
        self.live_preview = None;
        let needs_new_monitor = self.power_monitor.as_ref().map(|pm| pm.is_stopped()).unwrap_or(true);
        if needs_new_monitor {
            self.power_monitor = Some(PowerMonitor::start());
            self.training_start_sample = 0;
        } else {
            self.training_start_sample = self.power_monitor.as_ref().map(|pm| pm.sample_count()).unwrap_or(0);
        }
        
        if let Ok(mut history) = self.metrics_history.lock() {
            history.clear();
        }
        
        let (tx, rx) = channel();
        let (eval_tx, eval_rx) = channel();
        self.metrics_receiver = Some(rx);
        self.eval_receiver = Some(eval_rx);
        let metrics_history = Arc::clone(&self.metrics_history);
        
        thread::spawn(move || {
            println!(
                "\n{}",
                "╔═══════════════════════════════════════════════════╗"
                    .bold()
                    .cyan()
            );
            println!(
                "{}",
                "║      Coffee Arabica 1D-CNN Classifier           ║"
                    .bold()
                    .cyan()
            );
            println!(
                "{}",
                "║    CNN from Scratch with Temperature Scaling    ║"
                    .bold()
                    .cyan()
            );
            println!(
                "{}",
                "╚═══════════════════════════════════════════════════╝"
                    .bold()
                    .cyan()
            );
            
            let total_start = Instant::now();
            
            let training_config = TrainingConfig {
                batch_size: 8,
                learning_rate: 0.0042,
                num_epochs: 100,
                weight_decay: 0.0001,
                patience: 25,
                min_delta: 0.001,
                max_overfitting_gap: 0.50,
                stop_on_perfect: false,
            };
            
            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 1/7: Loading Dataset".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            
            let load_start = Instant::now();
            // Load fitur TSFRESH (seragam dengan RF/SVM/MLP)
            let dataset = match FeatureLoader::new("data/features").load_raw() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("❌ Gagal load TSFRESH features: {}", e);
                    eprintln!("   Jalankan dulu: python tools/python/tsfresh_pipeline.py");
                    return;
                }
            };
            // Split + z-score anti-leakage (fit di train saja)
            let split = stratified_split(&dataset, 0.2, Some(42));
            let n_features = dataset.features.ncols();
            let load_time = load_start.elapsed();

            println!("\n📊 Dataset Summary (TSFRESH):");
            println!("   Total fitur  : {}", n_features);
            println!("   Train samples: {}", split.train_labels.len());
            println!("   Val samples  : {}", split.val_labels.len());
            let train_high = split.train_labels.iter().filter(|&&x| x == 1).count();
            let train_low  = split.train_labels.iter().filter(|&&x| x == 0).count();
            let val_high   = split.val_labels.iter().filter(|&&x| x == 1).count();
            let val_low    = split.val_labels.iter().filter(|&&x| x == 0).count();
            println!("   Train: {} High, {} Low", train_high, train_low);
            println!("   Val: {} High, {} Low", val_high, val_low);
            println!("   ⏱️  Loading time: {:.2}s", load_time.as_secs_f32());

            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 2/7: Reshape fitur → sekuens 1D".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("   ✅ {} fitur TSFRESH → bentuk (n, 1, {})", n_features, n_features);

            // CNN butuh (samples, channels, length). TSFRESH: 1 channel, panjang = n_features.
            let to_cnn = |feat: &ndarray::Array2<f32>| -> ndarray::Array3<f32> {
                let n = feat.nrows();
                let mut out = ndarray::Array3::<f32>::zeros((n, 1, n_features));
                for i in 0..n { for j in 0..n_features { out[[i, 0, j]] = feat[[i, j]]; } }
                out
            };
            let train_cnn = to_cnn(&split.train_features);
            let val_cnn = to_cnn(&split.val_features);
            let train_labels = split.train_labels.clone();
            let val_labels = split.val_labels.clone();

            // Hitung flatten size CNN dari panjang sekuens (sama rumus dgn arsitektur)
            let conv = |l: usize, k: usize| (l - k) + 1;
            let pool = |l: usize| (l - 2) / 2 + 1;
            let mut len = n_features;
            len = conv(len, 7); len = pool(len);
            len = conv(len, 5); len = pool(len);
            let flatten_size = 64 * len;

            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 3/7: Building 1D-CNN (TSFRESH input)".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());

            let mut model = CoffeeCNN::new_with_input(1, flatten_size);
            
            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 4/7: Training".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            
            let training_start = Instant::now();
            let trainer = Trainer::new(training_config);
            let final_metrics = trainer.train_with_callback(
                &mut model,
                &train_cnn,
                &train_labels,
                &val_cnn,
                &val_labels,
                Some(tx),
            );
            
            let training_time = training_start.elapsed();
            
            if let Ok(mut history) = metrics_history.lock() {
                *history = final_metrics.clone();
            }
            
            let best_val_loss = final_metrics
                .iter()
                .map(|m| m.val_loss)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);
            let best_val_acc = final_metrics
                .iter()
                .map(|m| m.val_accuracy)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);
            
            println!("\n🏆 Training Summary:");
            println!("   Total epochs: {}", final_metrics.len());
            println!("   Best val loss: {:.4}", best_val_loss);
            println!("   Best val acc: {:.2}%", best_val_acc * 100.0);
            println!(
                "   ⏱️  Training time: {:.2}s ({:.2}min)",
                training_time.as_secs_f32(),
                training_time.as_secs_f32() / 60.0
            );
            
            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 5/7: Skipped (GUI Mode)".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            
            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 6/7: Temperature Calibration".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            
            let calibration_start = Instant::now();
            model.calibrate_temperature(&val_cnn, &val_labels);
            let calibration_time = calibration_start.elapsed();
            println!("   ⏱️  Calibration time: {:.2}s", calibration_time.as_secs_f32());
            
            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 7/7: Final Evaluation (Calibrated)".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            
            let train_probs = model.predict(&train_cnn);
            let mut train_preds = Array1::<i64>::zeros(train_probs.shape()[0]);
            for i in 0..train_probs.shape()[0] {
                let row = train_probs.row(i);
                train_preds[i] = if row[0] > row[1] { 0 } else { 1 };
            }
            
            let train_eval = Evaluator::evaluate(&train_preds, &train_labels);
            Evaluator::print_results(&train_eval, "Train");
            
            let val_probs = model.predict(&val_cnn);
            let mut val_preds = Array1::<i64>::zeros(val_probs.shape()[0]);
            
            println!("\n📊 Calibrated Confidence Distribution:");
            let mut high_confidences = Vec::new();
            let mut low_confidences = Vec::new();
            
            for i in 0..val_probs.shape()[0] {
                let row = val_probs.row(i);
                val_preds[i] = if row[0] > row[1] { 0 } else { 1 };
                let confidence = row[0].max(row[1]);
                
                if val_labels[i] == 1 {
                    high_confidences.push(confidence);
                } else {
                    low_confidences.push(confidence);
                }
            }
            
            if !high_confidences.is_empty() {
                let avg_high_conf = high_confidences.iter().sum::<f32>() / high_confidences.len() as f32;
                println!("   High Grade samples: avg confidence = {:.1}%", avg_high_conf * 100.0);
            }
            
            if !low_confidences.is_empty() {
                let avg_low_conf = low_confidences.iter().sum::<f32>() / low_confidences.len() as f32;
                println!("   Low Grade samples: avg confidence = {:.1}%", avg_low_conf * 100.0);
            }
            
            let val_eval = Evaluator::evaluate(&val_preds, &val_labels);
            Evaluator::print_results(&val_eval, "Validation");
            
            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Saving Calibrated Model & Metrics".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            
            std::fs::create_dir_all("models").ok();
            
            model.save("models/trained_model.json").ok();
            println!("✅ Model saved: models/trained_model.json");
            
            if let Ok(norm_json) = serde_json::to_string_pretty(&split.norm_stats) {
                std::fs::write("models/cnn_norm_stats.json", norm_json).ok();
                println!("✅ Normalization stats saved: models/cnn_norm_stats.json");
            }
            
            if let Ok(history) = metrics_history.lock() {
                if let Ok(_) = save_metrics_json(&history, "models/training_metrics.json") {
                    println!("✅ Training metrics saved: models/training_metrics.json");
                }
            }
            
            let total_time = total_start.elapsed();
            
            println!(
                "\n{}",
                "╔═══════════════════════════════════════════════════╗"
                    .bold()
                    .green()
            );
            println!(
                "{}",
                "║    Training & Calibration Complete! 🎉          ║"
                    .bold()
                    .green()
            );
            println!(
                "{}",
                "╚═══════════════════════════════════════════════════╝"
                    .bold()
                    .green()
            );
            
            println!(
                "\n⏱️  Total Training Time: {:.2}s ({:.2}min)",
                total_time.as_secs_f32(),
                total_time.as_secs_f32() / 60.0
            );
            
            println!("\n📂 Generated Files:");
            println!("   ✓ models/trained_model.json");
            println!("   ✓ models/normalization_stats.json");
            println!("   ✓ models/training_metrics.json");
            
            let _ = eval_tx.send((train_eval, val_eval));
            println!("\n✅ Training completed successfully!");
        });
    }
    
    fn update_metrics(&mut self) {
        if let Some(ref receiver) = self.metrics_receiver {
            while let Ok(metrics) = receiver.try_recv() {
                if metrics.is_final {
                    // Completed epoch — push to history and clear preview
                    if let Ok(mut history) = self.metrics_history.lock() {
                        history.push(metrics);
                        self.current_epoch = history.len();
                    }
                    self.live_preview = None;
                } else {
                    // Intra-epoch batch preview — just overwrite live_preview
                    self.live_preview = Some(metrics);
                }
            }
        }
        
        if let Some(ref eval_receiver) = self.eval_receiver {
            if let Ok((train_eval, val_eval)) = eval_receiver.try_recv() {
                self.train_eval = Some(train_eval);
                self.val_eval = Some(val_eval);
                self.is_training = false;
                self.training_complete = true;
                if let Some(ref pm) = self.power_monitor { pm.stop(); }
                
                if let Some(start) = self.start_time {
                    self.final_time = Some(start.elapsed().as_secs_f64());
                }
            }
        }
    }
    
    fn draw_tree_view(&mut self, ui: &mut egui::Ui, is_high_grade: bool) {
        let (folders, expanded) = if is_high_grade {
            (&mut self.high_grade_folders, &mut self.high_grade_expanded)
        } else {
            (&mut self.low_grade_folders, &mut self.low_grade_expanded)
        };
        
        let total_folders = folders.len();
        let root_name = if is_high_grade { "high_grade" } else { "low_grade" };
        let arrow = if *expanded { "▼" } else { "►" };
        let folder_icon = if *expanded { "📂" } else { "📁" };
        
        ui.horizontal(|ui| {
            let response = ui.button(format!("{} {} {} ({} folders)", arrow, folder_icon, root_name, total_folders));
            if response.clicked() {
                *expanded = !*expanded;
            }
        });
        
        if *expanded {
            for folder in folders.iter_mut() {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let folder_arrow = if folder.expanded { "▼" } else { "►" };
                    let subfolder_icon = if folder.expanded { "📂" } else { "📁" };
                    let response = ui.button(format!("{} {} {} ({} files)", folder_arrow, subfolder_icon, folder.name, folder.files.len()));
                    if response.clicked() {
                        folder.expanded = !folder.expanded;
                    }
                });
                
                if folder.expanded {
                    for file in &folder.files {
                        ui.horizontal(|ui| {
                            ui.add_space(40.0);
                            ui.label(egui::RichText::new(format!("📄 {}", file))
                                .size(12.0)
                                .color(egui::Color32::from_rgb(80, 80, 80))
                                .family(egui::FontFamily::Monospace));
                        });
                    }
                }
            }
        }
    }
}

impl eframe::App for TrainingGUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_metrics();
        
        if !self.training_complete {
            self.auto_refresh_data();
        }
        
        ctx.request_repaint_after(Duration::from_millis(100));
        
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = egui::Color32::from_rgb(225, 225, 225);
        visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::BLACK;
        ctx.set_visuals(visuals);
        
        egui::TopBottomPanel::top("header")
            .exact_height(60.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(15, 92, 112)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("CNN")
                        .size(28.0)
                        .color(egui::Color32::WHITE)
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                });
            });
        
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(225, 225, 225)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_source("main_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                ui.add_space(15.0);
                
                if !self.training_complete {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Data (Split data 80% Train, 20% Val per folder)")
                            .size(16.0)
                            .color(egui::Color32::BLACK)
                            .family(egui::FontFamily::Name("PoppinsBold".into())));
                    });
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        
                        ui.vertical(|ui| {
                            egui::Frame::none()
                                .fill(egui::Color32::WHITE)
                                .inner_margin(15.0)
                                .rounding(5.0)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)))
                                .show(ui, |ui| {
                                    ui.set_width(590.0);
                                    ui.set_height(220.0);
                                    
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("High grade")
                                            .size(18.0)
                                            .color(egui::Color32::BLACK)
                                            .family(egui::FontFamily::Name("PoppinsBold".into())));
                                    });
                                    ui.add_space(8.0);
                                    
                                    egui::ScrollArea::vertical()
                                        .id_source("high_grade_scroll")
                                        .max_height(170.0)
                                        .auto_shrink([false; 2])
                                        .show(ui, |ui| {
                                            self.draw_tree_view(ui, true);
                                        });
                                });
                        });
                        
                        ui.add_space(15.0);
                        
                        ui.vertical(|ui| {
                            egui::Frame::none()
                                .fill(egui::Color32::WHITE)
                                .inner_margin(15.0)
                                .rounding(5.0)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)))
                                .show(ui, |ui| {
                                    ui.set_width(590.0);
                                    ui.set_height(220.0);
                                    
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("Low Grade")
                                            .size(18.0)
                                            .color(egui::Color32::BLACK)
                                            .family(egui::FontFamily::Name("PoppinsBold".into())));
                                    });
                                    ui.add_space(8.0);
                                    
                                    egui::ScrollArea::vertical()
                                        .id_source("low_grade_scroll")
                                        .max_height(170.0)
                                        .auto_shrink([false; 2])
                                        .show(ui, |ui| {
                                            self.draw_tree_view(ui, false);
                                        });
                                });
                        });
                    });
                    ui.add_space(15.0);
                }
                
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    
                    ui.vertical(|ui| {
                        ui.group(|ui| {
                            ui.set_width(590.0);
                            ui.set_height(250.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("Train")
                                    .size(20.0)
                                    .family(egui::FontFamily::Name("PoppinsBold".into())));
                            });
                            self.draw_metrics_plot(ui, true);
                        });
                    });
                    
                    ui.add_space(15.0);
                    
                    ui.vertical(|ui| {
                        ui.group(|ui| {
                            ui.set_width(590.0);
                            ui.set_height(250.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("Validation")
                                    .size(20.0)
                                    .family(egui::FontFamily::Name("PoppinsBold".into())));
                            });
                            self.draw_metrics_plot(ui, false);
                        });
                    });
                });
                ui.add_space(15.0);

                // ── Live Power (during training) ─────────────────────
                if self.is_training {
                    if let Some(ref pm) = self.power_monitor {
                        if let Some(s) = pm.current_sample() {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new("⚡ Power:")
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(60, 60, 60))
                                    .family(egui::FontFamily::Name("Poppins".into())));
                                let total_j = pm.accumulated_joules_from(self.training_start_sample);
                                ui.label(egui::RichText::new(format!(
                                    "System {:.1} W  |  CPU+GPU {:.1} W  |  🌡 CPU {:.1}°C  GPU {:.1}°C  |  ⚡ Total: {:.1} J ({:.4} Wh)",
                                    s.vdd_in_mw as f32 / 1000.0,
                                    s.vdd_cpu_gpu_mw as f32 / 1000.0,
                                    s.cpu_temp_c,
                                    s.gpu_temp_c,
                                    total_j,
                                    total_j / 3600.0,
                                ))
                                .size(13.0)
                                .color(egui::Color32::from_rgb(15, 92, 112))
                                .family(egui::FontFamily::Name("Poppins".into())));
                            });
                            ui.add_space(6.0);
                        }
                    }
                }

                if self.training_complete && self.train_eval.is_some() && self.val_eval.is_some() {
                    let train_eval = self.train_eval.as_ref().unwrap();
                    let val_eval = self.val_eval.as_ref().unwrap();
                    
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.vertical(|ui| {
                            self.draw_eval_tables(ui, "Train", train_eval);
                        });
                        ui.add_space(15.0);
                        ui.vertical(|ui| {
                            self.draw_eval_tables(ui, "Validation", val_eval);
                        });
                    });
                    ui.add_space(15.0);

                    // ── Power Summary ────────────────────────────────
                    if let Some(ref pm) = self.power_monitor {
                        if let Some(ps) = pm.summary_from(self.training_start_sample) {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                egui::Frame::none()
                                    .fill(egui::Color32::from_rgb(235, 246, 250))
                                    .inner_margin(5.0)
                                    .rounding(5.0)
                                    .rounding(4.0)
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("⚡ Power Summary")
                                                .size(11.0)
                                                .color(egui::Color32::from_rgb(15, 92, 112))
                                                .family(egui::FontFamily::Name("PoppinsBold".into())));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(egui::RichText::new(format!("{} samples", ps.sample_count))
                                                    .size(10.0)
                                                    .color(egui::Color32::from_rgb(150, 150, 150))
                                                    .family(egui::FontFamily::Name("Poppins".into())));
                                            });
                                        });
                                        ui.add_space(2.0);
                                        ui.columns(4, |cols| {
                                            cols[0].vertical_centered(|ui| {
                                                ui.label(egui::RichText::new("Avg System").size(10.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                ui.label(egui::RichText::new(format!("{:.1} W", ps.avg_total_w)).size(12.0).color(egui::Color32::from_rgb(15,92,112)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                ui.label(egui::RichText::new(format!("Peak {:.1} W", ps.peak_total_w)).size(10.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                            });
                                            cols[1].vertical_centered(|ui| {
                                                ui.label(egui::RichText::new("Avg CPU+GPU").size(10.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                ui.label(egui::RichText::new(format!("{:.1} W", ps.avg_cpu_gpu_w)).size(12.0).color(egui::Color32::from_rgb(15,92,112)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                ui.label(egui::RichText::new(format!("Peak {:.1} W", ps.peak_cpu_gpu_w)).size(10.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                            });
                                            cols[2].vertical_centered(|ui| {
                                                ui.label(egui::RichText::new("CPU/GPU Temp").size(10.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                ui.label(egui::RichText::new(format!("{:.1}°C / {:.1}°C", ps.avg_cpu_temp, ps.avg_gpu_temp)).size(12.0).color(egui::Color32::from_rgb(15,92,112)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                ui.label(egui::RichText::new(format!("Peak {:.1}°C / {:.1}°C", ps.peak_cpu_temp, ps.peak_gpu_temp)).size(10.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                            });
                                            cols[3].vertical_centered(|ui| {
                                                ui.label(egui::RichText::new("🔋 Total Energy").size(10.0).color(egui::Color32::from_rgb(15,92,112)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                ui.label(egui::RichText::new(format!("{:.1} J", ps.energy_joules)).size(13.0).color(egui::Color32::from_rgb(15,92,112)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                ui.label(egui::RichText::new(format!("{:.4} Wh", ps.energy_joules / 3600.0)).size(10.0).color(egui::Color32::from_rgb(15,92,112)).family(egui::FontFamily::Name("Poppins".into())));
                                            });
                                        });
                                    });
                            });
                            ui.add_space(10.0);
                        }
                    }
                }
                
                ui.add_space(10.0);
                    }); // end ScrollArea
            });
        
        egui::TopBottomPanel::bottom("bottom_bar")
            .exact_height(70.0)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(225, 225, 225))
                .inner_margin(egui::Margin::symmetric(20.0, 15.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // START TRAINING Button
                    let button_size = egui::vec2(200.0, 40.0);
                    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
                    
                    let (fill_color, icon, text) = if self.is_training {
                        (egui::Color32::from_rgb(255, 140, 0), "⏳", "TRAINING...")
                    } else {
                        (egui::Color32::from_rgb(40, 130, 50), "▶", "START TRAINING")
                    };
                    let fill_color = if response.hovered() && !self.is_training {
                        egui::Color32::from_rgb(50, 140, 60)
                    } else { fill_color };
                    ui.painter().rect_filled(rect, 4.0, fill_color);
                    let icon_galley = ui.painter().layout_no_wrap(
                        icon.to_string(), egui::FontId::proportional(16.0), egui::Color32::WHITE);
                    let text_galley = ui.painter().layout_no_wrap(
                        format!(" {}", text),
                        egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                        egui::Color32::WHITE);
                    let total_width = icon_galley.size().x + text_galley.size().x;
                    let start_x = rect.center().x - total_width / 2.0;
                    ui.painter().galley(
                        egui::pos2(start_x, rect.center().y - icon_galley.size().y / 2.0),
                        icon_galley, egui::Color32::WHITE);
                    ui.painter().galley(
                        egui::pos2(start_x + {let icon_galley2 = ui.painter().layout_no_wrap(icon.to_string(), egui::FontId::proportional(16.0), egui::Color32::WHITE); icon_galley2.size().x},
                            rect.center().y - text_galley.size().y / 2.0),
                        text_galley, egui::Color32::WHITE);
                    if response.clicked() && !self.is_training { self.start_training(); }

                    ui.add_space(15.0);


                    // Manage Data Button
                    if !self.training_complete {
                        let (mr, mresp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                        let mc = if mresp.hovered() { egui::Color32::from_rgb(120,20,20) } else { egui::Color32::from_rgb(100,0,0) };
                        ui.painter().rect_filled(mr, 4.0, mc);
                        let mt = ui.painter().layout_no_wrap("Manage Data".into(), egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                        ui.painter().galley(egui::pos2(mr.center().x - mt.size().x/2.0, mr.center().y - mt.size().y/2.0), mt, egui::Color32::WHITE);
                        if mresp.clicked() { Self::open_file_explorer(); }
                        ui.add_space(15.0);
                    }

                    // Predict Button
                    let (pred_rect, pred_resp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                    let pred_fill = if pred_resp.hovered() { egui::Color32::from_rgb(220,120,0) } else { egui::Color32::from_rgb(200,100,0) };
                    ui.painter().rect_filled(pred_rect, 4.0, pred_fill);
                    let pt = ui.painter().layout_no_wrap("🔍 Predict".into(), egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(pred_rect.center().x - pt.size().x/2.0, pred_rect.center().y - pt.size().y/2.0), pt, egui::Color32::WHITE);
                    if pred_resp.clicked() {
                        std::thread::spawn(|| {
                            let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
                            let bin_name = "predict_gui";
                            let possible_paths = [
                                format!("./target/release/{}",   bin_name),
                                format!("target/release/{}",     bin_name),
                                format!("./target/debug/{}",     bin_name),
                                format!("target/debug/{}",       bin_name),
                            ];
                            let binary_path = possible_paths.iter()
                                .find(|p| std::path::Path::new(p.as_str()).exists())
                                .cloned()
                                .or_else(|| {
                                    std::env::current_exe().ok()
                                        .and_then(|e| e.parent().map(|d| d.join(bin_name).to_string_lossy().into_owned()))
                                });
                            match binary_path {
                                Some(path) => {
                                    if let Err(e) = std::process::Command::new(&path)
                                        .env("DISPLAY", display)
                                        .env("LIBGL_ALWAYS_SOFTWARE", "1")
                                        .env("GDK_BACKEND", "x11")
                                        .spawn()
                                    { eprintln!("[X] Failed to launch predict GUI: {}", e); }
                                }
                                None => eprintln!("[X] predict GUI binary not found: {}", bin_name),
                            }
                        });
                    }

                    ui.add_space(15.0);

                    // LOOO Button (biru tua, ukuran/bentuk/font sama dengan Predict)
                    let (loo_rect, loo_resp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                    let loo_fill = if loo_resp.hovered() { egui::Color32::from_rgb(20, 70, 110) } else { egui::Color32::from_rgb(15, 55, 90) };
                    ui.painter().rect_filled(loo_rect, 4.0, loo_fill);
                    let lt = ui.painter().layout_no_wrap("🔬 LOOO".into(), egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(loo_rect.center().x - lt.size().x/2.0, loo_rect.center().y - lt.size().y/2.0), lt, egui::Color32::WHITE);
                    if loo_resp.clicked() {
                        std::thread::spawn(|| {
                            let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
                            let bin_name = "train_loo_gui";
                            let possible_paths = [
                                format!("./target/release/{}",   bin_name),
                                format!("target/release/{}",     bin_name),
                                format!("./target/debug/{}",     bin_name),
                                format!("target/debug/{}",       bin_name),
                            ];
                            let binary_path = possible_paths.iter()
                                .find(|p| std::path::Path::new(p.as_str()).exists())
                                .cloned()
                                .or_else(|| {
                                    std::env::current_exe().ok()
                                        .and_then(|e| e.parent().map(|d| d.join(bin_name).to_string_lossy().into_owned()))
                                });
                            match binary_path {
                                Some(path) => {
                                    if let Err(e) = std::process::Command::new(&path)
                                        .env("DISPLAY", display)
                                        .env("LIBGL_ALWAYS_SOFTWARE", "1")
                                        .env("GDK_BACKEND", "x11")
                                        .spawn()
                                    { eprintln!("[X] Failed to launch LOOO GUI: {}", e); }
                                }
                                None => eprintln!("[X] LOOO GUI binary not found: {}", bin_name),
                            }
                        });
                    }
                    ui.add_space(15.0);
                    // Independent Button (oranye) — luncurkan independent_cnn_gui
                    let (ind_rect, ind_resp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                    let ind_fill = if ind_resp.hovered() { egui::Color32::from_rgb(215, 180, 45) } else { egui::Color32::from_rgb(200, 165, 30) };
                    ui.painter().rect_filled(ind_rect, 4.0, ind_fill);
                    let it = ui.painter().layout_no_wrap("🧪 Cross Day".into(), egui::FontId::new(14.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(ind_rect.center().x - it.size().x/2.0, ind_rect.center().y - it.size().y/2.0), it, egui::Color32::WHITE);
                    if ind_resp.clicked() {
                        std::thread::spawn(|| {
                            use std::process::Command;
                            let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
                            let bin_name = "independent_cnn_gui";
                            let possible_paths = [
                                format!("./target/release/{}", bin_name),
                                format!("target/release/{}", bin_name),
                                format!("./target/debug/{}", bin_name),
                                format!("target/debug/{}", bin_name),
                            ];
                            let binary_path = possible_paths.iter()
                                .find(|p| std::path::Path::new(p.as_str()).exists())
                                .cloned()
                                .or_else(|| {
                                    std::env::current_exe().ok()
                                        .and_then(|e| e.parent().map(|d| d.join(bin_name).to_string_lossy().into_owned()))
                                });
                            match binary_path {
                                Some(path) => {
                                    if let Err(e) = Command::new(&path)
                                        .env("DISPLAY", display)
                                        .env("LIBGL_ALWAYS_SOFTWARE", "1")
                                        .env("GDK_BACKEND", "x11")
                                        .spawn()
                                    { eprintln!("[X] Failed to launch Independent GUI: {}", e); }
                                }
                                None => eprintln!("[X] Independent GUI binary not found: {}", bin_name),
                            }
                        });
                    }

                    // Elapsed time (training complete)
                    if self.training_complete {
                        if let Some(final_time) = self.final_time {
                            ui.add_space(20.0);
                            let ft32 = final_time as f32;
                            let time_label = if ft32 < 1.0 {
                                format!("✅ {:.0} ms", ft32 * 1000.0)
                            } else if ft32 < 60.0 {
                                format!("✅ {:.2} s", ft32)
                            } else {
                                format!("✅ {:.2} min", ft32 / 60.0)
                            };
                            ui.label(egui::RichText::new(time_label)
                                .size(15.0).color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("PoppinsBold".into())));
                        }

                        // PDF Report button
                        ui.add_space(10.0);
                        let pdf_size = egui::vec2(170.0, 40.0);
                        let (pdf_rect, pdf_resp) = ui.allocate_exact_size(pdf_size, egui::Sense::click());
                        let pdf_fill = if pdf_resp.hovered() {
                            egui::Color32::from_rgb(50, 120, 200)
                        } else {
                            egui::Color32::from_rgb(20, 80, 160)
                        };
                        ui.painter().rect_filled(pdf_rect, 4.0, pdf_fill);
                        let pdf_t = ui.painter().layout_no_wrap(
                            "📄 PDF Report".to_string(),
                            egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                            egui::Color32::WHITE,
                        );
                        ui.painter().galley(
                            egui::pos2(
                                pdf_rect.center().x - pdf_t.size().x / 2.0,
                                pdf_rect.center().y - pdf_t.size().y / 2.0,
                            ),
                            pdf_t, egui::Color32::WHITE,
                        );
                        if pdf_resp.clicked() {
                            self.save_pdf();
                        }

                        // PDF status label
                        if let Some(ref status) = self.pdf_saved {
                            ui.add_space(8.0);
                            let (color, text) = if status.starts_with("PDF:") {
                                (egui::Color32::from_rgb(20, 140, 60), status.as_str())
                            } else {
                                (egui::Color32::from_rgb(180, 30, 30), status.as_str())
                            };
                            ui.label(egui::RichText::new(text)
                                .size(11.0)
                                .color(color)
                                .family(egui::FontFamily::Name("Poppins".into())));
                        }
                    }
                });
            });

    }
}
impl TrainingGUI {
    fn save_pdf(&mut self) {
        let history = if let Ok(h) = self.metrics_history.lock() {
            h.clone()
        } else {
            Vec::new()
        };

        let final_metrics: Vec<_> = history.iter().filter(|m| m.is_final).collect();
        let n_epochs = final_metrics.len();

        let accuracy_curve: Vec<(usize, f32, f32)> = final_metrics.iter().enumerate()
            .map(|(i, m)| (i + 1, m.train_accuracy, m.val_accuracy))
            .collect();
        let loss_curve: Vec<(usize, f32, f32)> = final_metrics.iter().enumerate()
            .map(|(i, m)| (i + 1, m.train_loss, m.val_loss))
            .collect();

        let (train_accuracy, val_accuracy, train_loss, val_loss) = if let Some(last) = final_metrics.last() {
            (last.train_accuracy, last.val_accuracy, last.train_loss, last.val_loss)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        if let (Some(train_eval), Some(val_eval)) = (self.train_eval.clone(), self.val_eval.clone()) {
            let report = TrainingReport::CNN {
                train_eval,
                val_eval,
                train_accuracy,
                val_accuracy,
                train_loss,
                val_loss,
                n_epochs,
                training_secs: self.final_time.unwrap_or(0.0),
                accuracy_curve,
                loss_curve,
                power: self.power_monitor.as_ref()
                           .and_then(|pm| pm.summary_from(self.training_start_sample)),
            };
            match generate_training_pdf(&report, "testing_results") {
                Ok(path) => self.pdf_saved = Some(format!("PDF: {}", path)),
                Err(e)   => self.pdf_saved = Some(format!("Error: {}", e)),
            }
        } else {
            self.pdf_saved = Some("Error: Training results not available".to_string());
        }
    }

    fn draw_metrics_plot(&self, ui: &mut egui::Ui, is_train: bool) {
        let history = if let Ok(history) = self.metrics_history.lock() {
            history.clone()
        } else {
            Vec::new()
        };

        // Show live preview even before first epoch completes
        let has_data = !history.is_empty() || self.live_preview.is_some();
        if !has_data {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Waiting for training data...")
                    .family(egui::FontFamily::Name("Poppins".into())));
            });
            return;
        }

        let next_x = (history.len() + 1) as f64;
        let max_x = next_x.max(100.0);

        Plot::new(if is_train { "train_plot" } else { "val_plot" })
            .legend(Legend::default().position(Corner::RightBottom))
            .show_axes([true, true])
            .show_grid([true, true])
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .x_axis_label("Epoch")
            .height(200.0)
            .width(570.0)
            .include_x(0.0)
            .include_x(max_x)
            .include_y(0.0)
            .include_y(1.0)
            .show(ui, |plot_ui| {
                // Completed epoch lines
                if !history.is_empty() {
                    let epochs: Vec<f64> = (1..=history.len()).map(|i| i as f64).collect();

                    let (loss_data, acc_data) = if is_train {
                        (
                            history.iter().map(|m| m.train_loss as f64).collect::<Vec<_>>(),
                            history.iter().map(|m| m.train_accuracy as f64).collect::<Vec<_>>(),
                        )
                    } else {
                        (
                            history.iter().map(|m| m.val_loss as f64).collect::<Vec<_>>(),
                            history.iter().map(|m| m.val_accuracy as f64).collect::<Vec<_>>(),
                        )
                    };

                    let loss_points: PlotPoints = epochs.iter()
                        .zip(loss_data.iter())
                        .map(|(x, y)| [*x, *y])
                        .collect();
                    plot_ui.line(Line::new(loss_points)
                        .color(egui::Color32::from_rgb(51, 102, 255))
                        .width(2.0)
                        .name("loss"));

                    let acc_points: PlotPoints = epochs.iter()
                        .zip(acc_data.iter())
                        .map(|(x, y)| [*x, *y])
                        .collect();
                    plot_ui.line(Line::new(acc_points)
                        .color(egui::Color32::from_rgb(255, 140, 0))
                        .width(2.0)
                        .name("accuracy"));
                }

                // Live intra-epoch preview dot (dashed, dimmer)
                if let Some(ref preview) = self.live_preview {
                    let (prev_loss, prev_acc) = if is_train {
                        (preview.train_loss as f64, preview.train_accuracy as f64)
                    } else {
                        (preview.val_loss as f64, preview.val_accuracy as f64)
                    };
                    let x = next_x;
                    plot_ui.line(Line::new(PlotPoints::new(vec![[x, prev_loss]]))
                        .color(egui::Color32::from_rgba_unmultiplied(51, 102, 255, 120))
                        .width(6.0)
                        .name("loss (live)"));
                    plot_ui.line(Line::new(PlotPoints::new(vec![[x, prev_acc]]))
                        .color(egui::Color32::from_rgba_unmultiplied(255, 140, 0, 120))
                        .width(6.0)
                        .name("accuracy (live)"));
                }
            });
    }
    
    fn draw_eval_tables(&self, ui: &mut egui::Ui, _title: &str, eval: &EvaluationResults) {
        let table_width = 590.0;
        
        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(240, 235, 220))
                .inner_margin(10.0)
                .rounding(5.0)
                .show(ui, |ui| {
                    ui.set_width(table_width);
                    
                    ui.label(egui::RichText::new("Per-Class Metrics:")
                        .color(egui::Color32::BLACK)
                        .size(14.0)
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                    ui.add_space(5.0);
                    
                    egui::Grid::new(format!("metrics_{}", eval.accuracy))
                        .striped(false)
                        .spacing([20.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Metric")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new("High Grade")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new("Low Grade")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                            
                            ui.label(egui::RichText::new("Precision")
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{:.3}", eval.high_grade_metrics.precision))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{:.3}", eval.low_grade_metrics.precision))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                            
                            ui.label(egui::RichText::new("Recall")
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{:.3}", eval.high_grade_metrics.recall))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{:.3}", eval.low_grade_metrics.recall))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                            
                            ui.label(egui::RichText::new("F1-Score")
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{:.3}", eval.high_grade_metrics.f1_score))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{:.3}", eval.low_grade_metrics.f1_score))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                            
                            ui.label(egui::RichText::new("Support")
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.high_grade_metrics.support))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.low_grade_metrics.support))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                        });
                });
            
            ui.add_space(5.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(240, 235, 220))
                .inner_margin(10.0)
                .rounding(5.0)
                .show(ui, |ui| {
                    ui.set_width(table_width);
                    
                    ui.label(egui::RichText::new("Confusion Matrix:")
                        .color(egui::Color32::BLACK)
                        .size(14.0)
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                    ui.add_space(5.0);
                    
                    egui::Grid::new(format!("cm_{}", eval.accuracy))
                        .striped(false)
                        .spacing([20.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("")
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new("Pred: High")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new("Pred: Low")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                            
                            ui.label(egui::RichText::new("True: High")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.confusion_matrix[1][1]))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.confusion_matrix[1][0]))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                            
                            ui.label(egui::RichText::new("True: Low")
                                .color(egui::Color32::BLACK)
                                .strong()
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.confusion_matrix[0][1]))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.confusion_matrix[0][0]))
                                .color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.end_row();
                        });
                });
            
            ui.add_space(5.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(240, 235, 220))
                .inner_margin(10.0)
                .rounding(5.0)
                .show(ui, |ui| {
                    ui.set_width(table_width);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(format!("Accuracy: {:.1}%", eval.accuracy * 100.0))
                            .color(egui::Color32::BLACK)
                            .size(16.0)
                            .strong()
                            .family(egui::FontFamily::Name("Poppins".into())));
                    });
                });
        });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1250.0, 900.0])
            .with_title("CNN"),
        ..Default::default()
    };
    
    eframe::run_native(
        "CNN",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(TrainingGUI::new()))
        }),
    )
}

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