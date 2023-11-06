// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod faucet_server;

use clap::{Parser, Subcommand};
use color_eyre::eyre::{bail, eyre, Result};
use faucet_server::run_faucet_server;
use sn_client::{get_tokens_from_faucet, load_faucet_wallet_from_genesis_wallet, Client};
use sn_logging::{LogBuilder, LogOutputDest};
use sn_peers_acquisition::{parse_peers_args, PeersArgs};
use sn_transfers::{parse_main_pubkey, NanoTokens, Transfer};
use std::path::PathBuf;
use tracing::info;
use tracing_core::Level;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();

    let bootstrap_peers = parse_peers_args(opt.peers).await?;
    let bootstrap_peers = if bootstrap_peers.is_empty() {
        // empty vec is returned if `local-discovery` flag is provided
        None
    } else {
        Some(bootstrap_peers)
    };

    let _log_appender_guard = if let Some(log_output_dest) = opt.log_output_dest {
        let logging_targets = vec![
            // TODO: Reset to nice and clean defaults once we have a better idea of what we want
            ("sn_networking".to_string(), Level::DEBUG),
            ("safenode".to_string(), Level::TRACE),
            ("sn_build_info".to_string(), Level::TRACE),
            ("sn_logging".to_string(), Level::TRACE),
            ("sn_node".to_string(), Level::TRACE),
            ("sn_peers_acquisition".to_string(), Level::TRACE),
            ("sn_protocol".to_string(), Level::TRACE),
            ("sn_registers".to_string(), Level::TRACE),
            ("sn_testnet".to_string(), Level::TRACE),
            ("sn_transfers".to_string(), Level::TRACE),
        ];
        let mut log_builder = LogBuilder::new(logging_targets);
        log_builder.output_dest(log_output_dest);
        log_builder.initialize()?
    } else {
        None
    };

    info!("Instantiating a SAFE Test Faucet...");

    let secret_key = bls::SecretKey::random();
    let client = Client::new(secret_key, bootstrap_peers, None).await?;

    faucet_cmds(opt.cmd, &client).await?;

    Ok(())
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
    pub log_output_dest: Option<LogOutputDest>,

    #[command(flatten)]
    peers: PeersArgs,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
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
    }
    Ok(())
}

async fn claim_genesis(client: &Client) -> Result<()> {
    for i in 1..6 {
        if let Err(e) = load_faucet_wallet_from_genesis_wallet(client).await {
            println!("Failed to claim genesis: {}", e);
        } else {
            println!("Genesis claimed!");
            return Ok(());
        }
        println!("Trying to claiming genesis... attempt {}", i);
    }
    bail!("Failed to claim genesis")
}

/// returns the hex-encoded transfer
async fn send_tokens(client: &Client, amount: &str, to: &str) -> Result<String> {
    let to = parse_main_pubkey(to)?;
    use std::str::FromStr;
    let amount = NanoTokens::from_str(amount)?;
    if amount.as_nano() == 0 {
        println!("Invalid format or zero amount passed in. Nothing sent.");
        return Err(eyre!(
            "Invalid format or zero amount passed in. Nothing sent."
        ));
    }

    let cash_note = get_tokens_from_faucet(amount, to, client).await?;
    let transfer_hex = Transfer::transfers_from_cash_note(cash_note)?.to_hex()?;
    println!("{transfer_hex}");

    Ok(transfer_hex)
}

fn parse_log_output(val: &str) -> Result<LogOutputDest> {
    match val {
        "stdout" => Ok(LogOutputDest::Stdout),
        "data-dir" => {
            let dir = dirs_next::data_dir()
                .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
                .join("safe")
                .join("test_faucet")
                .join("logs");
            Ok(LogOutputDest::Path(dir))
        }
        // The path should be a directory, but we can't use something like `is_dir` to check
        // because the path doesn't need to exist. We can create it for the user.
        value => Ok(LogOutputDest::Path(PathBuf::from(value))),
    }
}
