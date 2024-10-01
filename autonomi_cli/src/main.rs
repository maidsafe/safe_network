// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod actions;
mod commands;
mod log_metrics;
mod opt;
mod utils;

use clap::Parser;
use color_eyre::Result;

use opt::Opt;

fn main() -> Result<()> {
    color_eyre::install().expect("Failed to initialise error handler");
    let opt = Opt::parse();
    log_metrics::init_logging_and_metrics(&opt).expect("Failed to initialise logging and metrics");

    // Log the full command that was run and the git version
    info!("\"{}\"", std::env::args().collect::<Vec<_>>().join(" "));
    let version = sn_build_info::git_info();
    info!("autonomi client built with git version: {version}");
    println!("autonomi client built with git version: {version}");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(commands::handle_subcommand(opt))
}
