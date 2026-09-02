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


#[derive(Clone, PartialEq, Debug)]
pub enum SystemPhase {
    Idle,
    Injecting,
    Purging,
    Off,
}

#[derive(Clone)]
pub struct SystemConfig {
    pub idle_mins: u32,
    pub inject_mins: u32,
    pub purge_mins: u32,
    pub pump1_pwm: u16,
    pub current_phase: SystemPhase,
    pub phase_start_time: Option<std::time::Instant>,
    pub show_popup: bool,
    pub show_save_dialog: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            idle_mins: 1,
            inject_mins: 1,
            purge_mins: 1,
            pump1_pwm: 50,
            current_phase: SystemPhase::Off,
            phase_start_time: None,
            show_popup: false,
            show_save_dialog: false,
        }
    }
}

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
    pub sys_config: std::sync::Arc<parking_lot::Mutex<SystemConfig>>,
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
            sys_config: std::sync::Arc::new(parking_lot::Mutex::new(SystemConfig::default())),
        }
    }
}
