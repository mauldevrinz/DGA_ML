#![allow(dead_code)]
use anyhow::Result;
use sqlx::SqlitePool;
use crate::domain::models::SensorReading;

/// SQLite implementation for sensor data storage
#[derive(Clone)]
pub struct SqliteSensorDataRepo {
    pool: SqlitePool,
}

impl SqliteSensorDataRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a batch of sensor readings efficiently
    pub async fn insert_batch(&self, session_id: &str, readings: &[SensorReading]) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for reading in readings {
            let v = &reading.values;
            sqlx::query(
                "INSERT INTO sensor_data (session_id, timestamp_ms,
                 mos_01, mos_02, mos_03, mos_04, mos_05, mos_06, mos_07,
                 mos_08, mos_09, mos_10, mos_11, mos_12, mos_13, mos_14,
                 ndir, sht20_temp, sht20_humidity)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(session_id)
            .bind(reading.timestamp_ms)
            .bind(v[0]).bind(v[1]).bind(v[2]).bind(v[3])
            .bind(v[4]).bind(v[5]).bind(v[6]).bind(v[7])
            .bind(v[8]).bind(v[9]).bind(v[10]).bind(v[11])
            .bind(v[12]).bind(v[13]).bind(v[14]).bind(v[15])
            .bind(v[16])
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get all sensor data for a session
    pub async fn get_session_data(&self, session_id: &str) -> Result<Vec<SensorReading>> {
        let rows = sqlx::query_as::<_, SensorRow>(
            "SELECT timestamp_ms,
             mos_01, mos_02, mos_03, mos_04, mos_05, mos_06, mos_07,
             mos_08, mos_09, mos_10, mos_11, mos_12, mos_13, mos_14,
             ndir, sht20_temp, sht20_humidity
             FROM sensor_data WHERE session_id = ? ORDER BY timestamp_ms ASC"
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_reading()).collect())
    }

    /// Get latest N readings for a session
    pub async fn get_latest(&self, session_id: &str, limit: i64) -> Result<Vec<SensorReading>> {
        let rows = sqlx::query_as::<_, SensorRow>(
            "SELECT timestamp_ms,
             mos_01, mos_02, mos_03, mos_04, mos_05, mos_06, mos_07,
             mos_08, mos_09, mos_10, mos_11, mos_12, mos_13, mos_14,
             ndir, sht20_temp, sht20_humidity
             FROM sensor_data WHERE session_id = ?
             ORDER BY timestamp_ms DESC LIMIT ?"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut result: Vec<SensorReading> = rows.into_iter().map(|r| r.into_reading()).collect();
        result.reverse();
        Ok(result)
    }

    /// Count readings in a session
    pub async fn count_by_session(&self, session_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sensor_data WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

#[derive(sqlx::FromRow)]
struct SensorRow {
    timestamp_ms: i64,
    mos_01: f64, mos_02: f64, mos_03: f64, mos_04: f64,
    mos_05: f64, mos_06: f64, mos_07: f64, mos_08: f64,
    mos_09: f64, mos_10: f64, mos_11: f64, mos_12: f64,
    mos_13: f64, mos_14: f64,
    ndir: f64,
    sht20_temp: f64, sht20_humidity: f64,
}

impl SensorRow {
    fn into_reading(self) -> SensorReading {
        SensorReading {
            timestamp_ms: self.timestamp_ms,
            values: [
                self.mos_01, self.mos_02, self.mos_03, self.mos_04,
                self.mos_05, self.mos_06, self.mos_07, self.mos_08,
                self.mos_09, self.mos_10, self.mos_11, self.mos_12,
                self.mos_13, self.mos_14, self.ndir,
                self.sht20_temp, self.sht20_humidity,
            ],
        }
    }
}
