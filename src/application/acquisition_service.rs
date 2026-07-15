#![allow(dead_code)]
use anyhow::Result;
use std::sync::Arc;
use crate::infrastructure::repositories::session_repo::SqliteSessionRepo;
use crate::infrastructure::repositories::sensor_data_repo::SqliteSensorDataRepo;
use crate::infrastructure::serial::teensy::TeensySerial;
use crate::domain::models::AcquisitionSession;
use crate::domain::enums::FaultClass;

#[derive(Clone)]
pub struct AcquisitionService {
    session_repo: SqliteSessionRepo,
    sensor_repo: SqliteSensorDataRepo,
    serial: Arc<TeensySerial>,
}

impl AcquisitionService {
    pub fn new(
        session_repo: SqliteSessionRepo,
        sensor_repo: SqliteSensorDataRepo,
        serial: Arc<TeensySerial>,
    ) -> Self {
        Self { session_repo, sensor_repo, serial }
    }

    pub async fn start_session(&self, name: String, label: FaultClass, notes: Option<String>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let session = AcquisitionSession {
            id: id.clone(),
            name,
            label,
            started_at: chrono::Utc::now().to_rfc3339(),
            ended_at: None,
            sample_count: 0,
            notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.session_repo.create(&session).await?;
        Ok(id)
    }

    pub async fn stop_session(&self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.session_repo.get(session_id).await? {
            session.ended_at = Some(chrono::Utc::now().to_rfc3339());
            session.sample_count = self.sensor_repo.count_by_session(session_id).await? as i32;
            self.session_repo.update(&session).await?;
        }
        Ok(())
    }

    pub async fn flush_buffer_to_db(&self, session_id: &str) -> Result<usize> {
        let readings = self.serial.drain_buffer();
        let count = readings.len();
        if count > 0 {
            self.sensor_repo.insert_batch(session_id, &readings).await?;
        }
        Ok(count)
    }
}
