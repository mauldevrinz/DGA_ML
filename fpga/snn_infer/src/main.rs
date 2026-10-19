//! Entry point binary snn_infer.
//!
//! Input stdin JSON:
//! {
//!   "recording": [[mos0, mos1, ..., mos15], ...], // array Nx16 mV
//!   "use_hw": false // optional, default false (true untuk coba FPGA HW)
//! }
//!
//! Output stdout JSON:
//! {
//!   "ok": true,
//!   "gas_names": ["C2H2", "C2H4", "CH4", "H2", "OTHER"],
//!   "gas_confidences": {"C2H2": 12.3, ...},
//!   "predicted_gas": "CH4",
//!   "predicted_idx": 2,
//!   "confidence": 0.85,
//!   "predicted_fault": "T1",
//!   "feature_ms": 15.2,
//!   "latency_ms": 0.5,
//!   "mode": "sw" // atau "hw"
//! }

mod features;
mod snn;
mod fpga;
mod fault;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Deserialize)]
struct InputRequest {
    recording: Vec<Vec<f64>>,
    #[serde(default)]
    use_hw: bool,
}

#[derive(Serialize)]
struct OutputResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas_confidences: Option<HashMap<String, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    predicted_gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    predicted_idx: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    predicted_fault: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    feature_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
}

fn get_models_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SNN_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let parent = exe.parent().unwrap_or(&exe);
    let check = parent.join("models");
    if check.exists() {
        return check;
    }
    // Fallback relative path dari repo
    PathBuf::from("/home/petalinux/maulvin/DGA_ML/fpga/snn_infer/models")
}

fn main() {
    let models_dir = get_models_dir();
    let snn_path = models_dir.join("snn_weights.json");
    let fault_path = models_dir.join("fault_trees.json");

    let weights = match snn::Weights::load(&snn_path) {
        Ok(w) => w,
        Err(e) => {
            let resp = OutputResponse {
                ok: false,
                error: Some(format!("Failed to load snn_weights.json from {:?}: {}", snn_path, e)),
                gas_names: None,
                gas_confidences: None,
                predicted_gas: None,
                predicted_idx: None,
                confidence: None,
                predicted_fault: None,
                feature_ms: None,
                latency_ms: None,
                mode: None,
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    let fault_model = match fault::RfModel::load(&fault_path) {
        Ok(m) => m,
        Err(e) => {
            let resp = OutputResponse {
                ok: false,
                error: Some(format!("Failed to load fault_trees.json from {:?}: {}", fault_path, e)),
                gas_names: None,
                gas_confidences: None,
                predicted_gas: None,
                predicted_idx: None,
                confidence: None,
                predicted_fault: None,
                feature_ms: None,
                latency_ms: None,
                mode: None,
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    // Baca baris per baris dari stdin (daemon/streaming mode)
    let stdin = io::stdin();
    use std::io::BufRead;
    for line_res in stdin.lock().lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: InputRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = OutputResponse {
                    ok: false,
                    error: Some(format!("Invalid JSON input: {}", e)),
                    gas_names: None,
                    gas_confidences: None,
                    predicted_gas: None,
                    predicted_idx: None,
                    confidence: None,
                    predicted_fault: None,
                    feature_ms: None,
                    latency_ms: None,
                    mode: None,
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
                continue;
            }
        };

        if req.recording.is_empty() {
            let resp = OutputResponse {
                ok: false,
                error: Some("Recording data is empty".to_string()),
                gas_names: None,
                gas_confidences: None,
                predicted_gas: None,
                predicted_idx: None,
                confidence: None,
                predicted_fault: None,
                feature_ms: None,
                latency_ms: None,
                mode: None,
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
            continue;
        }

        // 1. Ekstraksi 37 fitur tsfresh
        let t_feat_start = Instant::now();
        let raw_features = features::compute_all(&req.recording);
        let feature_ms = t_feat_start.elapsed().as_secs_f64() * 1000.0;

        // 2. Inferensi SNN Gas (HW atau SW)
        let t_infer_start = Instant::now();
        let (logits, mode) = if req.use_hw {
            let mut norm_feats = [0.0f32; 37];
            for i in 0..37 {
                let val = if raw_features[i].is_finite() { raw_features[i] } else { weights.impute[i] };
                norm_feats[i] = (val as f32 - weights.mean[i]) * weights.inv_scale[i];
            }
            match fpga::infer_hw(&norm_feats, std::time::Duration::from_millis(2000)) {
                Ok(lg) => (lg, "hw".to_string()),
                Err(e) => {
                    let lg = snn::infer(&weights, &raw_features);
                    (lg, format!("sw_fallback ({})", e))
                }
            }
        } else {
            let lg = snn::infer(&weights, &raw_features);
            (lg, "sw".to_string())
        };
        let latency_ms = t_infer_start.elapsed().as_secs_f64() * 1000.0;

        // 3. Confidence gas (Sigmoid)
        let conf = snn::logits_to_conf(&logits);

        let mut best_gas_idx = 0;
        let mut best_gas_conf = conf[0];
        for (i, &c) in conf.iter().enumerate() {
            if c > best_gas_conf {
                best_gas_conf = c;
                best_gas_idx = i;
            }
        }

        let mut gas_conf_map = HashMap::new();
        let mut conf_f64 = [0.0f64; 5];
        for (i, name) in weights.gas_classes.iter().enumerate() {
            gas_conf_map.insert(name.clone(), (conf[i] * 1000.0).round() / 10.0);
            conf_f64[i] = conf[i] as f64;
        }

        // 4. Prediksi Fault via RF
        let pred_fault = fault::predict(&fault_model, &conf_f64);

        let resp = OutputResponse {
            ok: true,
            error: None,
            gas_names: Some(weights.gas_classes.clone()),
            gas_confidences: Some(gas_conf_map),
            predicted_gas: Some(weights.gas_classes[best_gas_idx].clone()),
            predicted_idx: Some(best_gas_idx),
            confidence: Some((best_gas_conf * 1000.0).round() / 1000.0),
            predicted_fault: Some(pred_fault.to_string()),
            feature_ms: Some((feature_ms * 10.0).round() / 10.0),
            latency_ms: Some((latency_ms * 1000.0).round() / 1000.0),
            mode: Some(mode),
        };

        println!("{}", serde_json::to_string(&resp).unwrap());
    }
}
