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

        // Session controls
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
}
