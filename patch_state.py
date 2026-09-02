import re

with open("src/state.rs", "r") as f:
    code = f.read()

# Add phase struct and properties to AppState
phase_struct = """
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
"""

code = code.replace("/// Shared application state", phase_struct + "\n/// Shared application state")

code = code.replace("pub acquisition_service: AcquisitionService,\n}", "pub acquisition_service: AcquisitionService,\n    pub sys_config: std::sync::Arc<parking_lot::Mutex<SystemConfig>>,\n}")

code = code.replace("acquisition_service,\n        }", "acquisition_service,\n            sys_config: std::sync::Arc::new(parking_lot::Mutex::new(SystemConfig::default())),\n        }")

with open("src/state.rs", "w") as f:
    f.write(code)

