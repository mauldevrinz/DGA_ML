#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use super::enums::{FaultClass, ModelType};

/// Sensor names for the 17 channels
pub const SENSOR_NAMES: [&str; 17] = [
    "MOS-01", "MOS-02", "MOS-03", "MOS-04", "MOS-05", "MOS-06", "MOS-07",
    "MOS-08", "MOS-09", "MOS-10", "MOS-11", "MOS-12", "MOS-13", "MOS-14",
    "NDIR", "SHT30-Temp", "SHT30-Humidity",
];

/// Feature names for the 15 statistical features extracted per sensor
pub const FEATURE_NAMES: [&str; 15] = [
    "Mean", "Median", "Maximum", "Minimum", "RMS",
    "StdDev", "Variance", "Skewness", "Kurtosis",
    "AUC", "RiseTime", "FallTime", "PeakValue",
    "SignalEnergy", "ResponseRate",
];

pub const NUM_SENSORS: usize = 17;
pub const NUM_FEATURES_PER_SENSOR: usize = 15;
pub const TOTAL_FEATURES: usize = NUM_SENSORS * NUM_FEATURES_PER_SENSOR; // 255

/// A single reading from all 17 sensor channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub timestamp_ms: i64,
    pub values: [f64; NUM_SENSORS],
}

impl SensorReading {
    pub fn new(timestamp_ms: i64, values: [f64; NUM_SENSORS]) -> Self {
        Self { timestamp_ms, values }
    }

    pub fn mos(&self, index: usize) -> f64 {
        assert!(index < 14, "MOS index must be 0..13");
        self.values[index]
    }

    pub fn ndir(&self) -> f64 {
        self.values[14]
    }

    pub fn temperature(&self) -> f64 {
        self.values[15]
    }

    pub fn humidity(&self) -> f64 {
        self.values[16]
    }
}

impl Default for SensorReading {
    fn default() -> Self {
        Self {
            timestamp_ms: 0,
            values: [0.0; NUM_SENSORS],
        }
    }
}

/// Acquisition session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionSession {
    pub id: String,
    pub name: String,
    pub label: FaultClass,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub sample_count: i32,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Extracted feature vector for one session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFeature {
    pub id: Option<i64>,
    pub session_id: String,
    pub feature_vector: Vec<f64>,
    pub feature_names: Vec<String>,
    pub label: FaultClass,
    pub created_at: Option<String>,
}

/// Trained model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainedModel {
    pub id: String,
    pub model_type: ModelType,
    pub name: String,
    pub hyperparameters: serde_json::Value,
    pub feature_names: Vec<String>,
    pub normalization_params: Option<NormalizationParams>,
    pub model_binary: Option<Vec<u8>>,
    pub created_at: String,
}

/// Normalization parameters to apply during inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationParams {
    pub method: String,
    pub means: Vec<f64>,
    pub stds: Vec<f64>,
    pub mins: Vec<f64>,
    pub maxs: Vec<f64>,
}

/// Training evaluation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    pub id: Option<i64>,
    pub model_id: String,
    pub accuracy: f64,
    pub precision_score: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub confusion_matrix: Vec<Vec<i32>>,
    pub training_time_ms: i64,
    pub inference_time_ms: i64,
    pub memory_usage_bytes: i64,
    pub fold_results: Option<Vec<FoldResult>>,
    pub created_at: Option<String>,
}

/// Per-fold evaluation result for k-fold CV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldResult {
    pub fold: usize,
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// Real-time prediction log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionLog {
    pub id: Option<i64>,
    pub model_id: String,
    pub predicted_class: FaultClass,
    pub confidence: f64,
    pub probabilities: std::collections::HashMap<String, f64>,
    pub sensor_snapshot: SensorReading,
    pub inference_time_ms: i64,
    pub created_at: Option<String>,
}

/// Statistics for a single sensor channel during monitoring
#[derive(Debug, Clone, Default)]
pub struct SensorStats {
    pub current: f64,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub count: usize,
}

impl SensorStats {
    pub fn new() -> Self {
        Self {
            current: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            sum: 0.0,
            count: 0,
        }
    }

    pub fn update(&mut self, value: f64) {
        self.current = value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.count += 1;
    }

    pub fn average(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum / self.count as f64 }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Hyperparameters for SVM training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmHyperparams {
    pub kernel: String,
    pub c: f64,
    pub gamma: f64,
}

impl Default for SvmHyperparams {
    fn default() -> Self {
        Self {
            kernel: "rbf".to_string(),
            c: 1.0,
            gamma: 0.1,
        }
    }
}

/// Hyperparameters for Random Forest training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfHyperparams {
    pub n_trees: usize,
    pub max_depth: Option<usize>,
    pub min_samples_split: usize,
}

impl Default for RfHyperparams {
    fn default() -> Self {
        Self {
            n_trees: 100,
            max_depth: Some(10),
            min_samples_split: 2,
        }
    }
}

/// Hyperparameters for Spiking Neural Network training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnnHyperparams {
    pub hidden_neurons: usize,
    pub threshold: f64,
    pub time_steps: usize,
    pub learning_rate: f64,
    pub epochs: usize,
}

impl Default for SnnHyperparams {
    fn default() -> Self {
        Self {
            hidden_neurons: 128,
            threshold: 1.0,
            time_steps: 25,
            learning_rate: 0.001,
            epochs: 100,
        }
    }
}
