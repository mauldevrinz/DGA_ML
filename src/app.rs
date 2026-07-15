use eframe::egui;
use crate::state::AppState;
use crate::ui;
use crate::ui::theme;

#[derive(PartialEq)]
enum Page {
    Acquisition,
    MLStudio,
    Classification,
    About,
}

pub struct DgaApp {
    state: AppState,
    current_page: Page,
}

impl DgaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, state: AppState) -> Self {
        ui::theme::apply_scada_theme(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        
        Self {
            state,
            current_page: Page::Acquisition,
        }
    }
}

impl eframe::App for DgaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top Navbar
        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::default().fill(theme::SIDEBAR_BG).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("⚡").size(24.0).color(theme::ACCENT_CYAN));
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new("DGA Electronic Nose").color(theme::TEXT_PRIMARY).strong().size(20.0));
                    ui.label(egui::RichText::new("— Transformer Fault Diagnosis").color(theme::TEXT_MUTED).size(16.0));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        let status_color = if self.state.serial.is_connected() { theme::STATUS_NORMAL } else { theme::STATUS_WARN };
                        let status_text = if self.state.serial.is_connected() { "● Connected" } else { "○ Disconnected" };
                        ui.label(egui::RichText::new(status_text).color(status_color).strong());
                    });
                });
            });

        // Sidebar
        egui::SidePanel::left("sidebar")
            .exact_width(220.0)
            .frame(egui::Frame::default().fill(theme::SIDEBAR_BG).inner_margin(egui::Margin::symmetric(12, 24)))
            .show(ctx, |ui| {
                
                let nav_button = |ui: &mut egui::Ui, text: &str, is_selected: bool| -> egui::Response {
                    let text_color = if is_selected { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY };
                    let bg_color = if is_selected { theme::SIDEBAR_ACTIVE } else { egui::Color32::TRANSPARENT };
                    
                    let mut btn = egui::Button::new(egui::RichText::new(text).size(16.0).color(text_color))
                        .fill(bg_color)
                        .frame(true)
                        .min_size(egui::vec2(ui.available_width(), 40.0));
                    
                    if is_selected {
                        btn = btn.stroke(egui::Stroke::new(1.0, theme::ACCENT_BLUE));
                    }
                    
                    ui.add(btn)
                };

                ui.add_space(10.0);
                ui.label(egui::RichText::new("MAIN MENU").size(11.0).color(theme::TEXT_MUTED).strong());
                ui.add_space(8.0);

                if nav_button(ui, "📊 Data Acquisition", self.current_page == Page::Acquisition).clicked() {
                    self.current_page = Page::Acquisition;
                }
                ui.add_space(4.0);
                if nav_button(ui, "🧠 ML Studio", self.current_page == Page::MLStudio).clicked() {
                    self.current_page = Page::MLStudio;
                }
                ui.add_space(4.0);
                if nav_button(ui, "⚡ Classification", self.current_page == Page::Classification).clicked() {
                    self.current_page = Page::Classification;
                }
                
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(10.0);
                    if nav_button(ui, "ℹ About", self.current_page == Page::About).clicked() {
                        self.current_page = Page::About;
                    }
                });
            });

        // Main Content Area
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::BG_COLOR).inner_margin(24.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.current_page {
                        Page::Acquisition => ui::acquisition::render(ui, &mut self.state),
                        Page::MLStudio => ui::ml_studio::render(ui, &mut self.state),
                        Page::Classification => ui::classification::render(ui, &mut self.state),
                        Page::About => ui::about::render(ui),
                    }
                });
            });
        
        // Request constant repaints if serial is connected to render real-time charts
        if self.state.serial.is_connected() {
            ctx.request_repaint();
        }
    }
}
