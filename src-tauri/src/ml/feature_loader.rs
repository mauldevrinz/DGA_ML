// src/ml/feature_loader.rs
// FeatureLoader: Load fitur hasil TSFRESH dari data/features/features_final.csv
// Mendukung z-score anti-leakage (normalizer di-fit HANYA pada train set).

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use ndarray::{Array1, Array2};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

// ─── Tipe Data ────────────────────────────────────────────────────────────────

/// Dataset fitur TSFRESH MENTAH (belum dinormalisasi).
#[derive(Debug, Clone)]
pub struct FeatureDataset {
    pub features: Array2<f32>,           // (n_samples, n_features) mentah
    pub labels: Array1<i64>,             // 0=low, 1=high
    pub feature_names: Vec<String>,
    pub sample_ids: Vec<i64>,            // id tiap baris (untuk LOOO grouping)
}

/// Statistik z-score, di-fit pada train set, dipakai transform train & val.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureNormStats {
    pub mean: Vec<f32>,
    pub std: Vec<f32>,
}

impl FeatureNormStats {
    /// Fit mean & std HANYA pada data yang diberikan (harus train set).
    pub fn fit(features: &Array2<f32>) -> Self {
        let n_samples  = features.nrows();
        let n_features = features.ncols();
        let mut mean = vec![0.0f32; n_features];
        let mut std  = vec![1.0f32; n_features];
        for j in 0..n_features {
            let col: Vec<f32> = (0..n_samples).map(|i| features[[i, j]]).collect();
            let m = col.iter().sum::<f32>() / n_samples as f32;
            let v = col.iter().map(|x| (x - m).powi(2)).sum::<f32>() / n_samples as f32;
            mean[j] = m;
            std[j]  = (v + 1e-8).sqrt();
        }
        Self { mean, std }
    }

    /// Terapkan z-score ke matriks fitur apa pun.
    pub fn transform(&self, features: &Array2<f32>) -> Array2<f32> {
        let mut out = features.clone();
        for i in 0..features.nrows() {
            for j in 0..features.ncols() {
                out[[i, j]] = (features[[i, j]] - self.mean[j]) / self.std[j];
            }
        }
        out
    }

    pub fn save(&self, path: &str) -> Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }
}

#[derive(Debug, Deserialize)]
struct FeatureMeta {
    pub n_features: usize,
    pub feature_names: Vec<String>,
}

// ─── FeatureLoader ────────────────────────────────────────────────────────────

pub struct FeatureLoader {
    features_dir: PathBuf,
}

impl FeatureLoader {
    pub fn new(features_dir: impl AsRef<Path>) -> Self {
        Self { features_dir: features_dir.as_ref().to_path_buf() }
    }

    /// Load fitur MENTAH (tanpa normalisasi).
    pub fn load_raw(&self) -> Result<FeatureDataset> {
        println!("📂 Loading TSFRESH features dari {:?}", self.features_dir);
        let meta = self.load_feature_meta()?;
        println!("   Fitur terdaftar : {}", meta.n_features);
        let (features, sample_ids) = self.load_features_csv(&meta.feature_names)?;
        println!("   Sampel dimuat   : {}", features.nrows());
        let label_map = self.load_labels_csv()?;
        let labels    = self.align_labels(&sample_ids, &label_map)?;
        println!("   High grade (1)  : {}", labels.iter().filter(|&&l| l == 1).count());
        println!("   Low grade  (0)  : {}", labels.iter().filter(|&&l| l == 0).count());
        Ok(FeatureDataset { features, labels, feature_names: meta.feature_names, sample_ids })
    }

    fn load_feature_meta(&self) -> Result<FeatureMeta> {
        let path = self.features_dir.join("selected_feature_names.json");
        let file = File::open(&path).with_context(|| format!("Tidak bisa buka: {:?}", path))?;
        Ok(serde_json::from_reader(BufReader::new(file))
            .context("Gagal parse selected_feature_names.json")?)
    }

    fn load_features_csv(&self, expected_cols: &[String]) -> Result<(Array2<f32>, Vec<i64>)> {
        let path = self.features_dir.join("features_final.csv");
        let file = File::open(&path).with_context(|| format!("Tidak bisa buka: {:?}", path))?;
        let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(BufReader::new(file));
        let headers = rdr.headers()?.clone();
        let id_col_idx = headers.iter().position(|h| h == "id").unwrap_or(0);
        let col_idx_map: HashMap<&str, usize> =
            headers.iter().enumerate().map(|(i, h)| (h, i)).collect();
        for name in expected_cols {
            if !col_idx_map.contains_key(name.as_str()) {
                anyhow::bail!("Kolom '{}' tidak ada di features_final.csv", name);
            }
        }
        let feat_indices: Vec<usize> = expected_cols.iter()
            .map(|n| *col_idx_map.get(n.as_str()).unwrap()).collect();
        let mut rows: Vec<Vec<f32>> = Vec::new();
        let mut sample_ids: Vec<i64> = Vec::new();
        for result in rdr.records() {
            let record = result.context("Gagal baca baris CSV")?;
            let id: i64 = record.get(id_col_idx)
                .and_then(|s| s.parse().ok()).unwrap_or(rows.len() as i64);
            sample_ids.push(id);
            let row: Vec<f32> = feat_indices.iter()
                .map(|&idx| record.get(idx).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0))
                .collect();
            rows.push(row);
        }
        let n_samples  = rows.len();
        let n_features = expected_cols.len();
        let flat: Vec<f32> = rows.into_iter().flatten().collect();
        let arr = Array2::from_shape_vec((n_samples, n_features), flat)
            .context("Gagal membentuk Array2")?;
        Ok((arr, sample_ids))
    }

    fn load_labels_csv(&self) -> Result<HashMap<i64, i64>> {
        let path = self.features_dir.join("labels.csv");
        let file = File::open(&path).with_context(|| format!("Tidak bisa buka: {:?}", path))?;
        let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(BufReader::new(file));
        let mut map = HashMap::new();
        for result in rdr.records() {
            let record = result?;
            let id: i64    = record.get(0).and_then(|s| s.parse().ok()).context("id?")?;
            let label: i64 = record.get(1).and_then(|s| s.parse().ok()).context("label?")?;
            map.insert(id, label);
        }
        Ok(map)
    }

    fn align_labels(&self, sample_ids: &[i64], label_map: &HashMap<i64, i64>) -> Result<Array1<i64>> {
        let mut labels = Vec::with_capacity(sample_ids.len());
        for &id in sample_ids {
            let label = label_map.get(&id)
                .with_context(|| format!("Label tidak ditemukan untuk id={}", id))?;
            labels.push(*label);
        }
        Ok(Array1::from(labels))
    }
}

// ─── Train/Val Split (anti data leakage) ──────────────────────────────────────

pub struct FeatureSplit {
    pub train_features: Array2<f32>,    // dinormalisasi (fit di train)
    pub train_labels:   Array1<i64>,
    pub val_features:   Array2<f32>,     // dinormalisasi pakai stats train
    pub val_labels:     Array1<i64>,
    pub norm_stats:     FeatureNormStats,
}

/// Stratified split + z-score anti-leakage.
/// seed=None → acak; seed=Some(x) → reproducible.
pub fn stratified_split(
    dataset: &FeatureDataset,
    val_ratio: f32,
    seed: Option<u64>,
) -> FeatureSplit {
    let mut rng: StdRng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None     => StdRng::from_entropy(),
    };

    let mut idx_high: Vec<usize> = dataset.labels.iter().enumerate()
        .filter(|(_, &l)| l == 1).map(|(i, _)| i).collect();
    let mut idx_low: Vec<usize> = dataset.labels.iter().enumerate()
        .filter(|(_, &l)| l == 0).map(|(i, _)| i).collect();

    idx_high.shuffle(&mut rng);
    idx_low.shuffle(&mut rng);

    let n_val_high = ((idx_high.len() as f32) * val_ratio).round() as usize;
    let n_val_low  = ((idx_low.len()  as f32) * val_ratio).round() as usize;

    let val_idx: Vec<usize> = idx_high[..n_val_high].iter()
        .chain(idx_low[..n_val_low].iter()).cloned().collect();
    let train_idx: Vec<usize> = idx_high[n_val_high..].iter()
        .chain(idx_low[n_val_low..].iter()).cloned().collect();

    let select = |indices: &[usize]| -> (Array2<f32>, Array1<i64>) {
        let n  = indices.len();
        let nf = dataset.features.ncols();
        let mut f = Array2::<f32>::zeros((n, nf));
        let mut l = Array1::<i64>::zeros(n);
        for (row, &idx) in indices.iter().enumerate() {
            f.row_mut(row).assign(&dataset.features.row(idx));
            l[row] = dataset.labels[idx];
        }
        (f, l)
    };

    let (train_raw, train_labels) = select(&train_idx);
    let (val_raw,   val_labels)   = select(&val_idx);

    // Fit normalizer DI TRAIN SAJA, lalu transform keduanya (anti-leakage)
    let norm_stats = FeatureNormStats::fit(&train_raw);
    let train_features = norm_stats.transform(&train_raw);
    let val_features   = norm_stats.transform(&val_raw);

    FeatureSplit { train_features, train_labels, val_features, val_labels, norm_stats }
}

// ─── Split MENTAH (untuk model yang normalisasi internal: SVM, MLP) ──────────

pub struct FeatureSplitRaw {
    pub train_features: Array2<f32>,    // MENTAH (belum dinormalisasi)
    pub train_labels:   Array1<i64>,
    pub val_features:   Array2<f32>,     // MENTAH
    pub val_labels:     Array1<i64>,
}

/// Stratified split TANPA normalisasi — model (SVM/MLP) normalisasi sendiri.
pub fn stratified_split_raw(
    dataset: &FeatureDataset,
    val_ratio: f32,
    seed: Option<u64>,
) -> FeatureSplitRaw {
    let mut rng: StdRng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None     => StdRng::from_entropy(),
    };

    let mut idx_high: Vec<usize> = dataset.labels.iter().enumerate()
        .filter(|(_, &l)| l == 1).map(|(i, _)| i).collect();
    let mut idx_low: Vec<usize> = dataset.labels.iter().enumerate()
        .filter(|(_, &l)| l == 0).map(|(i, _)| i).collect();

    idx_high.shuffle(&mut rng);
    idx_low.shuffle(&mut rng);

    let n_val_high = ((idx_high.len() as f32) * val_ratio).round() as usize;
    let n_val_low  = ((idx_low.len()  as f32) * val_ratio).round() as usize;

    let val_idx: Vec<usize> = idx_high[..n_val_high].iter()
        .chain(idx_low[..n_val_low].iter()).cloned().collect();
    let train_idx: Vec<usize> = idx_high[n_val_high..].iter()
        .chain(idx_low[n_val_low..].iter()).cloned().collect();

    let select = |indices: &[usize]| -> (Array2<f32>, Array1<i64>) {
        let n  = indices.len();
        let nf = dataset.features.ncols();
        let mut f = Array2::<f32>::zeros((n, nf));
        let mut l = Array1::<i64>::zeros(n);
        for (row, &idx) in indices.iter().enumerate() {
            f.row_mut(row).assign(&dataset.features.row(idx));
            l[row] = dataset.labels[idx];
        }
        (f, l)
    };

    let (train_features, train_labels) = select(&train_idx);
    let (val_features,   val_labels)   = select(&val_idx);

    FeatureSplitRaw { train_features, train_labels, val_features, val_labels }
}