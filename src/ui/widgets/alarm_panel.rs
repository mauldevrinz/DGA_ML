#![allow(dead_code)]
use egui::{Ui, Color32, Stroke};
use crate::domain::enums::FaultClass;
use crate::ui::theme;

pub fn alarm_panel(ui: &mut Ui, current_fault: Option<FaultClass>) {
    theme::card_frame(ui, |ui| {
        ui.label(egui::RichText::new("Alarm Panel").strong().size(16.0).color(theme::TEXT_PRIMARY));
        ui.add_space(16.0);

        for class in FaultClass::all() {
            let is_active = current_fault == Some(*class);
            let bg_color = if is_active {
                class.color().linear_multiply(0.2)
            } else {
                theme::SURFACE
            };
            
            let text_color = if is_active { class.color() } else { theme::TEXT_MUTED };
            
            egui::Frame::NONE
                .fill(bg_color)
                .corner_radius(6)
                .stroke(Stroke::new(1.0, if is_active { class.color() } else { theme::BORDER }))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        crate::ui::widgets::status_led::status_led(
                            ui, 
                            if is_active { class.color() } else { Color32::from_gray(60) }, 
                            8.0
                        );
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(class.as_str().to_uppercase())
                            .color(text_color)
                            .size(16.0)
                            .strong());
                    });
                });
            ui.add_space(8.0);
        }
    });
}
