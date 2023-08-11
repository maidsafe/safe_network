// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod faucet_server;

use clap::{Parser, Subcommand};
use eyre::{eyre, Result};
use faucet_server::run_faucet_server;
use sn_client::{get_tokens_from_faucet, load_faucet_wallet_from_genesis_wallet, Client};
use sn_dbc::Token;
use sn_logging::{init_logging, LogFormat, LogOutputDest};
use sn_peers_acquisition::{parse_peer_addr, PeersArgs};
use sn_transfers::wallet::parse_public_address;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_core::Level;

#[tokio::main]
async fn main() -> Result<()> {
    let mut opt = Opt::parse();
    // This is only used for non-local-discocery,
    // i.e. make SAFE_PEERS always being a fall back option for initial peers.
    if !cfg!(feature = "local-discovery") {
        match std::env::var("SAFE_PEERS") {
            Ok(str) => match parse_peer_addr(&str) {
                Ok(peer) => {
                    if !opt
                        .peers
                        .peers
                        .iter()
                        .any(|existing_peer| *existing_peer == peer)
                    {
                        opt.peers.peers.push(peer);
                    }
                }
                Err(err) => error!("Can't parse SAFE_PEERS {str:?} with error {err:?}"),
            },
            Err(err) => error!("Can't get env var SAFE_PEERS with error {err:?}"),
        }
    }

    let _log_appender_guard = if let Some(log_output_dest) = opt.log_output_dest {
        let logging_targets = vec![
            ("safe".to_string(), Level::INFO),
            ("sn_client".to_string(), Level::INFO),
            ("sn_networking".to_string(), Level::INFO),
        ];
        init_logging(logging_targets, log_output_dest, LogFormat::Default)?
    } else {
        None
    };

    info!("Instantiating a SAFE Test Faucet...");

    let secret_key = bls::SecretKey::random();
    let client = Client::new(secret_key, Some(opt.peers.peers), None).await?;

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
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/client/logs
    ///  - macOS: $HOME/Library/Application Support/safe/client/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\client\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, value_parser = parse_log_output, verbatim_doc_comment)]
    log_output_dest: Option<LogOutputDest>,

    #[command(flatten)]
    peers: PeersArgs,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
enum SubCmd {
    /// Claim the amount in the genesis DBC and deposit it to the faucet local wallet.
    /// This needs to be run before a testnet is opened to the public, as to not have
    /// the genesis claimed by someone else (the key and dbc are public for audit).
    ClaimGenesis,
    Send {
        /// This shall be the number of nanos to send.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `PublicAddress`.
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
            claim_genesis(client).await;
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

async fn claim_genesis(client: &Client) {
    let _wallet = load_faucet_wallet_from_genesis_wallet(client).await;
}

/// returns the hex-encoded dbc
async fn send_tokens(client: &Client, amount: &str, to: &str) -> Result<String> {
    let to = parse_public_address(to)?;
    use std::str::FromStr;
    let amount = Token::from_str(amount)?;
    if amount.as_nano() == 0 {
        println!("Invalid format or zero amount passed in. Nothing sent.");
        return Err(eyre::eyre!(
            "Invalid format or zero amount passed in. Nothing sent."
        ));
    }

    let dbc = get_tokens_from_faucet(amount, to, client).await?;
    let dbc_hex = dbc.to_hex()?;
    println!("{dbc_hex}");

    Ok(dbc_hex)
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
