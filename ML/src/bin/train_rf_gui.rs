//! Random Forest Training GUI
//! File: src/bin/train_rf_gui.rs
//! Layout konsisten dengan train_gui.rs (CNN Training)

use coffee_classifier::ml::*;
use coffee_classifier::ml::evaluation::EvaluationResults;
use coffee_classifier::power_monitor::PowerMonitor;
use eframe::egui;
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints, Legend, Corner};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use colored::*;
use std::path::PathBuf;

// ─────────────────────────────────────────────
// Folder tree node (sama persis dengan train_gui.rs)
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct FolderNode {
    name: String,
    files: Vec<String>,
    expanded: bool,
}

// ─────────────────────────────────────────────
// Result data dari training thread
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct RFResult {
    train_eval: EvaluationResults,
    val_eval: EvaluationResults,
    train_accuracy: f32,
    val_accuracy: f32,
    n_trees: usize,
    feature_importance: Vec<(String, f32)>,
    training_secs: f64,
    /// (n_trees_checkpoint, train_acc, val_acc) — untuk kurva akurasi vs N trees
    accuracy_curve: Vec<(usize, f32, f32)>,
}

// ─────────────────────────────────────────────
// Main GUI struct
// ─────────────────────────────────────────────

use image as image_crate;
use coffee_classifier::report::{TrainingReport, generate_training_pdf};

struct RFTrainingGUI {
    is_training: bool,
    training_complete: bool,
    start_time: Option<Instant>,
    final_time: Option<f64>,
    result_receiver: Option<Receiver<RFResult>>,
    status_receiver: Option<Receiver<String>>,
    rf_result: Option<RFResult>,
    status_log: Arc<Mutex<Vec<String>>>,
    // Progress: (trees_done, total_trees) updated atomically by training thread
    trees_done: Arc<AtomicUsize>,
    power_monitor: Option<PowerMonitor>,
    training_start_sample: usize,
    screenshot_requested: bool,
    screenshot_saved: Option<String>,
    pdf_saved: Option<String>,
    total_trees: usize,

    // Data file panel
    high_grade_folders: Vec<FolderNode>,
    low_grade_folders: Vec<FolderNode>,
    high_grade_expanded: bool,
    low_grade_expanded: bool,
    last_refresh: Instant,
    refresh_interval: Duration,
}

impl Default for RFTrainingGUI {
    fn default() -> Self {
        let mut gui = Self {
            is_training: false,
            training_complete: false,
            start_time: None,
            final_time: None,
            result_receiver: None,
            status_receiver: None,
            rf_result: None,
            status_log: Arc::new(Mutex::new(Vec::new())),
            trees_done: Arc::new(AtomicUsize::new(0)),
            power_monitor: Some(PowerMonitor::start()),
            training_start_sample: 0,
            screenshot_requested: false,
            screenshot_saved: None,
            pdf_saved: None,
            total_trees: 100,
            high_grade_folders: Vec::new(),
            low_grade_folders: Vec::new(),
            high_grade_expanded: false,
            low_grade_expanded: false,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(3),
        };
        gui.load_data_files();
        gui
    }
}

impl RFTrainingGUI {
    fn new() -> Self {
        Self::default()
    }

    fn auto_refresh_data(&mut self) {
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.load_data_files();
            self.last_refresh = Instant::now();
        }
    }

    fn load_data_files(&mut self) {
        let base = PathBuf::from("data/raw");
        self.high_grade_folders = Self::load_folder_nodes(&base.join("high_grade"));
        self.low_grade_folders  = Self::load_folder_nodes(&base.join("low_grade"));
    }

    fn load_folder_nodes(base: &PathBuf) -> Vec<FolderNode> {
        let mut nodes = Vec::new();
        let Ok(entries) = std::fs::read_dir(base) else { return nodes };
        let mut dirs: Vec<_> = entries.flatten().collect();
        dirs.sort_by_key(|e| e.file_name());
        for entry in dirs {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let mut files: Vec<String> = std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| {
                        e.path().extension().map(|ext| ext == "csv").unwrap_or(false)
                    })
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                files.sort();
                nodes.push(FolderNode { name, files, expanded: false });
            }
        }
        nodes
    }

    fn open_file_explorer() {
        let path = PathBuf::from("data/raw");
        #[cfg(target_os = "linux")]
        { let _ = std::process::Command::new("xdg-open").arg(&path).spawn(); }
        #[cfg(target_os = "windows")]
        { let _ = std::process::Command::new("explorer").arg(&path).spawn(); }
        #[cfg(target_os = "macos")]
        { let _ = std::process::Command::new("open").arg(&path).spawn(); }
    }

    fn push_status(log: &Arc<Mutex<Vec<String>>>, msg: &str) {
        if let Ok(mut v) = log.lock() {
            println!("{}", msg);
            v.push(msg.to_string());
        }
    }

    // ─── Start training in background thread ───────────────────
    fn start_training(&mut self) {
        if self.is_training { return; }

        self.is_training = true;
        self.training_complete = false;
        self.start_time = Some(Instant::now());
        self.final_time = None;
        self.rf_result = None;
        self.trees_done.store(0, Ordering::Relaxed);
        let needs_new_monitor = self.power_monitor.as_ref().map(|pm| pm.is_stopped()).unwrap_or(true);
        if needs_new_monitor {
            self.power_monitor = Some(PowerMonitor::start());
            self.training_start_sample = 0;
        } else {
            self.training_start_sample = self.power_monitor.as_ref().map(|pm| pm.sample_count()).unwrap_or(0);
        }
        self.total_trees = 100; // must match rf_config.n_trees below

        if let Ok(mut log) = self.status_log.lock() { log.clear(); }

        let (result_tx, result_rx) = channel();
        let (status_tx, status_rx) = channel();
        self.result_receiver = Some(result_rx);
        self.status_receiver = Some(status_rx);
        let log = Arc::clone(&self.status_log);
        let trees_done = Arc::clone(&self.trees_done);

        thread::spawn(move || {
            let push = |msg: &str| {
                let _ = status_tx.send(msg.to_string());
                Self::push_status(&log, msg);
            };

            push(&format!(
                "\n{}",
                "╔═══════════════════════════════════════════════════╗"
                    .bold().green()
            ));
            push(&format!(
                "{}",
                "║   Coffee Arabica - Random Forest Classifier      ║"
                    .bold().green()
            ));
            push(&format!(
                "{}",
                "╚═══════════════════════════════════════════════════╝"
                    .bold().green()
            ));

            let total_start = Instant::now();

            // ── Phase 1: Load TSFRESH Features ──────────────────
            push(&format!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".green()));
            push(&format!("{}", " Phase 1/4: Loading TSFRESH Features".bold().green()));

            let dataset = match FeatureLoader::new("data/features").load_raw() {
                Ok(d) => d,
                Err(e) => {
                    push(&format!("❌ Gagal load features: {}", e));
                    push("   Pastikan sudah jalankan: python tools/python/tsfresh_pipeline.py");
                    return;
                }
            };

            // Stratified split + z-score anti-leakage (fit di train saja)
            let split = stratified_split(&dataset, 0.2, Some(42));
            let train_rf = &split.train_features;   // Array2 (n × n_features) ternormalisasi
            let val_rf   = &split.val_features;
            let train_labels = &split.train_labels;
            let val_labels   = &split.val_labels;

            push(&format!("   Total fitur   : {}", dataset.features.ncols()));
            push(&format!("   Train samples : {}", train_labels.len()));
            push(&format!("   Val   samples : {}", val_labels.len()));
            let t_high = train_labels.iter().filter(|&&x| x == 1).count();
            let t_low  = train_labels.iter().filter(|&&x| x == 0).count();
            let v_high = val_labels.iter().filter(|&&x| x == 1).count();
            let v_low  = val_labels.iter().filter(|&&x| x == 0).count();
            push(&format!("   Train: {} High, {} Low", t_high, t_low));
            push(&format!("   Val:   {} High, {} Low", v_high, v_low));

            // ── Phase 2: (Z-score sudah dilakukan di split) ─────
            push(&format!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".green()));
            push(&format!("{}", " Phase 2/4: Normalisasi (z-score, fit di train)".bold().green()));
            push("   ✅ Fitur ternormalisasi (anti data leakage)");

            // ── Phase 3: Train RF ───────────────────────────────
            push(&format!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".green()));
            push(&format!("{}", " Phase 3/4: Training Random Forest".bold().green()));

            let rf_config = RandomForestConfig {
                n_trees: 100,
                max_depth: 10,
                min_samples_split: 4,
                max_features: 0,     // auto = sqrt(n_features)
                bootstrap_fraction: 0.8,
            };
            push(&format!("   Config: {} trees, max_depth={}, bootstrap={:.0}%",
                rf_config.n_trees, rf_config.max_depth,
                rf_config.bootstrap_fraction * 100.0));

            let train_start = Instant::now();
            let mut rf = CoffeeRandomForest::new(rf_config);
            let fit_result = match rf.train(train_rf, train_labels) {
                Ok(r) => r,
                Err(e) => { push(&format!("❌ Training error: {}", e)); return; }
            };
            trees_done.store(rf.n_trees(), Ordering::Relaxed);
            let training_secs = train_start.elapsed().as_secs_f64();

            push(&format!("   ✅ {} trees trained in {:.2}s",
                fit_result.n_trees_built, training_secs));
            push(&format!("   Train accuracy: {:.2}%", fit_result.train_accuracy * 100.0));

            // ── Phase 4: Evaluate & Save ───────────────────────
            push(&format!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".green()));
            push(&format!("{}", " Phase 4/4: Evaluasi & Simpan Model".bold().green()));

            // Train eval
            let train_preds = rf.predict_labels_features(train_rf);
            let train_eval = Evaluator::evaluate(&train_preds, train_labels);
            Evaluator::print_results(&train_eval, "Train");

            // Val eval
            let val_preds = rf.predict_labels_features(val_rf);
            let val_eval = Evaluator::evaluate(&val_preds, val_labels);
            Evaluator::print_results(&val_eval, "Validation");

            // Feature importance labels — pakai nama fitur TSFRESH asli
            let mut importance_labeled: Vec<(String, f32)> = fit_result.feature_importance
                .iter()
                .enumerate()
                .map(|(idx, &score)| {
                    let name = dataset.feature_names.get(idx)
                        .cloned()
                        .unwrap_or_else(|| format!("feature_{}", idx));
                    (name, score)
                })
                .collect();
            importance_labeled.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // ── Accuracy curve vs N trees ────────────────────────
            push("   📈 Menghitung kurva akurasi vs N trees...");
            let n_total = rf.n_trees();
            let checkpoints: Vec<usize> = (1..=100).filter_map(|i| {
                if (i - 1) % 5 == 0 || i == 100 { Some(((n_total * i) / 100).max(1)) } else { None }
            }).collect();
            let accuracy_curve: Vec<(usize, f32, f32)> = checkpoints.iter().map(|&n| {
                let ta = rf.evaluate_at_n_trees_features(train_rf, train_labels, n);
                let va = rf.evaluate_at_n_trees_features(val_rf, val_labels, n);
                (n, ta, va)
            }).collect();

            // Save model + normalizer
            std::fs::create_dir_all("models").ok();
            rf.save("models/rf_model.json").ok();
            split.norm_stats.save("models/rf_norm_stats.json").ok();
            push("   ✅ Model disimpan: models/rf_model.json");
            push("   ✅ Normalizer    : models/rf_norm_stats.json");

            let val_correct = val_preds.iter().zip(val_labels.iter())
                .filter(|(p, l)| p == l).count();
            let val_accuracy = val_correct as f32 / val_labels.len() as f32;

            let total_time = total_start.elapsed();
            push(&format!(
                "\n✅ Training selesai! Total: {:.2}s ({:.2}min)",
                total_time.as_secs_f64(),
                total_time.as_secs_f64() / 60.0
            ));

            let _ = result_tx.send(RFResult {
                train_eval,
                val_eval,
                train_accuracy: fit_result.train_accuracy,
                val_accuracy,
                n_trees: fit_result.n_trees_built,
                feature_importance: importance_labeled,
                training_secs,
                accuracy_curve,
            });
        });
    }

    // ─── Poll receivers ────────────────────────────────────────
    fn update_state(&mut self) {
        // Poll status messages
        if let Some(ref rx) = self.status_receiver {
            while rx.try_recv().is_ok() {} // already printed; just drain
        }

        // Poll result
        if let Some(ref rx) = self.result_receiver {
            if let Ok(result) = rx.try_recv() {
                self.rf_result = Some(result);
                self.is_training = false;
                self.training_complete = true;
                if let Some(ref pm) = self.power_monitor { pm.stop(); }
                if let Some(start) = self.start_time {
                    self.final_time = Some(start.elapsed().as_secs_f64());
                }
            }
        }
    }

    /// Panel folder High/Low — DISAMAKAN PERSIS dengan CNN (train_gui.rs).
    /// Dua kolom WHITE lebar tetap 590px, tinggi 220px, judul di tengah,
    /// ScrollArea vertikal di dalamnya.
    fn draw_grade_panels_cnn(&mut self, ui: &mut egui::Ui) {
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
                            .id_source("cnnlike_high_scroll")
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
                            .id_source("cnnlike_low_scroll")
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
            if ui.button(format!("{} {} {} ({} folders)", arrow, folder_icon, root_name, total_folders)).clicked() {
                *expanded = !*expanded;
            }
        });

        if *expanded {
            for folder in folders.iter_mut() {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let fa = if folder.expanded { "▼" } else { "►" };
                    let fi = if folder.expanded { "📂" } else { "📁" };
                    if ui.button(format!("{} {} {} ({} files)", fa, fi, folder.name, folder.files.len())).clicked() {
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

    fn draw_feature_importance(&self, ui: &mut egui::Ui, result: &RFResult) {
        let top: Vec<_> = result.feature_importance.iter().take(10).collect();
        if top.is_empty() { return; }

        ui.label(egui::RichText::new("Top 10 Feature Importance")
            .size(14.0)
            .color(egui::Color32::BLACK)
            .family(egui::FontFamily::Name("PoppinsBold".into())));
        ui.add_space(6.0);

        // egui_plot BarChart — horizontal bars (nilai di sumbu X, fitur di sumbu Y)
        let max_score = top.first().map(|(_, s)| *s).unwrap_or(1.0).max(1e-6) as f64;
        let bars: Vec<Bar> = top.iter().enumerate().map(|(i, (_, score))| {
            Bar::new(i as f64, (*score as f64 / max_score) * 100.0)
                .fill(egui::Color32::from_rgb(34, 139, 34))
                .width(0.6)
        }).collect();

        let _plot_resp = Plot::new("rf_feature_importance")
            .height(220.0)
            .width(ui.available_width())
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_axes([true, true])
            .show_grid([true, false])
            .include_x(0.0)
            .include_y(top.len() as f64)
            .legend(Legend::default())
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars).name("Importance (%)"));
            });

        ui.add_space(4.0);
        for (i, (label, score)) in top.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("[{}]", i))
                    .size(11.0).color(egui::Color32::from_rgb(80, 80, 80)).family(egui::FontFamily::Monospace));
                ui.label(egui::RichText::new(format!("{:<22} {:.2}%", label, score * 100.0))
                    .size(11.0).color(egui::Color32::from_rgb(40, 40, 40)).family(egui::FontFamily::Monospace));
            });
        }
    }

    fn draw_accuracy_plot(&self, ui: &mut egui::Ui, result: &RFResult) {
        if result.accuracy_curve.is_empty() { return; }

        ui.label(egui::RichText::new("Accuracy vs N Trees")
            .size(14.0)
            .color(egui::Color32::BLACK)
            .family(egui::FontFamily::Name("PoppinsBold".into())));
        ui.add_space(6.0);

        let train_points: PlotPoints = result.accuracy_curve.iter()
            .map(|(n, ta, _)| [*n as f64, (*ta * 100.0) as f64])
            .collect();
        let val_points: PlotPoints = result.accuracy_curve.iter()
            .map(|(n, _, va)| [*n as f64, (*va * 100.0) as f64])
            .collect();

        let max_n = result.n_trees as f64;
        Plot::new("rf_accuracy_curve")
            .legend(Legend::default().position(Corner::RightBottom))
            .show_axes([true, true])
            .show_grid([true, true])
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .x_axis_label("N Trees")
            .y_axis_label("Accuracy (%)")
            .height(200.0)
            .width(ui.available_width())
            .include_x(0.0)
            .include_x(max_n)
            .include_y(0.0)
            .include_y(100.0)
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(train_points)
                    .color(egui::Color32::from_rgb(255, 140, 0))
                    .width(2.5)
                    .name("Train Accuracy"));
                plot_ui.line(Line::new(val_points)
                    .color(egui::Color32::from_rgb(51, 102, 255))
                    .width(2.5)
                    .name("Val Accuracy"));
            });
    }

    fn draw_eval_tables(&self, ui: &mut egui::Ui, id_prefix: &str, eval: &EvaluationResults) {

        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(240, 235, 220))
                .inner_margin(10.0)
                .rounding(5.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    ui.label(egui::RichText::new("Per-Class Metrics:")
                        .color(egui::Color32::BLACK)
                        .size(14.0)
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                    ui.add_space(5.0);

                    egui::Grid::new(format!("{}_rf_metrics", id_prefix))
                        .striped(false)
                        .spacing([20.0, 5.0])
                        .show(ui, |ui| {
                            for header in ["Metric", "High Grade", "Low Grade"] {
                                ui.label(egui::RichText::new(header)
                                    .color(egui::Color32::BLACK)
                                    .strong()
                                    .family(egui::FontFamily::Name("Poppins".into())));
                            }
                            ui.end_row();

                            let rows = [
                                ("Precision", eval.high_grade_metrics.precision, eval.low_grade_metrics.precision),
                                ("Recall",    eval.high_grade_metrics.recall,    eval.low_grade_metrics.recall),
                                ("F1-Score",  eval.high_grade_metrics.f1_score,  eval.low_grade_metrics.f1_score),
                            ];
                            for (name, high, low) in &rows {
                                ui.label(egui::RichText::new(*name).color(egui::Color32::BLACK)
                                    .family(egui::FontFamily::Name("Poppins".into())));
                                ui.label(egui::RichText::new(format!("{:.3}", high)).color(egui::Color32::BLACK)
                                    .family(egui::FontFamily::Name("Poppins".into())));
                                ui.label(egui::RichText::new(format!("{:.3}", low)).color(egui::Color32::BLACK)
                                    .family(egui::FontFamily::Name("Poppins".into())));
                                ui.end_row();
                            }
                            ui.label(egui::RichText::new("Support").color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.high_grade_metrics.support)).color(egui::Color32::BLACK)
                                .family(egui::FontFamily::Name("Poppins".into())));
                            ui.label(egui::RichText::new(format!("{}", eval.low_grade_metrics.support)).color(egui::Color32::BLACK)
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
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Confusion Matrix:")
                        .color(egui::Color32::BLACK)
                        .size(14.0)
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                    ui.add_space(5.0);

                    egui::Grid::new(format!("{}_rf_cm", id_prefix))
                        .striped(false)
                        .spacing([20.0, 5.0])
                        .show(ui, |ui| {
                            for header in ["", "Pred: High", "Pred: Low"] {
                                ui.label(egui::RichText::new(header).color(egui::Color32::BLACK)
                                    .strong().family(egui::FontFamily::Name("Poppins".into())));
                            }
                            ui.end_row();

                            // Konvensi high=1, low=0: petakan posisi tampilan → index array
                            for (row_label, row_idx) in [("True: High", 1usize), ("True: Low", 0usize)] {
                                ui.label(egui::RichText::new(row_label).color(egui::Color32::BLACK)
                                    .strong().family(egui::FontFamily::Name("Poppins".into())));
                                for col_idx in [1usize, 0usize] {
                                    ui.label(egui::RichText::new(format!("{}", eval.confusion_matrix[row_idx][col_idx]))
                                        .color(egui::Color32::BLACK)
                                        .family(egui::FontFamily::Name("Poppins".into())));
                                }
                                ui.end_row();
                            }
                        });
                });

            ui.add_space(5.0);

            egui::Frame::none()
                .fill(egui::Color32::from_rgb(240, 235, 220))
                .inner_margin(10.0)
                .rounding(5.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
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

    fn save_screenshot(&mut self) {
        self.screenshot_requested = true;
    }

    fn save_pdf(&mut self) {
        if let Some(ref result) = self.rf_result {
            let report = TrainingReport::RandomForest {
                train_eval:         result.train_eval.clone(),
                val_eval:           result.val_eval.clone(),
                train_accuracy:     result.train_accuracy,
                val_accuracy:       result.val_accuracy,
                n_trees:            result.n_trees,
                training_secs:      result.training_secs,
                accuracy_curve:     result.accuracy_curve.clone(),
                power:              self.power_monitor.as_ref()
                                        .and_then(|pm| pm.summary_from(self.training_start_sample)),
            };
            match generate_training_pdf(&report, "testing_results") {
                Ok(path)  => self.pdf_saved = Some(format!("PDF: {}", path)),
                Err(e)    => self.pdf_saved = Some(format!("PDF Error: {}", e)),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────
// eframe::App implementation
// ─────────────────────────────────────────────────────────────


impl eframe::App for RFTrainingGUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_state();

        if !self.training_complete {
            self.auto_refresh_data();
        }

        ctx.request_repaint_after(Duration::from_millis(100));

        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = egui::Color32::from_rgb(225, 225, 225);
        visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::BLACK;
        ctx.set_visuals(visuals);

        // ── Handle screenshot ──
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs()).unwrap_or(0);
                    let path = format!("testing_results/rf_training_{}.png", ts);
                    let [w, h] = image.size;
                    let raw: Vec<u8> = image.pixels.iter()
                        .flat_map(|p| [p.r(), p.g(), p.b(), p.a()])
                        .collect();
                    let _ = std::fs::create_dir_all("testing_results");
                    match image_crate::save_buffer(&path, &raw, w as u32, h as u32, image_crate::ColorType::Rgba8) {
                        Ok(_) => self.screenshot_saved = Some(format!("Saved: {}", path)),
                        Err(e) => self.screenshot_saved = Some(format!("Error: {}", e)),
                    }
                }
            }
        });
        if self.screenshot_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            self.screenshot_requested = false;
        }

        // ── Header ─────────────────────────────────────────────

        egui::TopBottomPanel::top("rf_header")
            .exact_height(60.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(34, 139, 34)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("RANDOM FOREST")
                        .size(26.0)
                        .color(egui::Color32::WHITE)
                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                });
            });

        // ── Bottom bar (must be declared before CentralPanel) ──
        egui::TopBottomPanel::bottom("rf_bottom")
            .exact_height(70.0)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(225, 225, 225))
                .inner_margin(egui::Margin::symmetric(20.0, 15.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // START TRAINING button
                    let btn_size = egui::vec2(220.0, 40.0);
                    let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                    let (fill_color, icon, text) = if self.is_training {
                        (egui::Color32::from_rgb(255, 140, 0), "⏳", "TRAINING...")
                    } else {
                        (egui::Color32::from_rgb(34, 139, 34), "▶", "START TRAINING")
                    };
                    let fill_color = if response.hovered() && !self.is_training {
                        egui::Color32::from_rgb(40, 160, 40)
                    } else {
                        fill_color
                    };
                    ui.painter().rect_filled(rect, 4.0, fill_color);
                    let icon_g = ui.painter().layout_no_wrap(
                        icon.to_string(), egui::FontId::proportional(16.0), egui::Color32::WHITE);
                    let text_g = ui.painter().layout_no_wrap(
                        format!(" {}", text),
                        egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                        egui::Color32::WHITE);
                    let total_w = icon_g.size().x + text_g.size().x;
                    let sx = rect.center().x - total_w / 2.0;
                    let icon_h = icon_g.size().y;
                    let icon_w = icon_g.size().x;
                    let text_h = text_g.size().y;
                    ui.painter().galley(egui::pos2(sx, rect.center().y - icon_h / 2.0), icon_g, egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(sx + icon_w, rect.center().y - text_h / 2.0), text_g, egui::Color32::WHITE);
                    if response.clicked() && !self.is_training {
                        self.start_training();
                    }
                    if self.training_complete {
                        ui.add_space(15.0);
                        let dl_size = egui::vec2(170.0, 40.0);
                        let (dl_rect, dl_resp) = ui.allocate_exact_size(dl_size, egui::Sense::click());
                        let dl_fill = if dl_resp.hovered() {
                            egui::Color32::from_rgb(0, 110, 200)
                        } else {
                            egui::Color32::from_rgb(0, 90, 170)
                        };
                        ui.painter().rect_filled(dl_rect, 4.0, dl_fill);
                        let dl_t = ui.painter().layout_no_wrap(
                            "📷 Download".to_string(),
                            egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                            egui::Color32::WHITE,
                        );
                        let dl_tp = egui::pos2(
                            dl_rect.center().x - dl_t.size().x / 2.0,
                            dl_rect.center().y - dl_t.size().y / 2.0,
                        );
                        ui.painter().galley(dl_tp, dl_t, egui::Color32::WHITE);
                        if dl_resp.clicked() {
                            self.save_screenshot();
                        }
                        // ── PDF Report button ──
                        ui.add_space(10.0);
                        let pdf_size = egui::vec2(170.0, 40.0);
                        let (pdf_rect, pdf_resp) = ui.allocate_exact_size(pdf_size, egui::Sense::click());
                        let pdf_fill = if pdf_resp.hovered() {
                            egui::Color32::from_rgb(140, 20, 20)
                        } else {
                            egui::Color32::from_rgb(110, 0, 0)
                        };
                        ui.painter().rect_filled(pdf_rect, 4.0, pdf_fill);
                        let pdf_t = ui.painter().layout_no_wrap(
                            "📄 PDF Report".to_string(),
                            egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                            egui::Color32::WHITE,
                        );
                        let pdf_tp = egui::pos2(
                            pdf_rect.center().x - pdf_t.size().x / 2.0,
                            pdf_rect.center().y - pdf_t.size().y / 2.0,
                        );
                        ui.painter().galley(pdf_tp, pdf_t, egui::Color32::WHITE);
                        if pdf_resp.clicked() {
                            self.save_pdf();
                        }
                    }
                    ui.add_space(15.0);
                    let pred_size = egui::vec2(150.0, 40.0);
                    let (pred_rect, pred_resp) = ui.allocate_exact_size(pred_size, egui::Sense::click());
                    let pred_fill = if pred_resp.hovered() {
                        egui::Color32::from_rgb(220, 120, 0)
                    } else {
                        egui::Color32::from_rgb(200, 100, 0)
                    };
                    ui.painter().rect_filled(pred_rect, 4.0, pred_fill);
                    let pred_t = ui.painter().layout_no_wrap(
                        "🔍 Predict".to_string(),
                        egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                        egui::Color32::WHITE,
                    );
                    let pred_tp = egui::pos2(
                        pred_rect.center().x - pred_t.size().x / 2.0,
                        pred_rect.center().y - pred_t.size().y / 2.0,
                    );
                    ui.painter().galley(pred_tp, pred_t, egui::Color32::WHITE);
                    if pred_resp.clicked() {
                        std::thread::spawn(|| {
                            use std::process::Command;
                            let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
                            let bin_name = "predict_rf_gui";
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
                                    { eprintln!("[X] Failed to launch predict GUI: {}", e); }
                                }
                                None => eprintln!("[X] predict GUI binary not found: {}", bin_name),
                            }
                        });
                    }
                    ui.add_space(15.0);
                    // LOOO Button (biru tua) — luncurkan train_loo_rf_gui
                    let (loo_rect, loo_resp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                    let loo_fill = if loo_resp.hovered() { egui::Color32::from_rgb(20, 70, 110) } else { egui::Color32::from_rgb(15, 55, 90) };
                    ui.painter().rect_filled(loo_rect, 4.0, loo_fill);
                    let lt = ui.painter().layout_no_wrap("🔬 LOOO".into(), egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(loo_rect.center().x - lt.size().x/2.0, loo_rect.center().y - lt.size().y/2.0), lt, egui::Color32::WHITE);
                    if loo_resp.clicked() {
                        std::thread::spawn(|| {
                            use std::process::Command;
                            let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
                            let bin_name = "train_loo_rf_gui";
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
                                    { eprintln!("[X] Failed to launch LOOO RF GUI: {}", e); }
                                }
                                None => eprintln!("[X] LOOO RF GUI binary not found: {}", bin_name),
                            }
                        });
                    }
                    ui.add_space(15.0);
                    // Independent Button (oranye) — luncurkan independent_rf_gui
                    let (ind_rect, ind_resp) = ui.allocate_exact_size(egui::vec2(150.0, 40.0), egui::Sense::click());
                    let ind_fill = if ind_resp.hovered() { egui::Color32::from_rgb(215, 180, 45) } else { egui::Color32::from_rgb(200, 165, 30) };
                    ui.painter().rect_filled(ind_rect, 4.0, ind_fill);
                    let it = ui.painter().layout_no_wrap("🧪 Cross Day".into(), egui::FontId::new(14.0, egui::FontFamily::Name("PoppinsBold".into())), egui::Color32::WHITE);
                    ui.painter().galley(egui::pos2(ind_rect.center().x - it.size().x/2.0, ind_rect.center().y - it.size().y/2.0), it, egui::Color32::WHITE);
                    if ind_resp.clicked() {
                        std::thread::spawn(|| {
                            use std::process::Command;
                            let display = std::env::var("DISPLAY").unwrap_or(":0".to_string());
                            let bin_name = "independent_rf_gui";
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
                                    { eprintln!("[X] Failed to launch Independent RF GUI: {}", e); }
                                }
                                None => eprintln!("[X] Independent RF GUI binary not found: {}", bin_name),
                            }
                        });
                    }
                    ui.add_space(15.0);
                    // Manage Data button
                    if !self.training_complete {
                        let manage_size = egui::vec2(150.0, 40.0);
                        let (mrect, mresp) = ui.allocate_exact_size(manage_size, egui::Sense::click());
                        let mc = if mresp.hovered() {
                            egui::Color32::from_rgb(120, 20, 20)
                        } else {
                            egui::Color32::from_rgb(100, 0, 0)
                        };
                        ui.painter().rect_filled(mrect, 4.0, mc);
                        let mt = ui.painter().layout_no_wrap(
                            "Manage Data".to_string(),
                            egui::FontId::new(15.0, egui::FontFamily::Name("PoppinsBold".into())),
                            egui::Color32::WHITE);
                        let mtp = egui::pos2(
                            mrect.center().x - mt.size().x / 2.0,
                            mrect.center().y - mt.size().y / 2.0);
                        ui.painter().galley(mtp, mt, egui::Color32::WHITE);
                        if mresp.clicked() {
                            Self::open_file_explorer();
                        }
                    }
                    // Elapsed time (right-aligned)
                    if self.training_complete {
                        if let Some(ft) = self.final_time {
                            let ft32 = ft as f32;
                            let time_label = if ft32 < 1.0 {
                                format!("✅ {:.0} ms", ft32 * 1000.0)
                            } else if ft32 < 60.0 {
                                format!("✅ {:.2} s", ft32)
                            } else {
                                format!("✅ {:.2} min", ft32 / 60.0)
                            };
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if let Some(ref msg) = self.pdf_saved {
                                    ui.label(egui::RichText::new(msg.as_str())
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(120, 0, 0))
                                        .family(egui::FontFamily::Name("Poppins".into())));
                                    ui.add_space(10.0);
                                }
                                if let Some(ref msg) = self.screenshot_saved {
                                    ui.label(egui::RichText::new(msg.as_str())
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(0, 120, 0))
                                        .family(egui::FontFamily::Name("Poppins".into())));
                                    ui.add_space(10.0);
                                }
                                ui.label(egui::RichText::new(time_label)
                                    .size(15.0)
                                    .color(egui::Color32::from_rgb(34, 139, 34))
                                    .family(egui::FontFamily::Name("PoppinsBold".into())));
                            });
                        }
                    }
                });
            });

        // ── Central Panel ───────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(225, 225, 225))
                .inner_margin(egui::Margin::symmetric(15.0, 0.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_space(15.0);

                        // ── Data file panels ──
                        if !self.training_complete {
                            self.draw_grade_panels_cnn(ui);
                        }

                        // ── Status / Progress ──
                        if self.is_training {
                            egui::Frame::none()
                                .fill(egui::Color32::WHITE)
                                .inner_margin(15.0)
                                .rounding(5.0)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)))
                                .show(ui, |ui| {
                                    let elapsed = self.start_time
                                        .map(|t| t.elapsed().as_secs_f64())
                                        .unwrap_or(0.0);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("⏳ Training Random Forest, harap tunggu...")
                                            .size(16.0)
                                            .color(egui::Color32::from_rgb(34, 139, 34))
                                            .family(egui::FontFamily::Name("PoppinsBold".into())));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(format!("⏱ {:.1}s", elapsed))
                                                .size(14.0)
                                                .color(egui::Color32::from_rgb(100, 100, 100))
                                                .family(egui::FontFamily::Name("Poppins".into())));
                                        });
                                    });
                                    ui.add_space(8.0);
                                    let done = self.trees_done.load(Ordering::Relaxed);
                                    let total = self.total_trees.max(1);
                                    let progress = done as f32 / total as f32;
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("🌲 Trees:")
                                            .size(13.0)
                                            .color(egui::Color32::from_rgb(60, 60, 60))
                                            .family(egui::FontFamily::Name("Poppins".into())));
                                        let bar_w = (ui.available_width() - 80.0).max(100.0);
                                        ui.add(
                                            egui::ProgressBar::new(progress)
                                                .desired_width(bar_w)
                                                .show_percentage()
                                                .animate(true)
                                        );
                                        ui.label(egui::RichText::new(format!("{}/{}", done, total))
                                            .size(13.0)
                                            .color(egui::Color32::from_rgb(60, 60, 60))
                                            .family(egui::FontFamily::Name("Poppins".into())));
                                    });
                                    ui.add_space(8.0);
                                    let log = if let Ok(v) = self.status_log.lock() { v.clone() } else { vec![] };
                                    egui::ScrollArea::vertical()
                                        .max_height(150.0)
                                        .stick_to_bottom(true)
                                        .show(ui, |ui| {
                                            for line in &log {
                                                ui.label(egui::RichText::new(line)
                                                    .size(12.0)
                                                    .color(egui::Color32::from_rgb(60, 60, 60))
                                                    .family(egui::FontFamily::Monospace));
                                            }
                                        });
                                    // ── Power ──
                                    if let Some(ref pm) = self.power_monitor {
                                        if let Some(s) = pm.current_sample() {
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
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
                                                .color(egui::Color32::from_rgb(120, 30, 200))
                                                .family(egui::FontFamily::Name("Poppins".into())));
                                            });
                                        }
                                    }
                                });
                            ui.add_space(15.0);
                        }

                        // ── Results after training ──
                        if let Some(ref result) = self.rf_result.clone() {
                            // Summary bar (full width)
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(34, 139, 34))
                                .inner_margin(10.0)
                                .rounding(5.0)
                                .show(ui, |ui| {
                                    let summary = format!(
                                        "🌲 {} Trees  |  ⏱ {:.1}s  |  Train: {:.1}%  |  Val: {:.1}%  |  Model: models/rf_model.json",
                                        result.n_trees,
                                        result.training_secs,
                                        result.train_accuracy * 100.0,
                                        result.val_accuracy * 100.0,
                                    );
                                    ui.label(egui::RichText::new(summary)
                                        .size(14.0)
                                        .color(egui::Color32::WHITE)
                                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                                });
                            ui.add_space(10.0);

                            // ── Power Summary ──
                            if let Some(ref pm) = self.power_monitor {
                                if let Some(ps) = pm.summary_from(self.training_start_sample) {
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(245, 240, 255))
                                        .inner_margin(10.0)
                                        .rounding(5.0)
                                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 185, 230)))
                                        .show(ui, |ui| {
                                            ui.set_min_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("⚡ Power Summary")
                                                    .size(13.0)
                                                    .color(egui::Color32::from_rgb(100, 20, 180))
                                                    .family(egui::FontFamily::Name("PoppinsBold".into())));
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    ui.label(egui::RichText::new(format!("{} samples", ps.sample_count))
                                                        .size(11.0)
                                                        .color(egui::Color32::from_rgb(150, 150, 150))
                                                        .family(egui::FontFamily::Name("Poppins".into())));
                                                });
                                            });
                                            ui.add_space(5.0);
                                            ui.columns(4, |cols| {
                                                cols[0].vertical_centered(|ui| {
                                                    ui.label(egui::RichText::new("Avg System").size(11.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                    ui.label(egui::RichText::new(format!("{:.1} W", ps.avg_total_w)).size(15.0).color(egui::Color32::from_rgb(100,20,180)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                    ui.label(egui::RichText::new(format!("Peak {:.1} W", ps.peak_total_w)).size(11.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                });
                                                cols[1].vertical_centered(|ui| {
                                                    ui.label(egui::RichText::new("Avg CPU+GPU").size(11.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                    ui.label(egui::RichText::new(format!("{:.1} W", ps.avg_cpu_gpu_w)).size(15.0).color(egui::Color32::from_rgb(100,20,180)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                    ui.label(egui::RichText::new(format!("Peak {:.1} W", ps.peak_cpu_gpu_w)).size(11.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                });
                                                cols[2].vertical_centered(|ui| {
                                                    ui.label(egui::RichText::new("CPU/GPU Temp").size(11.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                    ui.label(egui::RichText::new(format!("{:.1}°C / {:.1}°C", ps.avg_cpu_temp, ps.avg_gpu_temp)).size(15.0).color(egui::Color32::from_rgb(100,20,180)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                    ui.label(egui::RichText::new(format!("Peak {:.1}°C / {:.1}°C", ps.peak_cpu_temp, ps.peak_gpu_temp)).size(11.0).color(egui::Color32::from_rgb(130,130,130)).family(egui::FontFamily::Name("Poppins".into())));
                                                });
                                                cols[3].vertical_centered(|ui| {
                                                    ui.label(egui::RichText::new("🔋 Total Energy").size(11.0).color(egui::Color32::from_rgb(100,20,180)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                    ui.label(egui::RichText::new(format!("{:.1} J", ps.energy_joules)).size(18.0).color(egui::Color32::from_rgb(100,20,180)).family(egui::FontFamily::Name("PoppinsBold".into())));
                                                    ui.label(egui::RichText::new(format!("{:.4} Wh", ps.energy_joules / 3600.0)).size(12.0).color(egui::Color32::from_rgb(100,20,180)).family(egui::FontFamily::Name("Poppins".into())));
                                                });
                                            });
                                        });
                                    ui.add_space(10.0);
                                }
                            }

                            // Row 1: Accuracy plot (left) + Feature importance (right)
                            ui.columns(2, |cols| {
                                egui::Frame::none()
                                    .fill(egui::Color32::WHITE)
                                    .inner_margin(15.0)
                                    .rounding(5.0)
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)))
                                    .show(&mut cols[0], |ui| {
                                        self.draw_accuracy_plot(ui, result);
                                    });
                                egui::Frame::none()
                                    .fill(egui::Color32::WHITE)
                                    .inner_margin(15.0)
                                    .rounding(5.0)
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)))
                                    .show(&mut cols[1], |ui| {
                                        self.draw_feature_importance(ui, result);
                                    });
                            });
                            ui.add_space(10.0);

                            // Row 2: Eval tables (train left, val right)
                            ui.columns(2, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Train")
                                        .size(14.0)
                                        .color(egui::Color32::BLACK)
                                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                                    ui.add_space(4.0);
                                    self.draw_eval_tables(ui, "train", &result.train_eval);
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("Validation")
                                        .size(14.0)
                                        .color(egui::Color32::BLACK)
                                        .family(egui::FontFamily::Name("PoppinsBold".into())));
                                    ui.add_space(4.0);
                                    self.draw_eval_tables(ui, "val", &result.val_eval);
                                });
                            });
                            ui.add_space(15.0);
                        }
                    });
            });
    }
}


// ─────────────────────────────────────────────────────────────
// main + setup_fonts (identik dengan train_gui.rs)
// ─────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1250.0, 900.0])
            .with_title("Random Forest"),
        ..Default::default()
    };

    eframe::run_native(
        "Random Forest",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(RFTrainingGUI::new()))
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