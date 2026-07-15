#![allow(dead_code)]
use anyhow::Result;
use super::models::*;
use super::enums::FaultClass;

/// Repository trait for acquisition sessions
pub trait SessionRepository: Send + Sync {
    fn create_session(&self, session: &AcquisitionSession) -> impl std::future::Future<Output = Result<()>> + Send;
    fn update_session(&self, session: &AcquisitionSession) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_session(&self, id: &str) -> impl std::future::Future<Output = Result<Option<AcquisitionSession>>> + Send;
    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<AcquisitionSession>>> + Send;
    fn delete_session(&self, id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Repository trait for sensor data
pub trait SensorDataRepository: Send + Sync {
    fn insert_batch(&self, session_id: &str, readings: &[SensorReading]) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_session_data(&self, session_id: &str) -> impl std::future::Future<Output = Result<Vec<SensorReading>>> + Send;
    fn get_latest(&self, session_id: &str, limit: i64) -> impl std::future::Future<Output = Result<Vec<SensorReading>>> + Send;
    fn count_by_session(&self, session_id: &str) -> impl std::future::Future<Output = Result<i64>> + Send;
}

/// Repository trait for extracted features
pub trait FeatureRepository: Send + Sync {
    fn save_features(&self, features: &ExtractedFeature) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_all_features(&self) -> impl std::future::Future<Output = Result<Vec<ExtractedFeature>>> + Send;
    fn get_features_by_session(&self, session_id: &str) -> impl std::future::Future<Output = Result<Option<ExtractedFeature>>> + Send;
    fn delete_by_session(&self, session_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Repository trait for trained models
pub trait ModelRepository: Send + Sync {
    fn save_model(&self, model: &TrainedModel) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_model(&self, id: &str) -> impl std::future::Future<Output = Result<Option<TrainedModel>>> + Send;
    fn list_models(&self) -> impl std::future::Future<Output = Result<Vec<TrainedModel>>> + Send;
    fn delete_model(&self, id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    fn save_training_result(&self, result: &TrainingResult) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_training_result(&self, model_id: &str) -> impl std::future::Future<Output = Result<Option<TrainingResult>>> + Send;
}

/// Repository trait for prediction logs
pub trait PredictionRepository: Send + Sync {
    fn log_prediction(&self, log: &PredictionLog) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_recent(&self, limit: i64) -> impl std::future::Future<Output = Result<Vec<PredictionLog>>> + Send;
    fn get_fault_counts(&self) -> impl std::future::Future<Output = Result<std::collections::HashMap<FaultClass, i64>>> + Send;
}

/// Trait for ML model implementations
pub trait MLModel: Send + Sync {
    fn train(&mut self, features: &ndarray::Array2<f64>, labels: &ndarray::Array1<usize>) -> Result<()>;
    fn predict(&self, features: &ndarray::Array2<f64>) -> Result<Vec<usize>>;
    fn predict_proba(&self, features: &ndarray::Array2<f64>) -> Result<Vec<Vec<f64>>>;
    fn save(&self) -> Result<Vec<u8>>;
    fn load(data: &[u8]) -> Result<Self> where Self: Sized;
}
