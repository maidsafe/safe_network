/// initially generated from: https://github.com/ratatui-org/templates/blob/main/component/README.md
pub mod action;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod mode;
pub mod tui;
pub mod utils;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::Result;
use tokio::{runtime::Runtime, task::LocalSet};

use crate::{
    app::App,
    utils::{initialize_logging, initialize_panic_handler},
};

async fn tokio_main() -> Result<()> {
    initialize_logging()?;

    initialize_panic_handler()?;

    let args = Cli::parse();
    let mut app = App::new(args.tick_rate, args.frame_rate, args.peers)?;
    app.run().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Construct a local task set that can run `!Send` futures.
    let local = LocalSet::new();
    local
        .run_until(async {
            if let Err(e) = tokio_main().await {
                eprintln!("{} error: Something went wrong", env!("CARGO_PKG_NAME"));
                Err(e)
            } else {
                Ok(())
            }
        })
        .await
}
