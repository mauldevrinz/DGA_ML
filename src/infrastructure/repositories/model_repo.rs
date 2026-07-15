#![allow(dead_code)]
use anyhow::Result;
use sqlx::SqlitePool;
use crate::domain::models::{TrainedModel, TrainingResult};
use crate::domain::enums::ModelType;

#[derive(Clone)]
pub struct SqliteModelRepo {
    pool: SqlitePool,
}

impl SqliteModelRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn save_model(&self, model: &TrainedModel) -> Result<()> {
        let hp = serde_json::to_string(&model.hyperparameters)?;
        let fn_json = serde_json::to_string(&model.feature_names)?;
        let np = model.normalization_params.as_ref().map(|p| serde_json::to_string(p)).transpose()?;
        sqlx::query("INSERT OR REPLACE INTO trained_models (id,model_type,name,hyperparameters,feature_names,normalization_params,model_binary) VALUES(?,?,?,?,?,?,?)")
            .bind(&model.id).bind(model.model_type.as_str()).bind(&model.name)
            .bind(&hp).bind(&fn_json).bind(&np).bind(&model.model_binary)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_model(&self, id: &str) -> Result<Option<TrainedModel>> {
        let row = sqlx::query_as::<_, ModelRow>("SELECT id,model_type,name,hyperparameters,feature_names,normalization_params,model_binary,created_at FROM trained_models WHERE id=?")
            .bind(id).fetch_optional(&self.pool).await?;
        match row { Some(r) => Ok(Some(r.try_into()?)), None => Ok(None) }
    }

    pub async fn list_models(&self) -> Result<Vec<TrainedModel>> {
        let rows = sqlx::query_as::<_, ModelRow>("SELECT id,model_type,name,hyperparameters,feature_names,normalization_params,model_binary,created_at FROM trained_models ORDER BY created_at DESC")
            .fetch_all(&self.pool).await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    pub async fn delete_model(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM trained_models WHERE id=?").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn save_result(&self, r: &TrainingResult) -> Result<()> {
        let cm = serde_json::to_string(&r.confusion_matrix)?;
        let fr = r.fold_results.as_ref().map(|f| serde_json::to_string(f)).transpose()?;
        sqlx::query("INSERT INTO training_results (model_id,accuracy,precision_score,recall,f1_score,confusion_matrix,training_time_ms,inference_time_ms,memory_usage_bytes,fold_results) VALUES(?,?,?,?,?,?,?,?,?,?)")
            .bind(&r.model_id).bind(r.accuracy).bind(r.precision_score).bind(r.recall).bind(r.f1_score)
            .bind(&cm).bind(r.training_time_ms).bind(r.inference_time_ms).bind(r.memory_usage_bytes).bind(&fr)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_result(&self, model_id: &str) -> Result<Option<TrainingResult>> {
        let row = sqlx::query_as::<_, TrRow>("SELECT id,model_id,accuracy,precision_score,recall,f1_score,confusion_matrix,training_time_ms,inference_time_ms,memory_usage_bytes,fold_results,created_at FROM training_results WHERE model_id=?")
            .bind(model_id).fetch_optional(&self.pool).await?;
        match row { Some(r) => Ok(Some(r.try_into()?)), None => Ok(None) }
    }
}

#[derive(sqlx::FromRow)]
struct ModelRow { id: String, model_type: String, name: String, hyperparameters: String, feature_names: String, normalization_params: Option<String>, model_binary: Option<Vec<u8>>, created_at: String }

impl TryFrom<ModelRow> for TrainedModel {
    type Error = anyhow::Error;
    fn try_from(r: ModelRow) -> Result<Self> {
        Ok(Self { id: r.id, model_type: ModelType::from_str(&r.model_type).unwrap_or(ModelType::SVM), name: r.name,
            hyperparameters: serde_json::from_str(&r.hyperparameters)?, feature_names: serde_json::from_str(&r.feature_names)?,
            normalization_params: r.normalization_params.map(|s| serde_json::from_str(&s)).transpose()?,
            model_binary: r.model_binary, created_at: r.created_at })
    }
}

#[derive(sqlx::FromRow)]
struct TrRow { id: i64, model_id: String, accuracy: f64, precision_score: f64, recall: f64, f1_score: f64, confusion_matrix: String, training_time_ms: i64, inference_time_ms: i64, memory_usage_bytes: i64, fold_results: Option<String>, created_at: String }

impl TryFrom<TrRow> for TrainingResult {
    type Error = anyhow::Error;
    fn try_from(r: TrRow) -> Result<Self> {
        Ok(Self { id: Some(r.id), model_id: r.model_id, accuracy: r.accuracy, precision_score: r.precision_score,
            recall: r.recall, f1_score: r.f1_score, confusion_matrix: serde_json::from_str(&r.confusion_matrix)?,
            training_time_ms: r.training_time_ms, inference_time_ms: r.inference_time_ms, memory_usage_bytes: r.memory_usage_bytes,
            fold_results: r.fold_results.map(|s| serde_json::from_str(&s)).transpose()?, created_at: Some(r.created_at) })
    }
}
