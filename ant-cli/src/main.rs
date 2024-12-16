// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod access;
mod actions;
mod commands;
mod opt;
mod utils;
mod wallet;

pub use access::data_dir;
pub use access::keys;
pub use access::network;
pub use access::user_data;

use clap::Parser;
use color_eyre::Result;

#[cfg(feature = "local")]
use ant_logging::metrics::init_metrics;
use ant_logging::{LogBuilder, LogFormat, ReloadHandle, WorkerGuard};
use opt::Opt;
use tracing::Level;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install().expect("Failed to initialise error handler");
    let opt = Opt::parse();
    if let Some(network_id) = opt.network_id {
        ant_protocol::version::set_network_id(network_id);
    }
    let _log_guards = init_logging_and_metrics(&opt)?;
    #[cfg(feature = "local")]
    tokio::spawn(init_metrics(std::process::id()));

    // Log the full command that was run and the git version
    info!("\"{}\"", std::env::args().collect::<Vec<_>>().join(" "));
    let version = ant_build_info::git_info();
    info!("autonomi client built with git version: {version}");
    println!("autonomi client built with git version: {version}");

    commands::handle_subcommand(opt).await?;

    Ok(())
}

fn init_logging_and_metrics(opt: &Opt) -> Result<(ReloadHandle, Option<WorkerGuard>)> {
    let logging_targets = vec![
        ("ant_bootstrap".to_string(), Level::DEBUG),
        ("ant_build_info".to_string(), Level::TRACE),
        ("ant_evm".to_string(), Level::TRACE),
        ("ant_networking".to_string(), Level::INFO),
        ("ant_registers".to_string(), Level::TRACE),
        ("autonomi_cli".to_string(), Level::TRACE),
        ("autonomi".to_string(), Level::TRACE),
        ("evmlib".to_string(), Level::TRACE),
        ("ant_logging".to_string(), Level::TRACE),
        ("ant_protocol".to_string(), Level::TRACE),
    ];
    let mut log_builder = LogBuilder::new(logging_targets);
    log_builder.output_dest(opt.log_output_dest.clone());
    log_builder.format(opt.log_format.unwrap_or(LogFormat::Default));
    let guards = log_builder.initialize()?;
    Ok(guards)
}
