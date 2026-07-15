use egui::{Ui, Color32, Vec2, Sense, Stroke};

pub fn status_led(ui: &mut Ui, color: Color32, radius: f32) -> egui::Response {
    let size = Vec2::splat(radius * 2.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let center = rect.center();
        ui.painter().circle_filled(center, radius, color);
        // Inner glow/highlight for 3D effect
        ui.painter().circle_stroke(
            center,
            radius,
            Stroke::new(1.0, Color32::WHITE.linear_multiply(0.3)),
        );
    }
    response
}
