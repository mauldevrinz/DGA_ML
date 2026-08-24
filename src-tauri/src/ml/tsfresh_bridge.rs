// src/ml/tsfresh_bridge.rs
// Jembatan Rust → Python untuk ekstraksi fitur TSFRESH saat prediksi.
// Memanggil tools/python/tsfresh_predict_single.py dan membaca hasil JSON.

use anyhow::{Context, Result};
use ndarray::Array2;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct TsfreshOutput {
    #[serde(default)]
    features: Vec<f32>,
    #[serde(default)]
    feature_names: Vec<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Hasil ekstraksi fitur untuk satu file CSV.
pub struct ExtractedFeatures {
    /// Shape: (1, n_features) — siap masuk model
    pub features: Array2<f32>,
    pub feature_names: Vec<String>,
}

/// Tentukan perintah python (python / python3) sesuai OS.
fn python_command() -> &'static str {
    if cfg!(target_os = "windows") { "python" } else { "python3" }
}

/// Ekstrak fitur TSFRESH dari satu file CSV dengan memanggil script Python.
/// Mengembalikan Array2 (1 × n_features) yang siap diprediksi model.
///
/// PRASYARAT:
///   - Python + tsfresh terpasang
///   - data/features/selected_feature_names.json tersedia (hasil training)
pub fn extract_features_via_python(csv_path: &str) -> Result<ExtractedFeatures> {
    let script = "tools/python/tsfresh_predict_single.py";

    let output = Command::new(python_command())
        .arg(script)
        .arg(csv_path)
        .output()
        .with_context(|| format!(
            "Gagal menjalankan Python. Pastikan '{}' ada di PATH dan script '{}' ada.",
            python_command(), script
        ))?;

    // stderr berisi log progress (bukan error fatal); tampilkan untuk debug
    if !output.stderr.is_empty() {
        let log = String::from_utf8_lossy(&output.stderr);
        for line in log.lines() {
            println!("   [python] {}", line);
        }
    }

    if !output.status.success() {
        anyhow::bail!(
            "Script Python gagal (exit code {:?}). Cek apakah tsfresh terpasang: pip install tsfresh",
            output.status.code()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Ambil baris JSON terakhir (untuk jaga-jaga jika ada output lain)
    let json_line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .context("Output Python tidak berisi JSON yang valid")?;

    let parsed: TsfreshOutput = serde_json::from_str(json_line)
        .context("Gagal parse JSON dari Python")?;

    if let Some(err) = parsed.error {
        anyhow::bail!("Error dari script Python: {}", err);
    }

    if parsed.features.is_empty() {
        anyhow::bail!("Python tidak menghasilkan fitur (kosong)");
    }

    let n = parsed.features.len();
    let features = Array2::from_shape_vec((1, n), parsed.features)
        .context("Gagal membentuk Array2 dari fitur")?;

    Ok(ExtractedFeatures {
        features,
        feature_names: parsed.feature_names,
    })
}