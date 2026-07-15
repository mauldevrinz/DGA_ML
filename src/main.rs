mod domain;
mod infrastructure;
mod application;
mod state;
mod ui;
mod app;

use anyhow::Result;
use state::AppState;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("dga_enose=info".parse()?))
        .init();

    info!("Starting DGA Electronic Nose application...");

    // Initialize database
    let db_path = "dga_data.db";
    let pool = infrastructure::database::initialize_database(db_path).await?;
    let app_state = AppState::new(pool);

    info!("Database and state initialized. Starting GUI...");

    // GUI Options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1024.0, 768.0])
            .with_title("DGA Electronic Nose — Transformer Fault Diagnosis"),
        ..Default::default()
    };

    // Run eframe App
    eframe::run_native(
        "DGA ENose",
        options,
        Box::new(|cc| Ok(Box::new(app::DgaApp::new(cc, app_state)))),
    ).map_err(|e| anyhow::anyhow!("GUI error: {:?}", e))?;

    Ok(())
}
