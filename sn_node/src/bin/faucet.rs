// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::{Parser, Subcommand};
use eyre::Result;
use sn_client::{get_tokens_from_faucet, load_faucet_wallet, Client};
use sn_dbc::Token;
use sn_logging::init_logging;
use sn_peers_acquisition::peers_from_opts_or_env;
use sn_transfers::wallet::parse_public_address;
use tracing::info;
use tracing_core::Level;

#[tokio::main]
async fn main() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard = init_logging(logging_targets, &None)?;

    let opt = Opt::parse();

    info!("Instantiating a SAFE Test Faucet...");

    let secret_key = bls::SecretKey::random();
    let peers = peers_from_opts_or_env(&[])?;
    let client = Client::new(secret_key, Some(peers)).await?;

    faucet_cmds(opt.cmd, &client).await?;

    Ok(())
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
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
}

async fn faucet_cmds(cmds: SubCmd, client: &Client) -> Result<()> {
    match cmds {
        SubCmd::ClaimGenesis => {
            let _wallet = load_faucet_wallet(client).await;
        }
        SubCmd::Send { amount, to } => {
            let to = parse_public_address(to)?;
            use std::str::FromStr;
            let amount = Token::from_str(&amount)?;
            if amount.as_nano() == 0 {
                println!("Invalid format or zero amount passed in. Nothing sent.");
                return Ok(());
            }

            let dbc = get_tokens_from_faucet(amount, to, client).await;
            let dbc_hex = dbc.to_hex()?;
            println!("{dbc_hex}");
        }
    }
    Ok(())
}
