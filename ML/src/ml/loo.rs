//! Leave-One-Origin-Out (LOOO) untuk Electronic Nose.
//!
//! Memuat pemetaan id->origin dari data/features/groups.csv, lalu menyediakan
//! split LOOO: satu origin penuh dijadikan TEST, sisanya TRAIN.
//! Juga menyediakan perhitungan ROC curve & AUC dari probabilitas gabungan.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use ndarray::{Array1, Array2};

use crate::ml::feature_loader::{FeatureDataset, FeatureNormStats};

/// Info origin per sampel (sejajar urutan baris FeatureDataset).
#[derive(Debug, Clone)]
pub struct GroupInfo {
    /// origin[i] = "high_grade/sample1" untuk sampel ke-i
    pub origins: Vec<String>,
    /// daftar origin unik, terurut (12 origin)
    pub unique_origins: Vec<String>,
}

impl GroupInfo {
    /// Muat groups.csv (kolom: id,origin,grade,folder).
    /// `sample_ids` adalah urutan id pada FeatureDataset (baris ke-i = id apa).
    pub fn load(features_dir: impl AsRef<Path>, sample_ids: &[i64]) -> Result<Self> {
        let path = features_dir.as_ref().join("groups.csv");
        let file = File::open(&path)
            .with_context(|| format!("Tidak bisa buka {:?}. Jalankan ulang tsfresh_pipeline.py untuk generate groups.csv", path))?;
        let reader = BufReader::new(file);

        // id -> origin
        let mut id_to_origin: BTreeMap<i64, String> = BTreeMap::new();
        for (li, line) in reader.lines().enumerate() {
            let line = line?;
            if li == 0 { continue; } // header
            if line.trim().is_empty() { continue; }
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 2 { continue; }
            let id: i64 = cols[0].trim().parse()
                .with_context(|| format!("id tidak valid di groups.csv baris {}", li + 1))?;
            let origin = cols[1].trim().to_string();
            id_to_origin.insert(id, origin);
        }

        // Susun origins sejajar dengan urutan sample_ids FeatureDataset
        let mut origins = Vec::with_capacity(sample_ids.len());
        for &id in sample_ids {
            let origin = id_to_origin.get(&id)
                .with_context(|| format!("origin tidak ditemukan untuk id={}", id))?;
            origins.push(origin.clone());
        }

        // Daftar origin unik terurut
        let mut unique: Vec<String> = origins.clone();
        unique.sort();
        unique.dedup();

        Ok(GroupInfo { origins, unique_origins: unique })
    }
}

/// Satu split LOOO: train (semua origin kecuali satu) + test (satu origin).
pub struct LooSplit {
    pub train_features: Array2<f32>,  // dinormalisasi (z-score fit di train)
    pub train_labels:   Array1<i64>,
    pub test_features:  Array2<f32>,  // dinormalisasi pakai stats train
    pub test_labels:    Array1<i64>,
    pub train_raw:      Array2<f32>,  // TANPA normalisasi (utk MLP/model yg normalisasi internal)
    pub test_raw:       Array2<f32>,  // TANPA normalisasi
    pub norm_stats:     FeatureNormStats,
    pub test_origin:    String,
}

/// Bangun split LOOO untuk satu `test_origin`.
/// Semua sampel dengan origin == test_origin → test; sisanya → train.
/// Z-score di-fit HANYA pada train (anti-leakage), lalu transform keduanya.
pub fn loo_split(
    dataset: &FeatureDataset,
    groups: &GroupInfo,
    test_origin: &str,
) -> LooSplit {
    let n = dataset.features.nrows();
    let nf = dataset.features.ncols();

    let mut train_idx: Vec<usize> = Vec::new();
    let mut test_idx:  Vec<usize> = Vec::new();
    for i in 0..n {
        if groups.origins[i] == test_origin {
            test_idx.push(i);
        } else {
            train_idx.push(i);
        }
    }

    let gather = |idx: &[usize]| -> (Array2<f32>, Array1<i64>) {
        let mut feat = Array2::<f32>::zeros((idx.len(), nf));
        let mut lab = Array1::<i64>::zeros(idx.len());
        for (row, &i) in idx.iter().enumerate() {
            for j in 0..nf { feat[[row, j]] = dataset.features[[i, j]]; }
            lab[row] = dataset.labels[i];
        }
        (feat, lab)
    };

    let (train_raw, train_labels) = gather(&train_idx);
    let (test_raw, test_labels)   = gather(&test_idx);

    // z-score fit di train saja
    let norm_stats = FeatureNormStats::fit(&train_raw);
    let train_features = norm_stats.transform(&train_raw);
    let test_features  = norm_stats.transform(&test_raw);

    LooSplit {
        train_features,
        train_labels,
        test_features,
        test_labels,
        train_raw,
        test_raw,
        norm_stats,
        test_origin: test_origin.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ROC & AUC
// ─────────────────────────────────────────────────────────────────────────────

/// Satu titik pada kurva ROC.
#[derive(Debug, Clone, Copy)]
pub struct RocPoint {
    pub threshold: f32,
    pub fpr: f32, // false positive rate (1 - specificity)
    pub tpr: f32, // true positive rate (sensitivity / recall)
}

/// Hasil perhitungan ROC + AUC.
#[derive(Debug, Clone)]
pub struct RocResult {
    pub points: Vec<RocPoint>,
    pub auc: f32,
}

/// Hitung ROC + AUC dari probabilitas kelas positif (high=1).
/// `scores[i]` = P(high) untuk sampel i, `labels[i]` = 0 (low) / 1 (high).
///
/// Memakai metode threshold-sweep: urutkan skor menurun, geser threshold,
/// hitung TPR & FPR, lalu AUC via aturan trapesium.
pub fn compute_roc_auc(scores: &[f32], labels: &[i64]) -> RocResult {
    assert_eq!(scores.len(), labels.len());

    let n_pos = labels.iter().filter(|&&l| l == 1).count();
    let n_neg = labels.iter().filter(|&&l| l == 0).count();

    // Jika hanya satu kelas, AUC tak terdefinisi → kembalikan 0.5 (acak) + kurva kosong.
    if n_pos == 0 || n_neg == 0 {
        return RocResult { points: Vec::new(), auc: 0.5 };
    }

    // Urutkan indeks berdasarkan skor menurun
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap_or(std::cmp::Ordering::Equal));

    let mut points: Vec<RocPoint> = Vec::new();
    points.push(RocPoint { threshold: f32::INFINITY, fpr: 0.0, tpr: 0.0 });

    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut prev_score = f32::INFINITY;

    for &i in &order {
        let s = scores[i];
        if s != prev_score {
            points.push(RocPoint {
                threshold: s,
                fpr: fp as f32 / n_neg as f32,
                tpr: tp as f32 / n_pos as f32,
            });
            prev_score = s;
        }
        if labels[i] == 1 { tp += 1; } else { fp += 1; }
    }
    // Titik akhir (1,1)
    points.push(RocPoint { threshold: f32::NEG_INFINITY, fpr: 1.0, tpr: 1.0 });

    // AUC via trapesium pada (fpr, tpr)
    let mut auc = 0.0f32;
    for w in points.windows(2) {
        let dx = w[1].fpr - w[0].fpr;
        let avg_y = (w[1].tpr + w[0].tpr) / 2.0;
        auc += dx * avg_y;
    }
    auc = auc.clamp(0.0, 1.0);

    RocResult { points, auc }
}

/// Interpretasi kualitas AUC (mengikuti infografik standar).
pub fn auc_interpretation(auc: f32) -> &'static str {
    if auc >= 0.90 { "Excellent" }
    else if auc >= 0.80 { "Very Good" }
    else if auc >= 0.70 { "Good" }
    else if auc >= 0.60 { "Fair" }
    else if auc >= 0.50 { "Poor" }
    else { "No discrimination" }
}
