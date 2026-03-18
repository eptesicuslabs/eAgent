//! eCode — Native desktop GUI for agentic coding with Codex CLI.

mod app;
mod panels;
mod state;
mod theme;
mod widgets;

use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();

    tracing::info!("Starting eCode");

    // Create the tokio runtime for async operations
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // Run the eframe native app
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("eCode")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "eCode",
        native_options,
        Box::new(move |cc| {
            // Configure fonts and visuals
            theme::configure_theme(&cc.egui_ctx);

            // Enable image loading
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::new(app::ECodeApp::new(cc, runtime)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}
