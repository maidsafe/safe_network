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
use tracing::Level;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    if let Some(log_output_dest) = opt.log_output_dest {
        let logging_targets = vec![
            ("safe".to_string(), Level::INFO),
            ("sn_client".to_string(), Level::INFO),
            ("sn_networking".to_string(), Level::INFO),
        ];
        let _log_appender_guard = init_logging(logging_targets, log_output_dest, false)?;
    }
    #[cfg(feature = "metrics")]
    tokio::spawn(init_metrics(std::process::id()));

    debug!("Built with git version: {}", sn_build_info::git_info());
    println!("Built with git version: {}", sn_build_info::git_info());
    println!("Instantiating a SAFE client...");

    let secret_key = bls::SecretKey::random();
    let client_data_dir_path = get_client_data_dir_path().await?;

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
        SubCmd::Wallet(cmds) => wallet_cmds(cmds, &client, &client_data_dir_path).await?,
        SubCmd::Files(cmds) => files_cmds(cmds, client.clone(), &client_data_dir_path).await?,
        SubCmd::Register(cmds) => register_cmds(cmds, &client).await?,
    };

    Ok(())
}

async fn get_client_data_dir_path() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::data_dir().expect("Data directory is obtainable");
    home_dirs.push("safe");
    home_dirs.push("client");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}
