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

use crate::{
    cli::Opt,
    subcommands::{
        files::files_cmds,
        gossipsub::gossipsub_cmds,
        register::register_cmds,
        wallet::{wallet_cmds, wallet_cmds_without_client, WalletCmds},
        SubCmd,
    },
};
use bls::SecretKey;
use clap::Parser;
use color_eyre::Result;
use sn_client::Client;
#[cfg(feature = "metrics")]
use sn_logging::{metrics::init_metrics, LogBuilder, LogFormat};
use sn_peers_acquisition::parse_peers_args;
use sn_transfers::bls_secret_from_hex;
use std::path::PathBuf;
use tracing::Level;

const CLIENT_KEY: &str = "clientkey";

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let opt = Opt::parse();
    let _log_appender_guard = if let Some(log_output_dest) = opt.log_output_dest {
        let logging_targets = vec![
            // TODO: Reset to nice and clean defaults once we have a better idea of what we want
            ("sn_networking".to_string(), Level::DEBUG),
            ("safe".to_string(), Level::TRACE),
            ("sn_build_info".to_string(), Level::TRACE),
            ("sn_cli".to_string(), Level::TRACE),
            ("sn_client".to_string(), Level::TRACE),
            ("sn_logging".to_string(), Level::TRACE),
            ("sn_peers_acquisition".to_string(), Level::TRACE),
            ("sn_protocol".to_string(), Level::TRACE),
            ("sn_registers".to_string(), Level::TRACE),
            ("sn_transfers".to_string(), Level::TRACE),
        ];
        let mut log_builder = LogBuilder::new(logging_targets);
        log_builder.output_dest(log_output_dest);
        log_builder.format(opt.log_format.unwrap_or(LogFormat::Default));
        log_builder.initialize()?
    } else {
        None
    };
    #[cfg(feature = "metrics")]
    tokio::spawn(init_metrics(std::process::id()));

    // Log the full command that was run
    info!("\"{}\"", std::env::args().collect::<Vec<_>>().join(" "));

    debug!("Built with git version: {}", sn_build_info::git_info());
    println!("Built with git version: {}", sn_build_info::git_info());

    let client_data_dir_path = get_client_data_dir_path()?;
    // Perform actions that do not require us connecting to the network and return early
    if let SubCmd::Wallet(cmds) = &opt.cmd {
        if let WalletCmds::Address
        | WalletCmds::Balance { .. }
        | WalletCmds::Deposit { .. }
        | WalletCmds::Create { .. } = cmds
        {
            wallet_cmds_without_client(cmds, &client_data_dir_path).await?;
            return Ok(());
        }
    }

    println!("Instantiating a SAFE client...");
    let secret_key = get_client_secret_key(&client_data_dir_path)?;

    let bootstrap_peers = parse_peers_args(opt.peers).await?;

    println!("Connecting to the network w/peers:");
    for peer in &bootstrap_peers {
        println!("{peer}");
    }

    let bootstrap_peers = if bootstrap_peers.is_empty() {
        // empty vec is returned if `local-discovery` flag is provided
        None
    } else {
        Some(bootstrap_peers)
    };

    // use gossipsub only for the wallet cmd that requires it.
    let joins_gossipsub = matches!(opt.cmd, SubCmd::Wallet(WalletCmds::ReceiveOnline { .. }));

    let client = Client::new(
        secret_key,
        bootstrap_peers,
        joins_gossipsub,
        opt.connection_timeout,
    )
    .await?;

    // default to verifying storage
    let should_verify_store = !opt.no_verify;

    match opt.cmd {
        SubCmd::Wallet(cmds) => {
            wallet_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Files(cmds) => {
            files_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Register(cmds) => {
            register_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Gossipsub(cmds) => gossipsub_cmds(cmds, &client).await?,
    };

    Ok(())
}

fn get_client_secret_key(root_dir: &PathBuf) -> Result<SecretKey> {
    // create the root directory if it doesn't exist
    std::fs::create_dir_all(root_dir)?;
    let key_path = root_dir.join(CLIENT_KEY);
    let secret_key = if key_path.is_file() {
        info!("Client key found. Loading from file...");
        let secret_hex_bytes = std::fs::read(key_path)?;
        bls_secret_from_hex(secret_hex_bytes)?
    } else {
        info!("No key found. Generating a new client key...");
        let secret_key = SecretKey::random();
        std::fs::write(key_path, hex::encode(secret_key.to_bytes()))?;
        secret_key
    };
    Ok(secret_key)
}

fn get_client_data_dir_path() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::data_dir().expect("Data directory is obtainable");
    home_dirs.push("safe");
    home_dirs.push("client");
    std::fs::create_dir_all(home_dirs.as_path())?;
    Ok(home_dirs)
}
