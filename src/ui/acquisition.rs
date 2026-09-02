#![allow(dead_code)]
use egui::{Ui, Color32};
use crate::state::AppState;
use crate::ui::theme;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    theme::section_heading(ui, "📊", "Data Acquisition Dashboard");
    
    ui.add_space(16.0);

    // Top control bar
    ui.horizontal(|ui| {
        theme::card_frame(ui, |ui| {
            ui.horizontal(|ui| {
                if !state.serial.is_connected() {
                    if ui.button(egui::RichText::new("🔌 Connect Auto").color(Color32::WHITE)).clicked() {
                        if let Some(port) = crate::infrastructure::serial::teensy::TeensySerial::auto_detect() {
                            let _ = state.serial.connect(&port);
                        }
                    }
                    
                    ui.add_space(12.0);
                    
                    let mut current_port = state.serial.port_name().unwrap_or_else(|| "Select Port...".to_string());
                    egui::ComboBox::from_id_salt("port_selector")
                        .selected_text(&current_port)
                        .show_ui(ui, |ui| {
                            if let Ok(ports) = crate::infrastructure::serial::teensy::TeensySerial::list_ports() {
                                for p in ports {
                                    if ui.selectable_value(&mut current_port, p.name.clone(), &p.name).clicked() {
                                        let _ = state.serial.connect(&p.name);
                                    }
                                }
                            }
                        });
                } else {
                    if ui.button(egui::RichText::new("⏹ Disconnect").color(theme::STATUS_ERROR)).clicked() {
                        state.serial.disconnect();
                    }
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(format!("Connected to: {}", state.serial.port_name().unwrap_or_default())).color(theme::TEXT_MUTED));
                }
            });
        });

        ui.add_space(16.0);

        // System Phase Controls
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
                            let _ = state.serial.write(format!("PWM={}\n", config.pump1_pwm).as_bytes());
                            let _ = state.serial.write(b"PHASE=IDLE\n");
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
                            let _ = state.serial.write(b"PHASE=OFF\n");
                        }
                        config.show_save_dialog = true;
                    }
                }
            });
        });
    });

    ui.add_space(24.0);

    // Main sensor view
    if state.serial.is_connected() {
        if let Some(reading) = state.serial.get_latest() {
            ui.columns(4, |cols| {
                // Temperature
                cols[0].vertical(|ui| {
                    theme::card_frame(ui, |ui| {
                        theme::metric_value(ui, "Temperature", &format!("{:.1} °C", reading.temperature()), theme::ACCENT_CYAN);
                    });
                });
                // Humidity
                cols[1].vertical(|ui| {
                    theme::card_frame(ui, |ui| {
                        theme::metric_value(ui, "Humidity", &format!("{:.1} %", reading.humidity()), theme::ACCENT_BLUE);
                    });
                });
                // NDIR
                cols[2].vertical(|ui| {
                    theme::card_frame(ui, |ui| {
                        theme::metric_value(ui, "NDIR (CO2)", &format!("{:.0} ppm", reading.ndir()), theme::ACCENT_PURPLE);
                    });
                });
                // Data points
                cols[3].vertical(|ui| {
                    theme::card_frame(ui, |ui| {
                        // Placeholder for buffer size
                        theme::metric_value(ui, "Buffer Size", "0 points", theme::TEXT_PRIMARY);
                    });
                });
            });

            ui.add_space(24.0);
            ui.label(egui::RichText::new("MOS Sensors (Gas Array)").strong().color(theme::TEXT_SECONDARY));
            ui.add_space(8.0);

            // 14 MOS sensors in a flow layout
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
                for i in 0..14 {
                    theme::card_frame(ui, |ui| {
                        ui.set_min_width(120.0);
                        theme::metric_value(ui, &format!("MOS-{:02}", i + 1), &format!("{:.2} V", reading.mos(i)), theme::STATUS_INFO);
                    });
                }
            });
            
            ui.add_space(24.0);
            
            // Plot placeholder
            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Real-time Signal Plot").strong().color(theme::TEXT_SECONDARY));
                ui.add_space(8.0);
                let (rect, _resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 250.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, theme::SURFACE);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "[ egui_plot widget would go here ]",
                    egui::FontId::proportional(16.0),
                    theme::TEXT_MUTED
                );
            });

        } else {
            theme::card_frame(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("Waiting for data...").color(theme::TEXT_MUTED));
                    ui.add_space(40.0);
                });
            });
        }
    } else {
        theme::card_frame(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(egui::RichText::new("🔌").size(40.0));
                ui.add_space(16.0);
                ui.label(egui::RichText::new("Not Connected").size(20.0).strong().color(theme::TEXT_SECONDARY));
                ui.label(egui::RichText::new("Connect to the Teensy device to view real-time data.").color(theme::TEXT_MUTED));
                ui.add_space(80.0);
            });
        });
    }

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
                            let _ = state.serial.write(b"PHASE=INJECT
");
                        }
                    },
                    crate::state::SystemPhase::Injecting => {
                        config.current_phase = crate::state::SystemPhase::Purging;
                        config.phase_start_time = Some(std::time::Instant::now());
                        if state.serial.is_connected() {
                            let _ = state.serial.write(b"PHASE=PURGE
");
                        }
                    },
                    crate::state::SystemPhase::Purging => {
                        config.current_phase = crate::state::SystemPhase::Off;
                        if state.serial.is_connected() {
                            let _ = state.serial.write(b"PHASE=OFF
");
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
