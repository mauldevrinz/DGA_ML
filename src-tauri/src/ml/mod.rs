//! Machine Learning Module
//! preprocessing.rs DIHAPUS — pembersihan sinyal sudah di baseline_normalize.py (Python).
//! RF/SVM/MLP pakai TSFRESH; CNN/LSTM pakai deret waktu mentah dari data/processed.

pub mod models;
pub use models::cnn;
pub mod data_loader;
pub mod feature_loader;
pub mod loo;
pub mod tsfresh_bridge;
pub mod training;
pub mod evaluation;
pub mod validation;
pub mod predict;
pub mod analysis;
pub mod model;
pub mod visualization;

// ── Model types ──────────────────────────────────────────────────────────────
pub use models::CoffeeCNN;
pub use models::{CoffeeRandomForest, RandomForestConfig, RFTrainingMetrics, extract_features};
pub use models::{CoffeeSVM, SVMConfig, SVMTrainingMetrics, extract_features_svm};
pub use models::{CoffeeLSTM, LSTMConfig, LSTMTrainingMetrics};
pub use models::{CoffeeMLP, MLPConfig, MLPTrainingMetrics, extract_features_mlp};

// ── data_loader LAMA (dipakai CNN/LSTM GUI untuk deret waktu mentah) ─────────
pub use data_loader::{CoffeeDataLoader, NormalizationStats, CoffeeDataset, DataLoaderConfig};
pub use training::{Trainer, TrainingConfig, TrainingMetrics};
pub use evaluation::Evaluator;

// ── feature_loader BARU (dipakai RF/SVM/MLP untuk TSFRESH) ──────────────────
pub use feature_loader::{FeatureLoader, FeatureDataset, FeatureNormStats, stratified_split, FeatureSplit, stratified_split_raw, FeatureSplitRaw};
pub use loo::{GroupInfo, LooSplit, loo_split, RocPoint, RocResult, compute_roc_auc, auc_interpretation};
pub use tsfresh_bridge::{extract_features_via_python, ExtractedFeatures};