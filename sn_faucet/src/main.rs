// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod faucet_server;

#[cfg(feature = "distribution")]
mod token_distribution;

use clap::{Parser, Subcommand};
use color_eyre::eyre::{bail, eyre, Result};
use faucet_server::{restart_faucet_server, run_faucet_server};
use indicatif::ProgressBar;
use sn_client::{
    get_tokens_from_faucet, load_faucet_wallet_from_genesis_wallet, Client, ClientEvent,
    ClientEventsBroadcaster, ClientEventsReceiver,
};
use sn_logging::{Level, LogBuilder, LogOutputDest};
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_transfers::{get_faucet_data_dir, MainPubkey, NanoTokens, Transfer};
use std::{path::PathBuf, time::Duration};
use tokio::{sync::broadcast::error::RecvError, task::JoinHandle};
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();

    let bootstrap_peers = get_peers_from_args(opt.peers).await?;
    let bootstrap_peers = if bootstrap_peers.is_empty() {
        // empty vec is returned if `local-discovery` flag is provided
        None
    } else {
        Some(bootstrap_peers)
    };

    let logging_targets = vec![
        // TODO: Reset to nice and clean defaults once we have a better idea of what we want
        ("faucet".to_string(), Level::TRACE),
        ("sn_client".to_string(), Level::TRACE),
        ("sn_faucet".to_string(), Level::TRACE),
        ("sn_networking".to_string(), Level::DEBUG),
        ("sn_build_info".to_string(), Level::TRACE),
        ("sn_logging".to_string(), Level::TRACE),
        ("sn_peers_acquisition".to_string(), Level::TRACE),
        ("sn_protocol".to_string(), Level::TRACE),
        ("sn_registers".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::TRACE),
    ];

    let mut log_builder = LogBuilder::new(logging_targets);
    log_builder.output_dest(opt.log_output_dest);
    let _log_handles = log_builder.initialize()?;

    debug!("Built with git version: {}", sn_build_info::git_info());
    info!("Instantiating a SAFE Test Faucet...");

    let secret_key = bls::SecretKey::random();
    let broadcaster = ClientEventsBroadcaster::default();
    let (progress_bar, handle) = spawn_connection_progress_bar(broadcaster.subscribe());
    let result = Client::new(secret_key, bootstrap_peers, None, Some(broadcaster)).await;
    let client = match result {
        Ok(client) => client,
        Err(err) => {
            // clean up progress bar
            progress_bar.finish_with_message("Could not connect to the network");
            error!("Failed to get Client with err {err:?}");
            return Err(err.into());
        }
    };
    handle.await?;

    if let Err(err) = faucet_cmds(opt.cmd.clone(), &client).await {
        error!("Failed to run faucet cmd {:?} with err {err:?}", opt.cmd)
    }

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

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
    /// Specify the logging output destination.
    ///
    /// Valid values are "stdout", "data-dir", or a custom path.
    ///
    /// `data-dir` is the default value.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/client/logs
    ///  - macOS: $HOME/Library/Application Support/safe/client/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\client\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, value_parser = parse_log_output, verbatim_doc_comment, default_value = "data-dir")]
    pub log_output_dest: LogOutputDest,

    #[command(flatten)]
    peers: PeersArgs,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug, Clone)]
enum SubCmd {
    /// Claim the amount in the genesis CashNote and deposit it to the faucet local wallet.
    /// This needs to be run before a testnet is opened to the public, as to not have
    /// the genesis claimed by someone else (the key and cash_note are public for audit).
    ClaimGenesis,
    Send {
        /// This shall be the number of nanos to send.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `MainPubkey`.
        #[clap(name = "to")]
        to: String,
    },
    /// Starts an http server that will send tokens to anyone who requests them.
    /// curl http://localhost:8000/your-hex-encoded-wallet-public-address
    Server,
    /// Restart the faucet_server from the last breaking point.
    ///
    /// Before firing this cmd, ensure:
    ///   1, The previous faucet_server has been stopped.
    ///   2, Invalid cash_notes have been removed from the cash_notes folder.
    ///   3, The old `wallet` and `wallet.lock` files shall also be removed.
    /// The command will create a new wallet with the same key,
    /// then deposit all valid cash_notes into wallet and startup the faucet_server.
    RestartServer,
}

async fn faucet_cmds(cmds: SubCmd, client: &Client) -> Result<()> {
    match cmds {
        SubCmd::ClaimGenesis => {
            claim_genesis(client).await?;
        }
        SubCmd::Send { amount, to } => {
            send_tokens(client, &amount, &to).await?;
        }
        SubCmd::Server => {
            // shouldn't return except on error
            run_faucet_server(client).await?;
        }
        SubCmd::RestartServer => {
            // shouldn't return except on error
            restart_faucet_server(client).await?;
        }
    }
    Ok(())
}

async fn claim_genesis(client: &Client) -> Result<()> {
    for i in 1..6 {
        if let Err(e) = load_faucet_wallet_from_genesis_wallet(client).await {
            println!("Failed to claim genesis: {e}");
        } else {
            println!("Genesis claimed!");
            return Ok(());
        }
        println!("Trying to claiming genesis... attempt {i}");
    }
    bail!("Failed to claim genesis")
}

/// returns the hex-encoded transfer
async fn send_tokens(client: &Client, amount: &str, to: &str) -> Result<String> {
    let to = MainPubkey::from_hex(to)?;
    use std::str::FromStr;
    let amount = NanoTokens::from_str(amount)?;
    if amount.as_nano() == 0 {
        println!("Invalid format or zero amount passed in. Nothing sent.");
        return Err(eyre!(
            "Invalid format or zero amount passed in. Nothing sent."
        ));
    }

    let cash_note = get_tokens_from_faucet(amount, to, client).await?;
    let transfer_hex = Transfer::transfer_from_cash_note(&cash_note)?.to_hex()?;
    println!("{transfer_hex}");

    Ok(transfer_hex)
}

fn parse_log_output(val: &str) -> Result<LogOutputDest> {
    match val {
        "stdout" => Ok(LogOutputDest::Stdout),
        "data-dir" => {
            let dir = get_faucet_data_dir().join("logs");
            Ok(LogOutputDest::Path(dir))
        }
        // The path should be a directory, but we can't use something like `is_dir` to check
        // because the path doesn't need to exist. We can create it for the user.
        value => Ok(LogOutputDest::Path(PathBuf::from(value))),
    }
}
