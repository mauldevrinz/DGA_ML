import re

with open("src/ui/acquisition.rs", "r") as f:
    code = f.read()

# Replace session controls
session_controls_orig = """        // Session controls
        theme::card_frame(ui, |ui| {
            ui.horizontal(|ui| {
                // We'd have state for session name/label in a real app, placeholder for now
                if ui.button(egui::RichText::new("▶ Start Recording").color(theme::STATUS_NORMAL)).clicked() {
                    // Start session
                }
                if ui.button(egui::RichText::new("⏹ Stop Recording").color(theme::STATUS_ERROR)).clicked() {
                    // Stop session
                }
            });
        });"""

session_controls_new = """        // System Phase Controls
        theme::card_frame(ui, |ui| {
            ui.horizontal(|ui| {
                let mut config = state.sys_config.lock();
                if config.current_phase == crate::state::SystemPhase::Off {
                    if ui.button(egui::RichText::new("⚙ Configure System").color(theme::TEXT_PRIMARY)).clicked() {
                        config.show_popup = true;
                    }
                    if ui.button(egui::RichText::new("▶ Start System").color(theme::STATUS_NORMAL)).clicked() {
                        config.current_phase = crate::state::SystemPhase::Idle;
                        config.phase_start_time = Some(std::time::Instant::now());
                        if state.serial.is_connected() {
                            let _ = state.serial.write(format!("PWM={}\\n", config.pump1_pwm).as_bytes());
                            let _ = state.serial.write(b"PHASE=IDLE\\n");
                        }
                    }
                } else {
                    let phase_name = match config.current_phase {
                        crate::state::SystemPhase::Idle => "Idle",
                        crate::state::SystemPhase::Injecting => "Injecting",
                        crate::state::SystemPhase::Purging => "Purging",
                        crate::state::SystemPhase::Off => "Off",
                    };
                    let elapsed = config.phase_start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                    let target_mins = match config.current_phase {
                        crate::state::SystemPhase::Idle => config.idle_mins,
                        crate::state::SystemPhase::Injecting => config.inject_mins,
                        crate::state::SystemPhase::Purging => config.purge_mins,
                        _ => 0,
                    };
                    let remain = (target_mins * 60).saturating_sub(elapsed as u32);
                    
                    ui.label(egui::RichText::new(format!("Current Phase: {} ({}s remaining)", phase_name, remain)).strong().color(theme::ACCENT_CYAN));
                    
                    if ui.button(egui::RichText::new("⏹ Stop System").color(theme::STATUS_ERROR)).clicked() {
                        config.current_phase = crate::state::SystemPhase::Off;
                        if state.serial.is_connected() {
                            let _ = state.serial.write(b"PHASE=OFF\\n");
                        }
                        config.show_save_dialog = true;
                    }
                }
            });
        });"""
code = code.replace(session_controls_orig, session_controls_new)

# Add logic for updating phases and popups at the end of render function
update_logic = """
    // Phase update logic
    {
        let mut config = state.sys_config.lock();
        if config.current_phase != crate::state::SystemPhase::Off {
            let elapsed = config.phase_start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);
            let target_mins = match config.current_phase {
                crate::state::SystemPhase::Idle => config.idle_mins,
                crate::state::SystemPhase::Injecting => config.inject_mins,
                crate::state::SystemPhase::Purging => config.purge_mins,
                _ => 0,
            };
            
            if elapsed >= (target_mins * 60) as u64 {
                match config.current_phase {
                    crate::state::SystemPhase::Idle => {
                        config.current_phase = crate::state::SystemPhase::Injecting;
                        config.phase_start_time = Some(std::time::Instant::now());
                        if state.serial.is_connected() {
                            let _ = state.serial.write(b"PHASE=INJECT\\n");
                        }
                    },
                    crate::state::SystemPhase::Injecting => {
                        config.current_phase = crate::state::SystemPhase::Purging;
                        config.phase_start_time = Some(std::time::Instant::now());
                        if state.serial.is_connected() {
                            let _ = state.serial.write(b"PHASE=PURGE\\n");
                        }
                    },
                    crate::state::SystemPhase::Purging => {
                        config.current_phase = crate::state::SystemPhase::Off;
                        if state.serial.is_connected() {
                            let _ = state.serial.write(b"PHASE=OFF\\n");
                        }
                        config.show_save_dialog = true;
                    },
                    _ => {}
                }
            }
        }
        
        if config.show_popup {
            let mut show = config.show_popup;
            egui::Window::new("System Configuration").open(&mut show).show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Idle Phase (mins):");
                    ui.add(egui::Slider::new(&mut config.idle_mins, 0..=5));
                });
                ui.horizontal(|ui| {
                    ui.label("Injecting Phase (mins):");
                    ui.add(egui::Slider::new(&mut config.inject_mins, 0..=5));
                });
                ui.horizontal(|ui| {
                    ui.label("Purging Phase (mins):");
                    ui.add(egui::Slider::new(&mut config.purge_mins, 0..=5));
                });
                ui.horizontal(|ui| {
                    ui.label("Pump 1 PWM (%):");
                    ui.add(egui::Slider::new(&mut config.pump1_pwm, 0..=100));
                });
                if ui.button("Close").clicked() {
                    config.show_popup = false;
                }
            });
            if !show { config.show_popup = false; }
        }
        
        if config.show_save_dialog {
            let mut show = config.show_save_dialog;
            egui::Window::new("System Finished").open(&mut show).show(ui.ctx(), |ui| {
                ui.label("System has completed all phases.");
                if ui.button("Save CSV Data").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).save_file() {
                        // TODO: Dump sensor data repo to CSV
                        println!("Saving to {:?}", path);
                    }
                    config.show_save_dialog = false;
                }
            });
            if !show { config.show_save_dialog = false; }
        }
    }
}
"""

code = re.sub(r'}\s*$', update_logic, code)

with open("src/ui/acquisition.rs", "w") as f:
    f.write(code)

