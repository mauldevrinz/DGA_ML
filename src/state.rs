#![allow(dead_code)]
use std::sync::Arc;
use sqlx::SqlitePool;

use crate::infrastructure::repositories::session_repo::SqliteSessionRepo;
use crate::infrastructure::repositories::sensor_data_repo::SqliteSensorDataRepo;
use crate::infrastructure::repositories::feature_repo::SqliteFeatureRepo;
use crate::infrastructure::repositories::model_repo::SqliteModelRepo;
use crate::infrastructure::repositories::prediction_repo::SqlitePredictionRepo;
use crate::infrastructure::serial::teensy::TeensySerial;
use crate::application::acquisition_service::AcquisitionService;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db_pool: SqlitePool,
    
    // Repositories
    pub session_repo: SqliteSessionRepo,
    pub sensor_repo: SqliteSensorDataRepo,
    pub feature_repo: SqliteFeatureRepo,
    pub model_repo: SqliteModelRepo,
    pub prediction_repo: SqlitePredictionRepo,
    
    // Hardware
    pub serial: Arc<TeensySerial>,
    
    // Services
    pub acquisition_service: AcquisitionService,
}

impl AppState {
    pub fn new(db_pool: SqlitePool) -> Self {
        let session_repo = SqliteSessionRepo::new(db_pool.clone());
        let sensor_repo = SqliteSensorDataRepo::new(db_pool.clone());
        let feature_repo = SqliteFeatureRepo::new(db_pool.clone());
        let model_repo = SqliteModelRepo::new(db_pool.clone());
        let prediction_repo = SqlitePredictionRepo::new(db_pool.clone());
        
        let serial = Arc::new(TeensySerial::new(115200));
        
        let acquisition_service = AcquisitionService::new(
            session_repo.clone(),
            sensor_repo.clone(),
            serial.clone(),
        );

        Self {
            db_pool,
            session_repo,
            sensor_repo,
            feature_repo,
            model_repo,
            prediction_repo,
            serial,
            acquisition_service,
        }
    }
}
