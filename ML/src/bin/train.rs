// src/bin/train.rs
// Training RF + SVM + MLP dengan fitur TSFRESH + Stratified K-Fold (k=5)
// Non-GUI version. CNN/LSTM tidak di sini (pakai GUI dengan deret waktu mentah).

use coffee_classifier::ml::feature_loader::{FeatureLoader, FeatureNormStats};
use coffee_classifier::ml::{CoffeeRandomForest, RandomForestConfig};
use coffee_classifier::ml::{CoffeeSVM, SVMConfig};
use coffee_classifier::ml::{CoffeeMLP, MLPConfig};
use anyhow::Result;
use colored::*;
use ndarray::{Array1, Array2, Axis};
use std::time::Instant;

const K_FOLDS:      usize = 5;
const FEATURES_DIR: &str  = "data/features";
const MODELS_DIR:   &str  = "models";

fn main() -> Result<()> {
    println!("\n{}", "╔══════════════════════════════════════════════════╗".bold().cyan());
    println!("{}", "║   Coffee Arabica — TSFRESH + K-Fold Training    ║".bold().cyan());
    println!("{}", "╚══════════════════════════════════════════════════╝".bold().cyan());

    std::fs::create_dir_all(MODELS_DIR)?;
    let total_start = Instant::now();

    // ── 1. Load fitur TSFRESH MENTAH ──────────────────────────────────────────
    println!("\n{}", "─── Phase 1: Load TSFRESH Features ─────────────────".cyan());
    let dataset = FeatureLoader::new(FEATURES_DIR).load_raw()?;
    let n_samples  = dataset.features.nrows();
    let n_features = dataset.features.ncols();
    println!("  Sampel   : {}", n_samples);
    println!("  Fitur    : {}", n_features);

    // ── 2. Stratified K-Fold ──────────────────────────────────────────────────
    println!("\n{}", "─── Phase 2: Stratified K-Fold (k=5) ───────────────".cyan());
    let folds = make_stratified_kfold(&dataset.labels, K_FOLDS);

    let mut rf_accs:  Vec<f32> = Vec::new();
    let mut svm_accs: Vec<f32> = Vec::new();
    let mut mlp_accs: Vec<f32> = Vec::new();

    for fold in 0..K_FOLDS {
        println!("\n  Fold {}/{}", fold + 1, K_FOLDS);
        let (val_idx, train_idx) = &folds[fold];

        // Fitur MENTAH per fold
        let train_raw = dataset.features.select(Axis(0), train_idx);
        let train_l   = select_labels(&dataset.labels, train_idx);
        let val_raw   = dataset.features.select(Axis(0), val_idx);
        let val_l     = select_labels(&dataset.labels, val_idx);

        // Normalisasi anti-leakage: fit di train fold saja
        let norm = FeatureNormStats::fit(&train_raw);
        let train_f = norm.transform(&train_raw);
        let val_f   = norm.transform(&val_raw);

        // ── Random Forest (pakai fitur ternormalisasi) ──
        let t0 = Instant::now();
        let mut rf = CoffeeRandomForest::new(RandomForestConfig {
            n_trees: 100, max_depth: 10, min_samples_split: 4,
            max_features: 0, bootstrap_fraction: 0.8,
        });
        rf.train(&train_f, &train_l)?;
        let rf_acc = rf.evaluate(&val_f, &val_l);
        rf_accs.push(rf_acc);
        println!("    RF  val_acc={:.4}  ({:.1}s)", rf_acc, t0.elapsed().as_secs_f32());

        // ── SVM (normalisasi internal sendiri → pakai fitur MENTAH) ──
        let t0 = Instant::now();
        let svm_acc = train_eval_svm(&train_raw, &train_l, &val_raw, &val_l)?;
        svm_accs.push(svm_acc);
        println!("    SVM val_acc={:.4}  ({:.1}s)", svm_acc, t0.elapsed().as_secs_f32());

        // ── MLP (normalisasi internal sendiri → pakai fitur MENTAH) ──
        let t0 = Instant::now();
        let mlp_acc = train_eval_mlp(&train_raw, &train_l, &val_raw, &val_l)?;
        mlp_accs.push(mlp_acc);
        println!("    MLP val_acc={:.4}  ({:.1}s)", mlp_acc, t0.elapsed().as_secs_f32());
    }

    // ── 3. Ringkasan ──────────────────────────────────────────────────────────
    println!("\n{}", "─── Phase 3: Ringkasan K-Fold ───────────────────────".cyan());
    print_summary("Random Forest", &rf_accs);
    print_summary("SVM          ", &svm_accs);
    print_summary("MLP          ", &mlp_accs);

    // ── 4. Train final di seluruh data & simpan ───────────────────────────────
    println!("\n{}", "─── Phase 4: Train Final Model (all data) ───────────".cyan());
    let norm_all = FeatureNormStats::fit(&dataset.features);
    let feat_all = norm_all.transform(&dataset.features);

    let mut rf_final = CoffeeRandomForest::new(RandomForestConfig {
        n_trees: 100, max_depth: 10, min_samples_split: 4,
        max_features: 0, bootstrap_fraction: 0.8,
    });
    rf_final.train(&feat_all, &dataset.labels)?;
    rf_final.save(&format!("{}/rf_tsfresh_model.json", MODELS_DIR))?;
    norm_all.save(&format!("{}/rf_tsfresh_norm.json", MODELS_DIR))?;
    println!("  RF  → {}/rf_tsfresh_model.json", MODELS_DIR);

    println!("\n  Total waktu: {:.1}s", total_start.elapsed().as_secs_f32());
    println!("{}", "  Training selesai!".bold().green());
    Ok(())
}

// ─── SVM (fitur mentah, normalisasi internal) ─────────────────────────────────

fn train_eval_svm(
    train_f: &Array2<f32>, train_l: &Array1<i64>,
    val_f:   &Array2<f32>, val_l:   &Array1<i64>,
) -> Result<f32> {
    let mut svm = CoffeeSVM::new(SVMConfig::default());
    svm.fit_features(train_f, train_l, |_, _, _, _, _, _| {})?;
    let preds = svm.predict_features(val_f);
    let n = val_f.nrows();
    if n == 0 { return Ok(0.0); }
    Ok(preds.iter().zip(val_l.iter()).filter(|(&p, &l)| p == l).count() as f32 / n as f32)
}

// ─── MLP (fitur mentah, normalisasi internal) ─────────────────────────────────

fn train_eval_mlp(
    train_f: &Array2<f32>, train_l: &Array1<i64>,
    val_f:   &Array2<f32>, val_l:   &Array1<i64>,
) -> Result<f32> {
    let mut mlp = CoffeeMLP::new(MLPConfig {
        hidden1: 128, hidden2: 64, n_epochs: 50,
        learning_rate: 0.001, batch_size: 32, clip_grad: 1.0,
        disable_early_stop: false,
    });
    mlp.fit_features(train_f, train_l, val_f, val_l, |_, _, _, _, _, _| {})?;
    let preds = mlp.predict_features(val_f);
    let n = val_f.nrows();
    if n == 0 { return Ok(0.0); }
    Ok(preds.iter().zip(val_l.iter()).filter(|(&p, &l)| p == l).count() as f32 / n as f32)
}

// ─── K-Fold helpers ──────────────────────────────────────────────────────────

fn make_stratified_kfold(labels: &Array1<i64>, k: usize) -> Vec<(Vec<usize>, Vec<usize>)> {
    let idx_h: Vec<usize> = labels.iter().enumerate().filter(|(_, &l)| l == 1).map(|(i, _)| i).collect();
    let idx_l: Vec<usize> = labels.iter().enumerate().filter(|(_, &l)| l == 0).map(|(i, _)| i).collect();
    let ch = split_chunks(&idx_h, k);
    let cl = split_chunks(&idx_l, k);
    (0..k).map(|f| {
        let val:   Vec<usize> = ch[f].iter().chain(cl[f].iter()).cloned().collect();
        let train: Vec<usize> = (0..k).filter(|&x| x != f)
            .flat_map(|x| ch[x].iter().chain(cl[x].iter()).cloned()).collect();
        (val, train)
    }).collect()
}

fn split_chunks(idx: &[usize], k: usize) -> Vec<Vec<usize>> {
    let n = idx.len(); let base = n / k; let extra = n % k;
    let mut out = Vec::new(); let mut start = 0;
    for i in 0..k {
        let end = start + base + if i < extra { 1 } else { 0 };
        out.push(idx[start..end].to_vec()); start = end;
    }
    out
}

fn select_labels(labels: &Array1<i64>, idx: &[usize]) -> Array1<i64> {
    Array1::from(idx.iter().map(|&i| labels[i]).collect::<Vec<_>>())
}

fn print_summary(name: &str, accs: &[f32]) {
    let mean = accs.iter().sum::<f32>() / accs.len() as f32;
    let std  = (accs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / accs.len() as f32).sqrt();
    println!("  {} | mean={:.4} ± {:.4} | [{}]", name, mean, std,
        accs.iter().map(|a| format!("{:.3}", a)).collect::<Vec<_>>().join(", "));
}