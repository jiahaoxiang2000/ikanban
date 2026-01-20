use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::config;

pub fn init() -> color_eyre::Result<()> {
    let directory = config::get_data_dir();
    std::fs::create_dir_all(directory.clone())?;
    let log_file_path = directory.join(format!("{}.log", env!("CARGO_PKG_NAME")));
    let log_file = std::fs::File::create(log_file_path)?;
    let env_filter = EnvFilter::builder().with_default_directive(tracing::Level::INFO.into());
    let log_level_var = format!("{}_LOG_LEVEL", config::project_name());
    let env_filter = env_filter
        .try_from_env()
        .or_else(|_| env_filter.with_env_var(log_level_var).from_env())?;
    let file_subscriber = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_filter(env_filter);
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .try_init()?;
    Ok(())
}
