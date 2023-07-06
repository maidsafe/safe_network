// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod faucet_server;

use faucet_server::run_faucet_server;

use clap::{Parser, Subcommand};
use eyre::Result;
use sn_client::{get_tokens_from_faucet, load_faucet_wallet, Client};
use sn_dbc::Token;
use sn_peers_acquisition::PeersArgs;
use sn_transfers::wallet::parse_public_address;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();

    info!("Instantiating a SAFE Test Faucet...");

    let secret_key = bls::SecretKey::random();
    let client = Client::new(secret_key, Some(opt.peers.peers), None).await?;

    faucet_cmds(opt.cmd, &client).await?;

    Ok(())
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
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
    let _wallet = load_faucet_wallet(client).await;
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

    let dbc = get_tokens_from_faucet(amount, to, client).await;
    let dbc_hex = dbc.to_hex()?;
    println!("{dbc_hex}");

    Ok(dbc_hex)
}
