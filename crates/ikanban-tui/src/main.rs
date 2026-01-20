use clap::Parser;

mod action;
mod api;
mod app;
mod cli;
mod components;
mod config;
mod errors;
mod logging;
mod models;
mod tui;
mod ws;

use api::ApiClient;
use app::{App, Mode};
use cli::Cli;
use components::{help::Help, input::Input, projects::Projects, tasks::Tasks};
use models::{Project, Task, WsEvent};
use ws::WebSocketClient;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    errors::init()?;
    logging::init()?;

    let args = Cli::parse();
    let mut app = App::new(args.tick_rate, args.frame_rate, &args.server)?;
    app.run().await?;
    Ok(())
}
