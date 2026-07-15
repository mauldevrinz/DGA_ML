#![allow(dead_code)]
use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tracing::info;

/// Initialize the SQLite database with WAL mode and run migrations
pub async fn initialize_database(db_path: &str) -> Result<SqlitePool> {
    info!("Initializing database at: {}", db_path);

    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;

    info!("Database initialized successfully");
    Ok(pool)
}

/// Execute the initial migration SQL
async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    let migration_sql = include_str!("../../migrations/001_initial.sql");

    // Split by semicolons and execute each statement
    for statement in migration_sql.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed)
                .execute(pool)
                .await?;
        }
    }

    info!("Database migrations completed");
    Ok(())
}

/// Get database file size in bytes
pub async fn get_db_size(db_path: &str) -> Result<u64> {
    let metadata = tokio::fs::metadata(db_path).await?;
    Ok(metadata.len())
}

/// Get row counts for all tables
pub async fn get_table_counts(pool: &SqlitePool) -> Result<std::collections::HashMap<String, i64>> {
    let mut counts = std::collections::HashMap::new();

    let tables = [
        "acquisition_sessions",
        "sensor_data",
        "extracted_features",
        "trained_models",
        "training_results",
        "prediction_logs",
    ];

    for table in tables {
        let row: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(pool)
            .await?;
        counts.insert(table.to_string(), row.0);
    }

    Ok(counts)
}
