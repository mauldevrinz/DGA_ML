//! SVM LOOO Training GUI - Pure Rust Implementation (No Gnuplot)
//! File: src/bin/train_gui.rs

use coffee_classifier::ml::*;
use coffee_classifier::ml::evaluation::EvaluationResults;
use coffee_classifier::power_monitor::PowerMonitor;
use coffee_classifier::report::{TrainingReport, generate_loo_pdf, generate_loo_summary_pdf, LooRoc, LooFold, EpochPoint};
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



#[derive(Serialize)]
#[allow(dead_code)]
struct MetricsLog {
    epochs: Vec<usize>,
    train_loss: Vec<f32>,
    train_accuracy: Vec<f32>,
    val_loss: Vec<f32>,
    val_accuracy: Vec<f32>,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    selected_test_origin: Option<String>,
    current_fold_origin: Arc<Mutex<Option<(usize, usize, String)>>>,
    #[allow(dead_code)]
    predict_result: Arc<Mutex<Option<String>>>,
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
            selected_test_origin: None,
            current_fold_origin: Arc::new(Mutex::new(None)),
            predict_result: Arc::new(Mutex::new(None)),
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
    
#[allow(dead_code)]
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
        let current_fold_origin = Arc::clone(&self.current_fold_origin);

        thread::spawn(move || {
            println!(
                "\n{}",
                "╔═══════════════════════════════════════════════════╗"
                    .bold()
                    .cyan()
            );
            println!(
                "{}",
                "║      Coffee Arabica SVM Classifier           ║"
                    .bold()
                    .cyan()
            );
            println!(
                "{}",
                "║    SVM Leave-One-Origin-Out Testing    ║"
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

            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 1/7: Loading Dataset (LOOO)".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());

            // Load dataset + groups SEKALI (dipakai semua fold)
            let dataset = match FeatureLoader::new("data/features").load_raw() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("❌ Gagal load TSFRESH features: {}", e);
                    eprintln!("   Jalankan dulu: python tools/python/tsfresh_pipeline.py");
                    return;
                }
            };
            let groups = match GroupInfo::load("data/features", &dataset.sample_ids) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("❌ Gagal load groups.csv: {}", e);
                    eprintln!("   Jalankan ulang: python tools/python/tsfresh_pipeline.py");
                    return;
                }
            };

            let _n_features = dataset.features.ncols();

            // ── LOOO: loop SEMUA origin otomatis (12 fold) ──
            // Akumulator metrik per-fold + prediksi gabungan untuk ROC/AUC
            let mut fold_rows: Vec<(String, f32, f32, usize, usize, usize, usize, usize, usize, usize, usize, f64)> = Vec::new(); // origin, test_acc, train_acc, test_cm[4], train_cm[4]
            let mut all_scores: Vec<f32> = Vec::new();
            let mut all_labels: Vec<i64> = Vec::new();
            let origins_to_run = groups.unique_origins.clone();
            let n_folds = origins_to_run.len();

            let mut last_train_eval: Option<EvaluationResults> = None;

            // ── Bersihkan file kurva LOOO lama agar laporan PDF tidak memakai
            //    data run sebelumnya (mis. jumlah epoch/fold berbeda). ──
            std::fs::create_dir_all("models").ok();
            if let Ok(rd) = std::fs::read_dir("models") {
                for entry in rd.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("loo_svm_curve_") && name.ends_with(".csv") {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }

            for (fold_idx, test_origin) in origins_to_run.iter().enumerate() {
            let test_origin = test_origin.clone();
            let fold_start = std::time::Instant::now();
            // Beritahu UI fold mana yang sedang jalan (untuk centang bergerak)
            if let Ok(mut cur) = current_fold_origin.lock() {
                *cur = Some((fold_idx + 1, n_folds, test_origin.clone()));
            }
            // Reset grafik tiap fold
            if let Ok(mut history) = metrics_history.lock() { history.clear(); }

            let load_start = Instant::now();
            let split = loo_split(&dataset, &groups, &test_origin);
            let load_time = load_start.elapsed();

            println!("\n📊 LOOO Fold {}/{} — Test origin: {}", fold_idx + 1, n_folds, test_origin);
            let _ = load_time;
            println!("   Train samples: {} (11 origin)", split.train_labels.len());
            println!("   Test samples : {} (1 origin)", split.test_labels.len());
            println!("   ⏱️  Loading time: {:.2}s", load_time.as_secs_f32());

            println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
            println!("{}", " Phase 2/4: SVM on TSFRESH features".bold().cyan());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());

            // SVM pakai fitur RAW (SVM melakukan z-score sendiri di dalam fit).
            let train_svm = &split.train_raw;
            let val_svm = &split.test_raw;
            let train_labels = split.train_labels.clone();
            let val_labels = split.test_labels.clone();

            let svm_config = SVMConfig {
                c: 1.0,
                learning_rate: 0.01,
                n_epochs: 100,
                batch_size: 32,
            };
            println!("   Config: C={}, lr={}, epochs={}", svm_config.c, svm_config.learning_rate, svm_config.n_epochs);

            let training_start = Instant::now();
            let mut model = CoffeeSVM::new(svm_config);
            let live = tx.clone();
            let (_fit_result, accuracy_curve) = match model.fit_with_curve_features(
                train_svm,
                &train_labels,
                val_svm,
                &val_labels,
                |done, total, train_loss, train_acc, val_loss, val_acc| {
                    let m = TrainingMetrics {
                        train_loss, train_accuracy: train_acc,
                        val_loss, val_accuracy: val_acc, is_final: true,
                    };
                    let _ = live.send(m);
                    if done % 20 == 0 || done == total {
                        println!("   📈 Epoch: {}/{}", done, total);
                    }
                }
            ) {
                Ok(r) => r,
                Err(e) => { eprintln!("❌ Training error: {}", e); return; }
            };

            let training_time = training_start.elapsed();

            // Bangun metrics_history dari kurva akurasi (SVM tak punya loss curve terpisah).
            let mut final_metrics: Vec<TrainingMetrics> = Vec::new();
            for (ep_idx, &(_, tr_acc, va_acc)) in accuracy_curve.iter().enumerate() {
                final_metrics.push(TrainingMetrics {
                    train_loss: 0.0, train_accuracy: tr_acc,
                    val_loss: 0.0, val_accuracy: va_acc, is_final: true,
                });
                let _ = ep_idx;
            }
            if let Ok(mut history) = metrics_history.lock() {
                *history = final_metrics.clone();
            }

            // Simpan kurva per-epoch fold ini
            {
                use std::io::Write;
                let curve_path = format!("models/loo_svm_curve_{}.csv", fold_idx);
                if let Ok(mut cf) = std::fs::File::create(&curve_path) {
                    let _ = writeln!(cf, "epoch,train_acc,val_acc,train_loss,val_loss");
                    for (ep, m) in final_metrics.iter().enumerate() {
                        let _ = writeln!(cf, "{},{:.6},{:.6},{:.6},{:.6}",
                            ep + 1, m.train_accuracy, m.val_accuracy, 0.0, 0.0);
                    }
                }
            }

            println!("\n🏆 Training Summary:");
            println!("   Total epochs: {}", final_metrics.len());
            println!("   ⏱️  Training time: {:.2}s", training_time.as_secs_f32());

            let train_preds = model.predict_features(train_svm);
            let train_eval = Evaluator::evaluate(&train_preds, &train_labels);
            Evaluator::print_results(&train_eval, "Train");

            let val_preds = model.predict_features(val_svm);
            let val_eval = Evaluator::evaluate(&val_preds, &val_labels);
            Evaluator::print_results(&val_eval, &format!("Fold {}", test_origin));

            // SVM skor P(high) via decision function (sigmoid) untuk ROC
            let val_conf = model.predict_with_confidence_features(val_svm);

            // Akumulasi prediksi (in-memory) untuk ROC/AUC agregat
            for i in 0..val_conf.len() {
                all_scores.push(val_conf[i].1); // p_high (elemen ke-1 dari tuple)
                all_labels.push(val_labels[i]);
            }
            // Simpan metrik per-fold lengkap: origin, test_acc, train_acc,
            // test confusion (4) + train confusion (4)
            let cm = val_eval.confusion_matrix;
            let tcm = train_eval.confusion_matrix;
            fold_rows.push((
                test_origin.clone(),
                val_eval.accuracy as f32,
                train_eval.accuracy as f32,
                cm[0][0], cm[0][1], cm[1][0], cm[1][1],
                tcm[0][0], tcm[0][1], tcm[1][0], tcm[1][1],
                fold_start.elapsed().as_secs_f64(),
            ));

            // Catat eval fold (grafik tetap update via metrics_history live).
            // JANGAN kirim eval_tx di sini — biar tidak pindah ke halaman hasil tiap fold.
            last_train_eval = Some(train_eval.clone());

            // Simpan model fold terakhir saja (opsional)
            std::fs::create_dir_all("models").ok();
            model.save("models/loo_svm_model.json").ok();

            } // ── akhir loop 12 fold ──

            // ── OPSI 3: Latih SVM FULL pakai SEMUA 12 origin (untuk prediksi) ──
            {
                println!("\n🔧 Melatih model SVM FULL (12 origin) untuk prediksi...");
                let full_cfg = SVMConfig { c: 1.0, learning_rate: 0.01, n_epochs: 100, batch_size: 32 };
                let mut full_model = CoffeeSVM::new(full_cfg);
                let noop = |_:usize,_:usize,_:f32,_:f32,_:f32,_:f32| {};
                match full_model.fit_with_curve_features(&dataset.features, &dataset.labels, &dataset.features, &dataset.labels, noop) {
                    Ok(_) => {
                        std::fs::create_dir_all("models").ok();
                        full_model.save("models/loo_svm_full_model.json").ok();
                        println!("✅ Model SVM FULL tersimpan (loo_svm_full_model.json).");
                    }
                    Err(e) => eprintln!("❌ Gagal latih SVM FULL: {}", e),
                }
            }

            // ── Tulis semua prediksi + metrik fold ke file untuk PDF agregat ──
            {
                use std::io::Write;
                if let Ok(mut f) = std::fs::File::create("models/loo_svm_predictions.csv") {
                    let _ = writeln!(f, "p_high,label");
                    for (s, l) in all_scores.iter().zip(all_labels.iter()) {
                        let _ = writeln!(f, "{:.6},{}", s, l);
                    }
                }
                if let Ok(mut f) = std::fs::File::create("models/loo_svm_folds.csv") {
                    let _ = writeln!(f, "origin,test_acc,train_acc,cm00,cm01,cm10,cm11,tcm00,tcm01,tcm10,tcm11,secs");
                    for (o, ta, tra, c00, c01, c10, c11, t00, t01, t10, t11, secs) in &fold_rows {
                        let _ = writeln!(f, "{},{:.6},{:.6},{},{},{},{},{},{},{},{},{:.3}", o, ta, tra, c00, c01, c10, c11, t00, t01, t10, t11, secs);
                    }
                }
            }

            let total_time = total_start.elapsed();
            println!("\n✅ LOOO selesai: {} fold dalam {:.1}s", n_folds, total_time.as_secs_f32());

            // Tandai selesai ke UI
            if let Ok(mut cur) = current_fold_origin.lock() {
                *cur = None;
            }

            // ── Bangun eval AGREGAT dari semua 240 prediksi (kedua kelas) ──
            // Ini memperbaiki "class kebalik": fold terakhir hanya 1 kelas,
            // sedangkan agregat punya high & low lengkap.
            let agg_preds: Array1<i64> = Array1::from(
                all_scores.iter().map(|&s| if s >= 0.5 { 1 } else { 0 }).collect::<Vec<i64>>()
            );
            let agg_labels_arr: Array1<i64> = Array1::from(all_labels.clone());
            let agg_eval = Evaluator::evaluate(&agg_preds, &agg_labels_arr);
            // train_eval: pakai fold terakhir (representatif kualitas training)
            let train_eval_final = last_train_eval.unwrap_or_else(|| agg_eval.clone());

            let _ = eval_tx.send((train_eval_final, agg_eval));
            println!("\n✅ LOOO completed successfully!");
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
        let grade_prefix = if is_high_grade { "high_grade" } else { "low_grade" };
        let folders = if is_high_grade {
            &self.high_grade_folders
        } else {
            &self.low_grade_folders
        };

        // Origin yang sedang/sudah diuji (dari thread)
        let running = self.current_fold_origin.lock().ok().and_then(|c| c.clone());

        for f in folders.iter() {
            let origin_id = format!("{}/{}", grade_prefix, f.name);
            let is_running = running.as_ref().map(|(_, _, o)| o == &origin_id).unwrap_or(false);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let (mark, color) = if is_running {
                    ("▶", egui::Color32::from_rgb(255, 140, 0))   // sedang jalan
                } else {
                    ("○", egui::Color32::from_rgb(120, 120, 120))
                };
                ui.label(egui::RichText::new(mark).size(14.0).color(color));
                ui.label(egui::RichText::new(format!("{}  ({} file)", f.name, f.files.len()))
                    .size(14.0)
                    .color(if is_running { egui::Color32::from_rgb(200, 80, 0) } else { egui::Color32::BLACK })
                    .family(egui::FontFamily::Name(if is_running { "PoppinsBold" } else { "Poppins" }.into())));
                if is_running {
                    if let Some((i, n, _)) = &running {
                        ui.label(egui::RichText::new(format!("← TEST (fold {}/{})", i, n))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(255, 140, 0))
                            .family(egui::FontFamily::Name("PoppinsBold".into())));
                    }
                }
            });
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
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(0, 90, 160)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("Leave-One-Origin-Out for SVM")
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
                        ui.label(egui::RichText::new("1 arabica coffee sample for TEST — the others (11) for TRAIN")
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
                                ui.label(egui::RichText::new("Testing")
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
                                .color(egui::Color32::from_rgb(0, 90, 160))
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
                            self.draw_eval_tables(ui, "Testing", val_eval);
                        });
                    });
                    ui.add_space(15.0);

                    // ── ROC + AUC (agregat dari semua fold terkumpul) ──
                    Self::draw_roc_auc_panel(ui);
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
                                                .color(egui::Color32::from_rgb(0, 90, 160))
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
                    // START TRAINING Button — terkunci sampai 1 origin test dipilih
                    let origin_selected = true; // LOOO otomatis 12 fold, tak perlu pilih
                    let button_size = egui::vec2(200.0, 40.0);
                    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
                    
                    let (fill_color, icon, text) = if self.is_training {
                        (egui::Color32::from_rgb(255, 140, 0), "⏳", "RUNNING LOOO...")
                    } else {
                        (egui::Color32::from_rgb(40, 130, 50), "▶", "START LOOO (12 fold)")
                    };
                    let fill_color = if response.hovered() && !self.is_training && origin_selected {
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
                    // Hanya jalan kalau origin sudah dipilih
                    if response.clicked() && !self.is_training && origin_selected { self.start_training(); }

                    ui.add_space(15.0);



                    // Predict Button
                    let (pred_rect, pred_resp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                    let pred_fill = if pred_resp.hovered() { egui::Color32::from_rgb(220,120,0) } else { egui::Color32::from_rgb(200,100,0) };
                    ui.painter().rect_filled(pred_rect, 4.0, pred_fill);
                    let pt = ui.painter().layout_no_wrap("🔍 Predict".into(), egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(pred_rect.center().x - pt.size().x/2.0, pred_rect.center().y - pt.size().y/2.0), pt, egui::Color32::WHITE);
                    if pred_resp.clicked() {
                        launch_predict_binary("predict_loo_svm_gui");
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

                        // PDF Report button — terkunci selama masih training
                        ui.add_space(10.0);
                        let pdf_ready = self.training_complete && !self.is_training;
                        let pdf_size = egui::vec2(170.0, 40.0);
                        let (pdf_rect, pdf_resp) = ui.allocate_exact_size(pdf_size, egui::Sense::click());
                        let pdf_fill = if !pdf_ready {
                            egui::Color32::from_rgb(150, 150, 150)
                        } else if pdf_resp.hovered() {
                            egui::Color32::from_rgb(50, 120, 200)
                        } else {
                            egui::Color32::from_rgb(20, 80, 160)
                        };
                        ui.painter().rect_filled(pdf_rect, 4.0, pdf_fill);
                        let pdf_label = if pdf_ready { "📄 PDF Report" } else { "🔒 Tunggu selesai" };
                        let pdf_t = ui.painter().layout_no_wrap(
                            pdf_label.to_string(),
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
                        if pdf_resp.clicked() && pdf_ready {
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
        // Baca data 12 fold dari file
        let mut folds: Vec<LooFold> = Vec::new();
        if let Ok(content) = std::fs::read_to_string("models/loo_svm_folds.csv") {
            for (li, line) in content.lines().enumerate() {
                if li == 0 || line.trim().is_empty() { continue; }
                let c: Vec<&str> = line.split(',').collect();
                if c.len() < 11 { continue; }
                let pi = |i: usize| -> usize { c[i].parse().unwrap_or(0) };
                let fold_idx = folds.len();

                // Baca kurva per-epoch fold ini
                let mut curve: Vec<EpochPoint> = Vec::new();
                let mut final_train_loss = 0.0f32;
                let mut final_val_loss = 0.0f32;
                let curve_path = format!("models/loo_svm_curve_{}.csv", fold_idx);
                if let Ok(cc) = std::fs::read_to_string(&curve_path) {
                    for (cli, cline) in cc.lines().enumerate() {
                        if cli == 0 || cline.trim().is_empty() { continue; }
                        let cv: Vec<&str> = cline.split(',').collect();
                        if cv.len() < 5 { continue; }
                        let ep = EpochPoint {
                            epoch: cv[0].parse().unwrap_or(0),
                            train_acc: cv[1].parse().unwrap_or(0.0),
                            val_acc: cv[2].parse().unwrap_or(0.0),
                            train_loss: cv[3].parse().unwrap_or(0.0),
                            val_loss: cv[4].parse().unwrap_or(0.0),
                        };
                        final_train_loss = ep.train_loss;
                        final_val_loss = ep.val_loss;
                        curve.push(ep);
                    }
                }

                folds.push(LooFold {
                    origin: c[0].to_string(),
                    test_acc: c[1].parse().unwrap_or(0.0),
                    train_acc: c[2].parse().unwrap_or(0.0),
                    cm:       [[pi(3), pi(4)], [pi(5), pi(6)]],
                    train_cm: [[pi(7), pi(8)], [pi(9), pi(10)]],
                    curve,
                    final_train_loss,
                    final_val_loss,
                    secs: c.get(11).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                });
            }
        }
        if folds.is_empty() {
            self.pdf_saved = Some("Error: Jalankan LOOO sampai selesai dulu".to_string());
            return;
        }

        // ROC agregat + confusion agregat
        let loo_roc = Self::build_loo_roc();
        let agg_eval = Self::build_agg_eval();
        let total_secs = self.final_time.unwrap_or(0.0);

        match generate_loo_summary_pdf("SVM", &folds, &loo_roc, &agg_eval, total_secs, "testing_results") {
            Ok(path) => self.pdf_saved = Some(format!("PDF: {}", path)),
            Err(e)   => self.pdf_saved = Some(format!("Error: {}", e)),
        }
    }

    /// Confusion matrix agregat dari semua prediksi (240 sampel).
    fn build_agg_eval() -> EvaluationResults {
        let mut preds: Vec<i64> = Vec::new();
        let mut labels: Vec<i64> = Vec::new();
        if let Ok(content) = std::fs::read_to_string("models/loo_svm_predictions.csv") {
            for (li, line) in content.lines().enumerate() {
                if li == 0 || line.trim().is_empty() { continue; }
                let c: Vec<&str> = line.split(',').collect();
                if c.len() < 2 { continue; }
                if let (Ok(p), Ok(l)) = (c[0].parse::<f32>(), c[1].parse::<i64>()) {
                    preds.push(if p >= 0.5 { 1 } else { 0 });
                    labels.push(l);
                }
            }
        }
        let preds_arr = Array1::from(preds);
        let labels_arr = Array1::from(labels);
        Evaluator::evaluate(&preds_arr, &labels_arr)
    }

    #[allow(dead_code)]
    fn save_pdf_old(&mut self) {
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

            // Hitung ROC/AUC agregat dari prediksi terkumpul
            let loo_roc = Self::build_loo_roc();
            match generate_loo_pdf(&report, &loo_roc, "testing_results") {
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
    
    /// Baca akumulasi prediksi LOOO, hitung ROC+AUC untuk PDF.
    fn build_loo_roc() -> LooRoc {
        let path = "models/loo_svm_predictions.csv";
        let mut scores: Vec<f32> = Vec::new();
        let mut labels: Vec<i64> = Vec::new();
        if let Ok(content) = std::fs::read_to_string(path) {
            for (li, line) in content.lines().enumerate() {
                if li == 0 || line.trim().is_empty() { continue; }
                let cols: Vec<&str> = line.split(',').collect();
                if cols.len() < 2 { continue; }
                if let (Ok(p), Ok(l)) = (cols[0].parse::<f32>(), cols[1].parse::<i64>()) {
                    scores.push(p);
                    labels.push(l);
                }
            }
        }
        // n_origins dari folds file
        let mut n_origins = 0;
        if let Ok(content) = std::fs::read_to_string("models/loo_svm_folds.csv") {
            n_origins = content.lines().skip(1).filter(|l| !l.trim().is_empty()).count();
        }
        let roc = compute_roc_auc(&scores, &labels);
        LooRoc {
            points: roc.points.iter().map(|p| (p.fpr, p.tpr)).collect(),
            auc: roc.auc,
            n_origins,
            n_high: labels.iter().filter(|&&l| l == 1).count(),
            n_low:  labels.iter().filter(|&&l| l == 0).count(),
            scores: scores.clone(),
            labels: labels.clone(),
        }
    }

    /// Baca akumulasi prediksi LOOO, hitung ROC+AUC agregat, gambar kurva.
    fn draw_roc_auc_panel(ui: &mut egui::Ui) {
        let path = "models/loo_svm_predictions.csv";
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut scores: Vec<f32> = Vec::new();
        let mut labels: Vec<i64> = Vec::new();
        for (li, line) in content.lines().enumerate() {
            if li == 0 || line.trim().is_empty() { continue; }
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 2 { continue; }
            if let (Ok(p), Ok(l)) = (cols[0].parse::<f32>(), cols[1].parse::<i64>()) {
                scores.push(p);
                labels.push(l);
            }
        }
        if scores.is_empty() { return; }

        let roc = compute_roc_auc(&scores, &labels);
        let interp = auc_interpretation(roc.auc);
        let n_pos = labels.iter().filter(|&&l| l == 1).count();
        let n_neg = labels.iter().filter(|&&l| l == 0).count();

        ui.horizontal(|ui| {
            ui.add_space(20.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(245, 248, 252))
                .inner_margin(15.0)
                .rounding(5.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 210, 220)))
                .show(ui, |ui| {
                    ui.set_width(1180.0);
                    ui.label(egui::RichText::new("ROC Curve & AUC (agregat lintas fold)")
                        .size(16.0).color(egui::Color32::from_rgb(0, 90, 160))
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                    ui.label(egui::RichText::new(format!(
                        "AUC = {:.3}  ({})   •   {} high / {} low (agregat semua fold)",
                        roc.auc, interp, n_pos, n_neg))
                        .size(13.0).color(egui::Color32::BLACK)
                        .family(egui::FontFamily::Name("Poppins".into())));

                    if n_pos == 0 || n_neg == 0 {
                        ui.label(egui::RichText::new(
                            "⚠️ Perlu minimal 1 origin high DAN 1 origin low agar ROC/AUC valid.")
                            .size(12.0).color(egui::Color32::from_rgb(180, 80, 0)));
                        return;
                    }

                    ui.add_space(6.0);
                    let roc_pts: PlotPoints = roc.points.iter()
                        .map(|p| [p.fpr as f64, p.tpr as f64]).collect();
                    let diag: PlotPoints = vec![[0.0, 0.0], [1.0, 1.0]].into_iter().collect();
                    Plot::new("loo_roc")
                        .legend(Legend::default().position(Corner::RightBottom))
                        .show_axes([true, true]).show_grid([true, true])
                        .allow_drag(false).allow_zoom(false).allow_scroll(false)
                        .x_axis_label("False Positive Rate").y_axis_label("True Positive Rate")
                        .height(280.0).width(380.0)
                        .include_x(0.0).include_x(1.0).include_y(0.0).include_y(1.0)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(diag)
                                .color(egui::Color32::GRAY).width(1.0).name("Random (0.5)"));
                            plot_ui.line(Line::new(roc_pts)
                                .color(egui::Color32::from_rgb(0, 90, 200)).width(2.5)
                                .name(&format!("ROC (AUC={:.3})", roc.auc)));
                        });
                });
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

/// OPSI 3 — Prediksi 1 FOLDER (≈20 CSV) memakai model SVM FULL hasil LOOO.
/// SVM menormalkan fitur secara internal, jadi tidak perlu file norm terpisah.
#[allow(dead_code)]
fn run_folder_prediction_svm(result_slot: std::sync::Arc<std::sync::Mutex<Option<String>>>) {
    let folder = match rfd::FileDialog::new()
        .set_title("Pilih folder berisi 20 pengukuran (CSV) satu sampel")
        .pick_folder()
    {
        Some(p) => p,
        None => return,
    };
    if let Ok(mut s) = result_slot.lock() { *s = Some("⏳ Memproses...".to_string()); }

    let model = match CoffeeSVM::load("models/loo_svm_full_model.json") {
        Ok(m) => m,
        Err(_) => {
            if let Ok(mut s) = result_slot.lock() {
                *s = Some("❌ Model belum ada. Jalankan START LOOO dulu.".to_string());
            }
            return;
        }
    };

    let mut csvs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&folder) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x.eq_ignore_ascii_case("csv")).unwrap_or(false) { csvs.push(p); }
        }
    }
    csvs.sort();
    if csvs.is_empty() {
        if let Ok(mut s) = result_slot.lock() { *s = Some("❌ Tidak ada file CSV di folder itu.".to_string()); }
        return;
    }

    let mut n_high = 0usize; let mut n_low = 0usize;
    let mut conf_high_sum = 0.0f32; let mut conf_low_sum = 0.0f32; let mut n_ok = 0usize;
    for path in &csvs {
        let pstr = path.to_string_lossy().to_string();
        let extracted = match extract_features_via_python(&pstr) { Ok(e) => e, Err(_) => continue };
        let res = model.predict_with_confidence_features(&extracted.features);
        if let Some(&(class, p_high, p_low)) = res.first() {
            if class == 1 { n_high += 1; conf_high_sum += p_high; }
            else { n_low += 1; conf_low_sum += p_low; }
            n_ok += 1;
        }
    }
    if n_ok == 0 {
        if let Ok(mut s) = result_slot.lock() { *s = Some("❌ Semua file gagal diekstrak (cek Python/tsfresh).".to_string()); }
        return;
    }
    let (label, conf) = if n_high >= n_low {
        ("High Grade", if n_high > 0 { conf_high_sum / n_high as f32 } else { 0.0 })
    } else {
        ("Low Grade", if n_low > 0 { conf_low_sum / n_low as f32 } else { 0.0 })
    };
    let winner = n_high.max(n_low);
    let msg = format!("{}  —  {}/{} suara,  confidence {:.1}%", label, winner, n_ok, conf * 100.0);
    if let Ok(mut s) = result_slot.lock() { *s = Some(msg); }
}

/// Jalankan binary predict LOOO (window terpisah, desain seperti predict 80/20).
fn launch_predict_binary(bin_name: &str) {
    let bin_name = bin_name.to_string();
    std::thread::spawn(move || {
        let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
        let possible_paths = [
            format!("./target/release/{}", bin_name),
            format!("target/release/{}",   bin_name),
            format!("./target/debug/{}",   bin_name),
            format!("target/debug/{}",     bin_name),
        ];
        let binary_path = possible_paths.iter()
            .find(|p| std::path::Path::new(p.as_str()).exists())
            .cloned()
            .or_else(|| std::env::current_exe().ok()
                .and_then(|e| e.parent().map(|d| d.join(&bin_name).to_string_lossy().into_owned())));
        match binary_path {
            Some(path) => {
                if let Err(e) = std::process::Command::new(&path)
                    .env("DISPLAY", display)
                    .env("LIBGL_ALWAYS_SOFTWARE", "1")
                    .env("GDK_BACKEND", "x11")
                    .spawn()
                { eprintln!("[X] Gagal buka {}: {}", bin_name, e); }
            }
            None => eprintln!("[X] Binary tidak ditemukan: {}", bin_name),
        }
    });
}

fn main() -> eframe::Result<()> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1250.0, 900.0])
            .with_title("SVM LOOO Training"),
        ..Default::default()
    };
    
    eframe::run_native(
        "SVM LOOO Training",
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