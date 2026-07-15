#![allow(dead_code)]
use anyhow::Result;
use ndarray::Array2;
use std::time::Instant;

use crate::domain::models::{SensorReading, PredictionLog, TrainedModel};
use crate::domain::enums::{FaultClass, ModelType};
use crate::domain::traits::MLModel;
use crate::infrastructure::ml::svm::SvmModel;
use crate::infrastructure::ml::random_forest::RandomForestModel;
use crate::infrastructure::ml::snn::CustomSnnModel;

pub struct InferenceService;

impl InferenceService {
    pub fn predict(
        reading: &SensorReading,
        model_meta: &TrainedModel,
        model_bytes: &[u8]
    ) -> Result<PredictionLog> {
        let start = Instant::now();

        // 1. Extract features from a single reading
        // Normally feature extraction needs a window. For a single reading, we mock window features.
        // Or we pass a window of readings. For pure real-time single-point classification,
        // we extract the 15 features across 1 reading (which means std=0, var=0, min=max=val, etc).
        let (mut feature_vec, _) = super::feature_service::extract_features(&[reading.clone()])?;

        // 2. Normalize
        if let Some(norm) = &model_meta.normalization_params {
            super::preprocessing_service::apply_normalization(&mut feature_vec, norm);
        }

        // 3. Load model
        let model: Box<dyn MLModel> = match model_meta.model_type {
            ModelType::SVM => Box::new(SvmModel::load(model_bytes)?),
            ModelType::RandomForest => Box::new(RandomForestModel::load(model_bytes)?),
            ModelType::SNN => Box::new(CustomSnnModel::load(model_bytes)?),
        };

        // 4. Predict
        let mut x = Array2::<f64>::zeros((1, feature_vec.len()));
        for (i, &v) in feature_vec.iter().enumerate() {
            x[[0, i]] = v;
        }

        let preds = model.predict(&x)?;
        let pred_class_idx = preds.first().copied().unwrap_or(0);
        let pred_class = FaultClass::from_index(pred_class_idx).unwrap_or(FaultClass::Baseline);

        let probs = model.predict_proba(&x).unwrap_or_else(|_| vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let class_probs = probs.first().cloned().unwrap_or_else(|| vec![1.0, 0.0, 0.0, 0.0]);

        let mut prob_map = std::collections::HashMap::new();
        for (i, c) in FaultClass::all().iter().enumerate() {
            prob_map.insert(c.as_str().to_string(), class_probs[i]);
        }

        let inference_time_ms = start.elapsed().as_millis() as i64;

        Ok(PredictionLog {
            id: None,
            model_id: model_meta.id.clone(),
            predicted_class: pred_class,
            confidence: class_probs[pred_class_idx],
            probabilities: prob_map,
            sensor_snapshot: reading.clone(),
            inference_time_ms,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
        })
    }
}
