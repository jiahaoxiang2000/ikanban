use clap::Parser;

mod action;
mod app;
mod cli;
mod components;
mod config;
mod errors;
mod logging;
mod models;
mod tui;
mod ws_client;

use app::App;
use cli::Cli;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    errors::init()?;
    logging::init()?;

    let args = Cli::parse();
    let mut app = App::new(args.tick_rate, args.frame_rate, &args.server)?;
    app.run().await?;
    Ok(())
}
