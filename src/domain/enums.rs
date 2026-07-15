#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Classification labels for transformer oil fault diagnosis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FaultClass {
    Baseline,
    Normal,
    Overheating,
    Arcing,
}

impl FaultClass {
    pub fn all() -> &'static [FaultClass] {
        &[
            FaultClass::Baseline,
            FaultClass::Normal,
            FaultClass::Overheating,
            FaultClass::Arcing,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FaultClass::Baseline => "baseline",
            FaultClass::Normal => "normal",
            FaultClass::Overheating => "overheating",
            FaultClass::Arcing => "arcing",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "baseline" => Some(FaultClass::Baseline),
            "normal" => Some(FaultClass::Normal),
            "overheating" => Some(FaultClass::Overheating),
            "arcing" => Some(FaultClass::Arcing),
            _ => None,
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            FaultClass::Baseline => 0,
            FaultClass::Normal => 1,
            FaultClass::Overheating => 2,
            FaultClass::Arcing => 3,
        }
    }

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(FaultClass::Baseline),
            1 => Some(FaultClass::Normal),
            2 => Some(FaultClass::Overheating),
            3 => Some(FaultClass::Arcing),
            _ => None,
        }
    }

    /// Color for UI display (RGBA)
    pub fn color(&self) -> egui::Color32 {
        match self {
            FaultClass::Baseline => egui::Color32::from_rgb(107, 114, 128),   // Gray
            FaultClass::Normal => egui::Color32::from_rgb(34, 197, 94),       // Green
            FaultClass::Overheating => egui::Color32::from_rgb(234, 179, 8),  // Yellow
            FaultClass::Arcing => egui::Color32::from_rgb(239, 68, 68),       // Red
        }
    }
}

impl std::fmt::Display for FaultClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Machine learning model types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelType {
    SVM,
    RandomForest,
    SNN,
}

impl ModelType {
    pub fn all() -> &'static [ModelType] {
        &[ModelType::SVM, ModelType::RandomForest, ModelType::SNN]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ModelType::SVM => "svm",
            ModelType::RandomForest => "random_forest",
            ModelType::SNN => "snn",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ModelType::SVM => "Support Vector Machine",
            ModelType::RandomForest => "Random Forest",
            ModelType::SNN => "Spiking Neural Network",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "svm" => Some(ModelType::SVM),
            "random_forest" => Some(ModelType::RandomForest),
            "snn" => Some(ModelType::SNN),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Normalization methods for preprocessing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalizationMethod {
    ZScore,
    MinMax,
}

impl NormalizationMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            NormalizationMethod::ZScore => "Z-Score",
            NormalizationMethod::MinMax => "Min-Max",
        }
    }
}

/// Serial connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl ConnectionStatus {
    pub fn color(&self) -> egui::Color32 {
        match self {
            ConnectionStatus::Disconnected => egui::Color32::from_rgb(107, 114, 128),
            ConnectionStatus::Connecting => egui::Color32::from_rgb(234, 179, 8),
            ConnectionStatus::Connected => egui::Color32::from_rgb(34, 197, 94),
            ConnectionStatus::Error => egui::Color32::from_rgb(239, 68, 68),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ConnectionStatus::Disconnected => "Disconnected",
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Error => "Error",
        }
    }
}

/// SVM kernel types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SvmKernel {
    Linear,
    Rbf,
    Polynomial,
}

impl SvmKernel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SvmKernel::Linear => "Linear",
            SvmKernel::Rbf => "RBF",
            SvmKernel::Polynomial => "Polynomial",
        }
    }
}

/// Training progress state
#[derive(Debug, Clone)]
pub enum TrainingStatus {
    Idle,
    Extracting,
    Preprocessing,
    Training { progress: f32, message: String },
    Evaluating,
    Complete,
    Failed(String),
}
