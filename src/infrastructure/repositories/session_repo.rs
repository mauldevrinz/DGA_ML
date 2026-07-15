#![allow(dead_code)]
use anyhow::Result;
use sqlx::SqlitePool;
use crate::domain::models::AcquisitionSession;
use crate::domain::enums::FaultClass;

/// SQLite implementation of the session repository
#[derive(Clone)]
pub struct SqliteSessionRepo {
    pool: SqlitePool,
}

impl SqliteSessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, session: &AcquisitionSession) -> Result<()> {
        sqlx::query(
            "INSERT INTO acquisition_sessions (id, name, label, started_at, ended_at, sample_count, notes)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(session.label.as_str())
        .bind(&session.started_at)
        .bind(&session.ended_at)
        .bind(session.sample_count)
        .bind(&session.notes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update(&self, session: &AcquisitionSession) -> Result<()> {
        sqlx::query(
            "UPDATE acquisition_sessions SET name=?, label=?, ended_at=?, sample_count=?, notes=? WHERE id=?"
        )
        .bind(&session.name)
        .bind(session.label.as_str())
        .bind(&session.ended_at)
        .bind(session.sample_count)
        .bind(&session.notes)
        .bind(&session.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<AcquisitionSession>> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT id, name, label, started_at, ended_at, sample_count, notes, created_at
             FROM acquisition_sessions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.into()))
    }

    pub async fn list_all(&self) -> Result<Vec<AcquisitionSession>> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT id, name, label, started_at, ended_at, sample_count, notes, created_at
             FROM acquisition_sessions ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM acquisition_sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    name: String,
    label: String,
    started_at: String,
    ended_at: Option<String>,
    sample_count: i32,
    notes: Option<String>,
    created_at: String,
}

impl From<SessionRow> for AcquisitionSession {
    fn from(row: SessionRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            label: FaultClass::from_str(&row.label).unwrap_or(FaultClass::Baseline),
            started_at: row.started_at,
            ended_at: row.ended_at,
            sample_count: row.sample_count,
            notes: row.notes,
            created_at: row.created_at,
        }
    }
}
