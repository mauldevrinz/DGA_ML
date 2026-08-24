#![allow(dead_code)]
use egui::{Ui, RichText, Color32, Layout, Align};
use crate::ui::theme;

pub fn render(ui: &mut Ui) {
    theme::section_heading(ui, "ℹ", "About DGA Electronic Nose");
    ui.add_space(16.0);

    theme::card_frame(ui, |ui| {
        ui.vertical(|ui| {
            // --- Header Section: Logos and Title ---
            ui.horizontal(|ui| {
                let logo_height = 120.0;
                
                // Left Logo
                // ui.add(Image::new(egui::include_image!("../../LOGO ITS.png")).max_height(logo_height));
                ui.add_space(100.0);
                
                // Center Title (dynamically takes up available space minus the right logo)
                let text_width = ui.available_width() - 140.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(text_width, logo_height),
                    Layout::top_down(Align::Center).with_cross_align(Align::Center),
                    |ui| {
                        ui.add_space(10.0);
                        ui.heading(RichText::new("PROPOSAL TUGAS AKHIR").size(28.0).strong().color(theme::ACCENT_CYAN));
                        ui.label(RichText::new("DGA Electronic Nose - Transformer Oil Fault Diagnosis System").size(18.0).color(theme::TEXT_PRIMARY));
                        ui.add_space(5.0);
                        ui.label(RichText::new("Diajukan untuk memenuhi salah satu syarat memperoleh gelar Sarjana Terapan").size(14.0).color(theme::TEXT_SECONDARY));
                    }
                );

                // Right Logo
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // ui.add(Image::new(egui::include_image!("../../ELKA LOGO.jpg")).max_height(logo_height));
                    ui.add_space(100.0);
                });
            });

            ui.add_space(30.0);
            ui.separator();
            ui.add_space(30.0);

            // --- Profile Section ---
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                
                // Profile Picture
                let profile_size = 240.0;
                // ui.add(
                //     Image::new(egui::include_image!("../../Profile.jpeg"))
                //         .max_height(profile_size)
                //         .max_width(profile_size)
                //         .corner_radius(12.0)
                // );
                ui.add_space(profile_size);
                
                ui.add_space(60.0);

                // Student Details
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("STUDENT PROFILE").size(14.0).strong().color(theme::TEXT_MUTED));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Akhmad Maulvin Nazir Zakaria").size(32.0).strong().color(theme::TEXT_PRIMARY));
                    ui.label(RichText::new("NRP. 2042231028").size(20.0).color(theme::ACCENT_BLUE));
                    
                    ui.add_space(20.0);
                    ui.label(RichText::new("Department:").size(16.0).color(theme::TEXT_MUTED));
                    ui.label(RichText::new("D-4 Teknologi Rekayasa Instrumentasi\nDepartemen Teknik Instrumentasi\nFakultas Vokasi\nInstitut Teknologi Sepuluh Nopember").size(16.0).color(theme::TEXT_SECONDARY));
                    
                    ui.add_space(20.0);
                    ui.label(RichText::new("Supervisor:").size(16.0).color(theme::TEXT_MUTED));
                    ui.label(RichText::new("Ahmad Radhy, S.Si., M.Si").size(18.0).strong().color(theme::TEXT_PRIMARY));
                });
            });

            ui.add_space(40.0);
            ui.separator();
            ui.add_space(20.0);

            // --- Build Information ---
            ui.vertical_centered(|ui| {
                let version = env!("CARGO_PKG_VERSION");
                let profile = if cfg!(debug_assertions) { "Debug" } else { "Release" };
                
                ui.label(RichText::new(format!("DGA Electronic Nose v{}", version)).color(theme::TEXT_MUTED).size(14.0));
                ui.label(RichText::new(format!("Build: {} | UI Framework: egui 0.31 | Database: SQLite", profile)).color(Color32::from_gray(100)).size(12.0));
                ui.add_space(10.0);
            });
        });
    });
}
