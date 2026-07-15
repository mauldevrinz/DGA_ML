#![allow(dead_code)]
use eframe::egui::{Color32, Visuals, Style, Stroke, FontId, FontFamily, TextStyle};

// ── Dark SCADA / Industrial Color Palette ───────────────────────────────────
pub const BG_DARK: Color32 = Color32::from_rgb(8, 10, 18);
pub const BG_COLOR: Color32 = Color32::from_rgb(12, 15, 25);
pub const PANEL_BG: Color32 = Color32::from_rgb(16, 20, 35);
pub const CARD_BG: Color32 = Color32::from_rgb(20, 26, 44);
pub const SURFACE: Color32 = Color32::from_rgb(26, 32, 52);
pub const BORDER: Color32 = Color32::from_rgb(38, 48, 72);
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(50, 62, 90);

// Text colors
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(226, 232, 240);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(168, 178, 200);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(108, 120, 148);

// Accent colors
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(56, 136, 255);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(6, 200, 230);
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(138, 92, 246);
pub const ACCENT_INDIGO: Color32 = Color32::from_rgb(99, 102, 241);

// Status colors
pub const STATUS_NORMAL: Color32 = Color32::from_rgb(34, 197, 94);
pub const STATUS_WARN: Color32 = Color32::from_rgb(250, 180, 20);
pub const STATUS_ERROR: Color32 = Color32::from_rgb(239, 68, 68);
pub const STATUS_INFO: Color32 = Color32::from_rgb(56, 189, 248);

// Sidebar
pub const SIDEBAR_BG: Color32 = Color32::from_rgb(10, 12, 22);
pub const SIDEBAR_HOVER: Color32 = Color32::from_rgb(22, 28, 50);
pub const SIDEBAR_ACTIVE: Color32 = Color32::from_rgb(30, 38, 65);

pub fn apply_scada_theme(ctx: &eframe::egui::Context) {
    let mut style = Style::default();

    // Better font sizing
    style.text_styles = [
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ].into();

    // Spacing
    style.spacing.item_spacing = egui::Vec2::new(8.0, 6.0);
    style.spacing.button_padding = egui::Vec2::new(14.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(16);

    let mut visuals = Visuals::dark();
    visuals.window_fill = PANEL_BG;
    visuals.panel_fill = BG_COLOR;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_corner_radius = egui::CornerRadius::same(10);

    visuals.widgets.noninteractive.bg_fill = PANEL_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 50, 80);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.active.bg_fill = ACCENT_BLUE;
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);

    visuals.selection.bg_fill = Color32::from_rgba_premultiplied(56, 136, 255, 60);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_BLUE);

    visuals.extreme_bg_color = BG_DARK;
    visuals.faint_bg_color = CARD_BG;

    style.visuals = visuals;

    ctx.set_style(style);
}

/// Draw a styled card frame (reusable widget)
pub fn card_frame(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(CARD_BG)
        .corner_radius(10)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(16.0)
        .show(ui, |ui| {
            add_contents(ui);
        });
}

/// Draw a section heading with accent line
pub fn section_heading(ui: &mut egui::Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).size(20.0));
        ui.label(egui::RichText::new(title).size(18.0).strong().color(TEXT_PRIMARY));
    });
    ui.add_space(2.0);
    let rect = ui.available_rect_before_wrap();
    let line_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x, rect.min.y),
        egui::vec2(rect.width(), 2.0),
    );
    ui.painter().rect_filled(line_rect, 1.0, Color32::from_rgba_premultiplied(56, 136, 255, 40));
    ui.add_space(8.0);
}

/// Styled metric display for dashboard cards
pub fn metric_value(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.label(egui::RichText::new(value).size(22.0).strong().color(color));
    });
}

use egui;
