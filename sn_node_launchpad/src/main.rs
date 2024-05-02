// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod action;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod mode;
pub mod tui;
pub mod utils;

#[macro_use]
extern crate tracing;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::Result;
use tokio::task::LocalSet;

use crate::{
    app::App,
    utils::{initialize_logging, initialize_panic_handler},
};

async fn tokio_main() -> Result<()> {
    initialize_logging()?;

    initialize_panic_handler()?;

    let args = Cli::parse();
    let mut app = App::new(args.tick_rate, args.frame_rate)?;
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
