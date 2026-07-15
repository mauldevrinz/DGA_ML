#![allow(dead_code)]
use egui::{Ui, Color32};
use crate::state::AppState;
use crate::ui::theme;

pub fn render(ui: &mut Ui, _state: &mut AppState) {
    theme::section_heading(ui, "🧠", "Machine Learning Studio");
    
    ui.add_space(16.0);

    let total_width = ui.available_width();
    let spacing = 24.0;
    let left_width = (total_width * 0.4) - (spacing / 2.0);
    let right_width = (total_width * 0.6) - (spacing / 2.0);

    ui.horizontal(|ui| {
        // Left Column - Datasets & Models
        ui.vertical(|ui| {
            ui.set_min_width(left_width.max(300.0));
            ui.set_max_width(left_width.max(300.0));
            
            theme::card_frame(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Datasets").strong().size(16.0).color(theme::TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("📁 Select Folder").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                println!("Selected dataset folder: {:?}", path);
                            }
                        }
                        if ui.button("⟳ Refresh").clicked() { }
                    });
                });
                ui.add_space(8.0);
                ui.label(egui::RichText::new("No datasets available. Record some data first.").color(theme::TEXT_MUTED));
            });

            ui.add_space(16.0);

            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Trained Models").strong().size(16.0).color(theme::TEXT_PRIMARY));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("No models trained yet.").color(theme::TEXT_MUTED));
            });
        });

        ui.add_space(spacing);

        // Right Column - Training config
        ui.vertical(|ui| {
            ui.set_min_width(right_width.max(400.0));
            
            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Train New Model").strong().size(16.0).color(theme::TEXT_PRIMARY));
                ui.add_space(16.0);
                
                egui::Grid::new("train_config").spacing(egui::vec2(16.0, 16.0)).show(ui, |ui| {
                    ui.label("Model Type:");
                    egui::ComboBox::from_id_salt("model_type_combo")
                        .selected_text("Support Vector Machine (SVM)")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut 0, 0, "Support Vector Machine (SVM)");
                            ui.selectable_value(&mut 1, 1, "Random Forest");
                            ui.selectable_value(&mut 2, 2, "Spiking Neural Network (SNN)");
                        });
                    ui.end_row();

                    ui.label("Normalization:");
                    egui::ComboBox::from_id_salt("norm_type_combo")
                        .selected_text("Z-Score")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut 0, 0, "Z-Score");
                            ui.selectable_value(&mut 1, 1, "Min-Max");
                        });
                    ui.end_row();
                });

                ui.add_space(24.0);
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = egui::Button::new(egui::RichText::new("▶ Start Training").color(Color32::WHITE).strong())
                        .fill(theme::ACCENT_BLUE)
                        .min_size(egui::vec2(120.0, 32.0));
                    if ui.add(btn).clicked() { }
                });
            });
            
            ui.add_space(16.0);
            
            theme::card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Training Progress").strong().size(16.0).color(theme::TEXT_PRIMARY));
                ui.add_space(16.0);
                ui.add(egui::ProgressBar::new(0.0).text("Ready"));
            });
        });
    });
}
