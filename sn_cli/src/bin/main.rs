// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod subcommands;

use subcommands::{
    files::files_cmds,
    folders::folders_cmds,
    register::register_cmds,
    wallet::{
        hot_wallet::{wallet_cmds, wallet_cmds_without_client, WalletCmds},
        wo_wallet::{wo_wallet_cmds, wo_wallet_cmds_without_client, WatchOnlyWalletCmds},
    },
    Opt, SubCmd,
};

use bls::SecretKey;
use clap::Parser;
use color_eyre::Result;
use indicatif::ProgressBar;
use sn_client::transfers::bls_secret_from_hex;
use sn_client::{Client, ClientEvent, ClientEventsBroadcaster, ClientEventsReceiver};
#[cfg(feature = "metrics")]
use sn_logging::{metrics::init_metrics, Level, LogBuilder, LogFormat};
use sn_peers_acquisition::get_peers_from_args;
use std::{io, path::PathBuf, time::Duration};
use tokio::{sync::broadcast::error::RecvError, task::JoinHandle};

const CLIENT_KEY: &str = "clientkey";

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let opt = Opt::parse();
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
    log_builder.output_dest(opt.log_output_dest);
    log_builder.format(opt.log_format.unwrap_or(LogFormat::Default));
    let _log_handles = log_builder.initialize()?;

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
        | WalletCmds::Create { .. }
        | WalletCmds::Sign { .. } = cmds
        {
            wallet_cmds_without_client(cmds, &client_data_dir_path).await?;
            return Ok(());
        }
    }

    if let SubCmd::WatchOnlyWallet(cmds) = &opt.cmd {
        if let WatchOnlyWalletCmds::Addresses
        | WatchOnlyWalletCmds::Balance { .. }
        | WatchOnlyWalletCmds::Deposit { .. }
        | WatchOnlyWalletCmds::Create { .. }
        | WatchOnlyWalletCmds::Transaction { .. } = cmds
        {
            wo_wallet_cmds_without_client(cmds, &client_data_dir_path).await?;
            return Ok(());
        }
    }

    println!("Instantiating a SAFE client...");
    let secret_key = get_client_secret_key(&client_data_dir_path)?;

    let bootstrap_peers = get_peers_from_args(opt.peers).await?;

    println!(
        "Connecting to the network with {} peers",
        bootstrap_peers.len(),
    );

    let bootstrap_peers = if bootstrap_peers.is_empty() {
        // empty vec is returned if `local-discovery` flag is provided
        None
    } else {
        Some(bootstrap_peers)
    };

    // get the broadcaster as we want to have our own progress bar.
    let broadcaster = ClientEventsBroadcaster::default();
    let (progress_bar, progress_bar_handler) =
        spawn_connection_progress_bar(broadcaster.subscribe());

    let result = Client::new(
        secret_key,
        bootstrap_peers,
        opt.connection_timeout,
        Some(broadcaster),
    )
    .await;
    let client = match result {
        Ok(client) => client,
        Err(err) => {
            // clean up progress bar
            progress_bar.finish_with_message("Could not connect to the network");
            return Err(err.into());
        }
    };
    progress_bar_handler.await?;

    // default to verifying storage
    let should_verify_store = !opt.no_verify;

    match opt.cmd {
        SubCmd::Wallet(cmds) => {
            wallet_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::WatchOnlyWallet(cmds) => {
            wo_wallet_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Files(cmds) => {
            files_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Folders(cmds) => {
            folders_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
        SubCmd::Register(cmds) => {
            register_cmds(cmds, &client, &client_data_dir_path, should_verify_store).await?
        }
    };

    Ok(())
}

/// Helper to subscribe to the client events broadcaster and spin up a progress bar that terminates when the
/// client successfully connects to the network or if it errors out.
fn spawn_connection_progress_bar(mut rx: ClientEventsReceiver) -> (ProgressBar, JoinHandle<()>) {
    // Network connection progress bar
    let progress_bar = ProgressBar::new_spinner();
    let progress_bar_clone = progress_bar.clone();
    progress_bar.enable_steady_tick(Duration::from_millis(120));
    progress_bar.set_message("Connecting to The SAFE Network...");
    let new_style = progress_bar.style().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈🔗");
    progress_bar.set_style(new_style);

    progress_bar.set_message("Connecting to The SAFE Network...");

    let handle = tokio::spawn(async move {
        let mut peers_connected = 0;
        loop {
            match rx.recv().await {
                Ok(ClientEvent::ConnectedToNetwork) => {
                    progress_bar.finish_with_message("Connected to the Network");
                    break;
                }
                Ok(ClientEvent::PeerAdded {
                    max_peers_to_connect,
                }) => {
                    peers_connected += 1;
                    progress_bar.set_message(format!(
                        "{peers_connected}/{max_peers_to_connect} initial peers found.",
                    ));
                }
                Err(RecvError::Lagged(_)) => {
                    // Even if the receiver is lagged, we would still get the ConnectedToNetwork during each new
                    // connection. Thus it would be okay to skip this error.
                }
                Err(RecvError::Closed) => {
                    progress_bar.finish_with_message("Could not connect to the network");
                    break;
                }
                _ => {}
            }
        }
    });
    (progress_bar_clone, handle)
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

fn get_stdin_response(prompt: &str) -> String {
    println!("{prompt}");
    let mut buffer = String::new();
    let stdin = io::stdin();
    if stdin.read_line(&mut buffer).is_err() {
        // consider if error should process::exit(1) here
        return "".to_string();
    };
    buffer
}
