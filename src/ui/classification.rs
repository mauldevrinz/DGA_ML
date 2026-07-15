#![allow(dead_code)]
use egui::Ui;
use crate::state::AppState;
use crate::ui::theme;
use crate::ui::widgets::alarm_panel::alarm_panel;

pub fn render(ui: &mut Ui, _state: &mut AppState) {
    theme::section_heading(ui, "⚡", "Real-Time Classification");
    
    ui.add_space(16.0);

    let total_width = ui.available_width();
    let spacing = 24.0;
    let left_width = (total_width * 0.4) - (spacing / 2.0);
    let right_width = (total_width * 0.6) - (spacing / 2.0);

    ui.horizontal(|ui| {
        // Left Column - Active Model
        ui.vertical(|ui| {
            ui.set_min_width(left_width.max(300.0));
            ui.set_max_width(left_width.max(300.0));
            
            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Active Model Config").strong().size(16.0).color(theme::TEXT_PRIMARY));
                ui.add_space(16.0);
                
                ui.horizontal(|ui| {
                    ui.label("Selected Model:");
                    egui::ComboBox::from_id_salt("active_model_combo")
                        .selected_text("None")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut 0, 0, "None");
                        });
                });
                
                ui.add_space(16.0);
                if ui.button(egui::RichText::new("▶ Start Classification").color(theme::STATUS_NORMAL)).clicked() {
                    // start
                }
            });
            
            ui.add_space(16.0);
            
            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Prediction History").strong().size(16.0).color(theme::TEXT_PRIMARY));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("No predictions yet.").color(theme::TEXT_MUTED));
            });
        });

        ui.add_space(spacing);

        // Right Column - Alarm Panel
        ui.vertical(|ui| {
            ui.set_min_width(right_width.max(400.0));
            
            alarm_panel(ui, None);
            
            ui.add_space(16.0);
            
            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Confidence Scores").strong().size(16.0).color(theme::TEXT_PRIMARY));
                ui.add_space(16.0);
                
                // Placeholder bars
                for &class in &["Baseline", "Normal", "Overheating", "Arcing"] {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(class).size(14.0).color(theme::TEXT_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("0.0%");
                            ui.add(egui::ProgressBar::new(0.0).show_percentage());
                        });
                    });
                    ui.add_space(8.0);
                }
            });
        });
    });
}
