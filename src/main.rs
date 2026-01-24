use ikanban::{AppState, KanbanApp};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("iKanban - AI-Powered Task Management"),
        ..Default::default()
    };

    let db_path = std::path::PathBuf::from("ikanban.db");
    let app_state = Arc::new(AppState::new(db_path).await.expect("Failed to initialize database"));

    eframe::run_native(
        "iKanban",
        native_options,
        Box::new(move |_cc| Ok(Box::new(KanbanApp::new(app_state.clone())))),
    )
}
