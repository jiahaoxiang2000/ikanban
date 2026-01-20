use clap::Parser;

use crate::config::{get_config_dir, get_data_dir};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Tick rate, i.e. number of ticks per second
    #[arg(short, long, value_name = "FLOAT", default_value_t = 4.0)]
    pub tick_rate: f64,

    /// Frame rate, i.e. number of frames per second
    #[arg(short, long, value_name = "FLOAT", default_value_t = 60.0)]
    pub frame_rate: f64,

    /// Server URL
    #[arg(
        short,
        long,
        value_name = "URL",
        default_value = "http://127.0.0.1:3000"
    )]
    pub server: String,
}

pub fn version() -> String {
    let author = clap::crate_authors!();
    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
ikanban-tui

Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}"
    )
}
