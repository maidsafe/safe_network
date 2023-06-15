// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod cli;
mod subcommands;

use crate::cli::Opt;
use crate::subcommands::{files::files_cmds, register::register_cmds, wallet::wallet_cmds, SubCmd};
use sn_client::Client;
use sn_logging::init_logging;
#[cfg(feature = "metrics")]
use sn_logging::metrics::init_metrics;

use clap::Parser;
use color_eyre::Result;
use std::path::PathBuf;
use tracing_core::Level;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    // For client, default to log to std::out
    // This is ruining the log output for the CLI. Needs to be fixed.
    let tmp_dir = std::env::temp_dir();
    let logging_targets = vec![
        ("safe".to_string(), Level::INFO),
        ("sn_client".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
    ];
    let log_appender_guard = init_logging(logging_targets, &Some(tmp_dir.join("safe-client")))?;
    #[cfg(feature = "metrics")]
    tokio::spawn(init_metrics(std::process::id()));

    debug!("Built with git version: {}", sn_build_info::git_info());
    println!("Built with git version: {}", sn_build_info::git_info());
    info!("Full client logs will be written to {:?}", tmp_dir);
    println!("Instantiating a SAFE client...");

    let secret_key = bls::SecretKey::random();
    let root_dir = get_client_dir().await?;

    if opt.peers.peers.is_empty() {
        if !cfg!(feature = "local-discovery") {
            let log_str = "No peers given. As `local-discovery` feature is disabled, we will not be able to connect to the network.";
            warn!(log_str);
            return Err(color_eyre::eyre::eyre!(log_str));
        } else {
            info!("No peers given. As `local-discovery` feature is enabled, we will be attempt to connect to the network using mDNS.");
        }
    }

    let client = Client::new(secret_key, Some(opt.peers.peers), opt.timeout).await?;

    match opt.cmd {
        SubCmd::Wallet(cmds) => wallet_cmds(cmds, &client, &root_dir).await?,
        SubCmd::Files(cmds) => files_cmds(cmds, client.clone(), &root_dir).await?,
        SubCmd::Register(cmds) => register_cmds(cmds, &client).await?,
    };

    drop(log_appender_guard);
    Ok(())
}

async fn get_client_dir() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("client");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}
