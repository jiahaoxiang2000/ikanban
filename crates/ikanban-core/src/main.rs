use std::net::SocketAddr;
use std::path::PathBuf;

use ikanban_core::{db, routes, AppState};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn get_database_path() -> PathBuf {
    let mut path = PathBuf::from(".ikanban");
    path.push("data");
    path.push("db.sqlite");
    path
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ikanban_core=debug"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();



    // Ensure the database directory exists
    let db_path = get_database_path();
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
            tracing::info!("Created database directory: {}", parent.display());
        }
    }

    // For SeaORM SQLite, we need to ensure the file exists first
    if !db_path.exists() {
        std::fs::File::create(&db_path)?;
        tracing::info!("Created database file: {}", db_path.display());
    }

    // SeaORM uses sqlite:// prefix (not sqlite://)
    let database_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    tracing::info!("Connecting to database: {}", database_url);
    let db = db::create_connection(&database_url).await?;

    // Create application state
    let state = AppState::new(db);

    // Build router
    let app = routes::router(state);

    // Get port from environment or use default
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("iKanban server listening on http://{}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
