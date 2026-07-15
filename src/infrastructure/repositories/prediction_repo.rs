#![allow(dead_code)]
use anyhow::Result;
use sqlx::SqlitePool;
use crate::domain::models::PredictionLog;
use crate::domain::enums::FaultClass;
use std::collections::HashMap;

#[derive(Clone)]
pub struct SqlitePredictionRepo {
    pool: SqlitePool,
}

impl SqlitePredictionRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn log_prediction(&self, log: &PredictionLog) -> Result<()> {
        let probs = serde_json::to_string(&log.probabilities)?;
        let snap = serde_json::to_string(&log.sensor_snapshot)?;
        sqlx::query("INSERT INTO prediction_logs (model_id,predicted_class,confidence,probabilities,sensor_snapshot,inference_time_ms) VALUES(?,?,?,?,?,?)")
            .bind(&log.model_id).bind(log.predicted_class.as_str()).bind(log.confidence)
            .bind(&probs).bind(&snap).bind(log.inference_time_ms)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_recent(&self, limit: i64) -> Result<Vec<PredictionLog>> {
        let rows = sqlx::query_as::<_, PredRow>("SELECT id,model_id,predicted_class,confidence,probabilities,sensor_snapshot,inference_time_ms,created_at FROM prediction_logs ORDER BY created_at DESC LIMIT ?")
            .bind(limit).fetch_all(&self.pool).await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    pub async fn get_fault_counts(&self) -> Result<HashMap<String, i64>> {
        let rows = sqlx::query_as::<_, (String, i64)>("SELECT predicted_class, COUNT(*) FROM prediction_logs GROUP BY predicted_class")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().collect())
    }
}

#[derive(sqlx::FromRow)]
struct PredRow { id: i64, model_id: String, predicted_class: String, confidence: f64, probabilities: String, sensor_snapshot: String, inference_time_ms: i64, created_at: String }

impl TryFrom<PredRow> for PredictionLog {
    type Error = anyhow::Error;
    fn try_from(r: PredRow) -> Result<Self> {
        Ok(Self { id: Some(r.id), model_id: r.model_id,
            predicted_class: FaultClass::from_str(&r.predicted_class).unwrap_or(FaultClass::Baseline),
            confidence: r.confidence, probabilities: serde_json::from_str(&r.probabilities)?,
            sensor_snapshot: serde_json::from_str(&r.sensor_snapshot)?,
            inference_time_ms: r.inference_time_ms, created_at: Some(r.created_at) })
    }
}
