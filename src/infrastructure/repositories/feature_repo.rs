#![allow(dead_code)]
use anyhow::Result;
use sqlx::SqlitePool;
use crate::domain::models::ExtractedFeature;
use crate::domain::enums::FaultClass;

/// SQLite implementation for extracted features storage
#[derive(Clone)]
pub struct SqliteFeatureRepo {
    pool: SqlitePool,
}

impl SqliteFeatureRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn save(&self, features: &ExtractedFeature) -> Result<()> {
        let vector_json = serde_json::to_string(&features.feature_vector)?;
        let names_json = serde_json::to_string(&features.feature_names)?;

        sqlx::query(
            "INSERT INTO extracted_features (session_id, feature_vector, feature_names, label)
             VALUES (?, ?, ?, ?)"
        )
        .bind(&features.session_id)
        .bind(&vector_json)
        .bind(&names_json)
        .bind(features.label.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_all(&self) -> Result<Vec<ExtractedFeature>> {
        let rows = sqlx::query_as::<_, FeatureRow>(
            "SELECT id, session_id, feature_vector, feature_names, label, created_at
             FROM extracted_features ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    pub async fn get_by_session(&self, session_id: &str) -> Result<Option<ExtractedFeature>> {
        let row = sqlx::query_as::<_, FeatureRow>(
            "SELECT id, session_id, feature_vector, feature_names, label, created_at
             FROM extracted_features WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(r) => Ok(Some(r.try_into()?)),
            None => Ok(None),
        }
    }

    pub async fn delete_by_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM extracted_features WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM extracted_features")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

#[derive(sqlx::FromRow)]
struct FeatureRow {
    id: i64,
    session_id: String,
    feature_vector: String,
    feature_names: String,
    label: String,
    created_at: String,
}

impl TryFrom<FeatureRow> for ExtractedFeature {
    type Error = anyhow::Error;
    fn try_from(row: FeatureRow) -> Result<Self> {
        Ok(Self {
            id: Some(row.id),
            session_id: row.session_id,
            feature_vector: serde_json::from_str(&row.feature_vector)?,
            feature_names: serde_json::from_str(&row.feature_names)?,
            label: FaultClass::from_str(&row.label).unwrap_or(FaultClass::Baseline),
            created_at: Some(row.created_at),
        })
    }
}
