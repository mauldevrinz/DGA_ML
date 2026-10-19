//! Inferensi SW untuk model SNN gas (tanpa FPGA).
//! Bobot dari snn_weights.json yang digenerate export_models.py.
//! Logika identik dengan snn_kria.py _logits_sw().

use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub struct Weights {
    pub w1:        Vec<Vec<f32>>,  // [120][37]
    pub b1:        Vec<f32>,       // [120]
    pub w2:        Vec<Vec<f32>>,  // [5][24]
    pub b2:        Vec<f32>,       // [5]
    pub mean:      Vec<f32>,       // [37]
    pub inv_scale: Vec<f32>,       // [37]
    pub impute:    Vec<f64>,       // [37]  nilai imputasi NaN
    pub gas_classes: Vec<String>,  // ["C2H2","C2H4","CH4","H2","OTHER"]
}

impl Weights {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&s)?)
    }
}

/// Imputasi + normalisasi fitur mentah.
fn prepare(w: &Weights, features: &[f64; 37]) -> Vec<f32> {
    features.iter().enumerate().map(|(i, &v)| {
        let val = if v.is_finite() { v } else { w.impute[i] };
        (val as f32 - w.mean[i]) * w.inv_scale[i]
    }).collect()
}

/// Inferensi SW (simplified LIF single-pass), returns 5 logits.
///
/// Pipeline identik snn_kria.py _logits_sw():
///   z = (W1 @ f_norm + B1) > 1.0      # threshold biner LIF
///   h = z.reshape(5, 24)
///   logits = (h * W2).sum(axis=1) + B2
pub fn infer(w: &Weights, features: &[f64; 37]) -> [f32; 5] {
    let f = prepare(w, features);

    // hidden pre-activation: W1 @ f + B1  → [120]
    let pre: Vec<f32> = w.w1.iter().zip(w.b1.iter()).map(|(row, &b)| {
        let dot: f32 = row.iter().zip(f.iter()).map(|(&wi, &fi)| wi * fi).sum();
        dot + b
    }).collect();

    // LIF spike: z[i] = 1 jika pre[i] > 1.0, else 0
    let z: Vec<f32> = pre.iter().map(|&v| if v > 1.0 { 1.0 } else { 0.0 }).collect();

    // readout: h = z.reshape(5,24); logits[i] = sum(h[i]*W2[i]) + B2[i]
    let mut logits = [0.0f32; 5];
    for i in 0..5 {
        logits[i] = w.b2[i] + w.w2[i].iter().enumerate()
            .map(|(j, &wij)| wij * z[i * 24 + j])
            .sum::<f32>();
    }
    logits
}

#[inline]
pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x as f64).exp() as f32)
}

pub fn logits_to_conf(logits: &[f32; 5]) -> [f32; 5] {
    let mut c = [0.0f32; 5];
    for (i, &l) in logits.iter().enumerate() { c[i] = sigmoid(l); }
    c
}
