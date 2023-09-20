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
use libp2p::Multiaddr;
use sn_client::Client;
#[cfg(feature = "metrics")]
use sn_logging::{init_logging, metrics::init_metrics, LogFormat};
use sn_transfers::wallet::bls_secret_from_hex;
use std::path::PathBuf;
use tracing::Level;

const CLIENT_KEY: &str = "clientkey";
// URL containing the multi-addresses of the bootstrap nodes
#[cfg(feature = "network-contacts")]
const NETWORK_CONTACTS_URL: &str = "PLACEHOLDER";

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let opt = Opt::parse();
    let _log_appender_guard = if let Some(log_output_dest) = opt.log_output_dest {
        let logging_targets = vec![
            ("safe".to_string(), Level::INFO),
            ("sn_client".to_string(), Level::INFO),
            ("sn_networking".to_string(), Level::INFO),
        ];
        init_logging(
            logging_targets,
            log_output_dest,
            opt.log_format.unwrap_or(LogFormat::Default),
        )?
    } else {
        None
    };
    #[cfg(feature = "metrics")]
    tokio::spawn(init_metrics(std::process::id()));

    debug!("Built with git version: {}", sn_build_info::git_info());
    println!("Built with git version: {}", sn_build_info::git_info());

    let client_data_dir_path = get_client_data_dir_path()?;
    // Perform actions that do not require us connecting to the network and return early
    if let SubCmd::Wallet(cmds) = &opt.cmd {
        if let WalletCmds::Address
        | WalletCmds::Balance { .. }
        | WalletCmds::Deposit { .. }
        | WalletCmds::GetFaucet { .. } = cmds
        {
            wallet_cmds_without_client(cmds, &client_data_dir_path).await?;
            return Ok(());
        }
    }
    println!("Instantiating a SAFE client...");
    let secret_key = get_client_secret_key(&client_data_dir_path)?;

    let peers: Option<Vec<Multiaddr>> = if opt.peers.peers.is_empty() {
        if cfg!(feature = "local-discovery") {
            info!("No peers given. As `local-discovery` feature is enabled, we will be attempt to connect to the network using mDNS.");
            None
        } else if cfg!(feature = "network-contacts") {
            info!("No peers given and `local-discovery` feature is disabled. As 'network-contacts' feature is enabled, trying to fetch the network contacts from s3.");
            #[cfg(feature = "network-contacts")]
            let peers = Some(get_bootstrap_peers_from_s3().await?);
            #[cfg(not(feature = "network-contacts"))]
            let peers = None;
            peers
        } else {
            let log_str = "No peers were passed using the '--peers' argument. And both the 'local-discovery' and 'network-contacts' flags are disabled. Thus we cannot connect to the network.";
            warn!(log_str);
            return Err(color_eyre::eyre::eyre!(log_str));
        }
    } else {
        Some(opt.peers.peers)
    };

    let client = Client::new(secret_key, peers, opt.timeout, opt.concurrency).await?;

    // default to verifying storage
    let should_verify_store = !opt.no_verify;

    match opt.cmd {
        SubCmd::Wallet(cmds) => {
            wallet_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Files(cmds) => {
            files_cmds(
                cmds,
                client.clone(),
                &client_data_dir_path,
                should_verify_store,
            )
            .await?
        }
        SubCmd::Register(cmds) => {
            register_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Gossipsub(cmds) => gossipsub_cmds(cmds, &client).await?,
    };

    Ok(())
}

/// Get bootstrap peers from the Network Contacts file stored in s3
/// This file is updated by Maidsafe and contains the contacts for the currently running "network"
#[cfg(feature = "network-contacts")]
async fn get_bootstrap_peers_from_s3() -> Result<Vec<Multiaddr>> {
    const MAX_RETRIES: usize = 3;

    let mut retries = 0;
    loop {
        let response = reqwest::get(url::Url::parse(NETWORK_CONTACTS_URL)?).await;

        match response {
            Ok(response) => {
                let mut multi_addresses = Vec::new();
                if response.status().is_success() {
                    // We store CSV of the multiaddr
                    for addr in response.text().await?.split(',') {
                        match sn_peers_acquisition::parse_peer_addr(addr) {
                            Ok(addr) => multi_addresses.push(addr),
                            Err(err) => {
                                error!("Failed to parse multi-address of {addr:?} with {err:?} from s3")
                            }
                        }
                    }
                    if !multi_addresses.is_empty() {
                        trace!("Successfully got bootstrap peers from s3 {multi_addresses:?}");
                        return Ok(multi_addresses);
                    } else {
                        return Err(color_eyre::eyre::eyre!(
                            "Could not obtain a single valid multi-addr from s3 {NETWORK_CONTACTS_URL}"
                        ));
                    }
                } else {
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(color_eyre::eyre::eyre!(
                            "Could not GET network contacts from {NETWORK_CONTACTS_URL} after {MAX_RETRIES} retries",
                        ));
                    }
                }
            }
            Err(err) => {
                retries += 1;
                if retries >= MAX_RETRIES {
                    return Err(color_eyre::eyre::eyre!(
                        "Failed to perform request to {NETWORK_CONTACTS_URL} after {MAX_RETRIES} retries due to: {err:?}"
                    ));
                }
            }
        }
        trace!("Failed to get bootstrap peers from s3, retrying {retries}/{MAX_RETRIES}");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
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
