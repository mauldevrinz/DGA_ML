#![allow(dead_code)]
use egui::{Ui, Color32, Stroke};
use crate::domain::models::SensorStats;
use crate::ui::theme::{PANEL_BG, ACCENT_BLUE, TEXT_MUTED};

pub fn sensor_card(ui: &mut Ui, name: &str, stats: &SensorStats, unit: &str) {
    let card_color = PANEL_BG;
    let stroke = Stroke::new(1.0, Color32::from_rgb(30, 41, 59));

    egui::Frame::NONE
        .fill(card_color)
        .corner_radius(8)
        .stroke(stroke)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(name).color(TEXT_MUTED).size(14.0));
                
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{:.2}", stats.current)).strong().size(24.0).color(ACCENT_BLUE));
                    ui.label(egui::RichText::new(unit).color(TEXT_MUTED).size(14.0));
                });
                
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Min:").color(TEXT_MUTED).size(12.0));
                    let min_val = if stats.count > 0 { stats.min } else { 0.0 };
                    ui.label(egui::RichText::new(format!("{:.2}", min_val)).size(12.0));
                    
                    ui.add_space(12.0);
                    
                    ui.label(egui::RichText::new("Max:").color(TEXT_MUTED).size(12.0));
                    let max_val = if stats.count > 0 { stats.max } else { 0.0 };
                    ui.label(egui::RichText::new(format!("{:.2}", max_val)).size(12.0));
                });
            });
        });
}
