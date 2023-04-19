// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    client::{Client, WalletClient},
    protocol::wallet::{parse_public_address, DepositWallet, LocalWallet, Wallet},
};

use sn_dbc::{Dbc, Token};

use clap::Parser;
use eyre::Result;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
pub enum WalletCmds {
    Deposit {
        /// Tries to load a hex encoded `Dbc` from the
        /// given path and deposit it to the wallet.
        #[clap(name = "dbc-dir")]
        dbc_dir: PathBuf,
        /// The location of the wallet file.
        #[clap(name = "wallet-dir")]
        wallet_dir: PathBuf,
    },
    Send {
        /// This shall be the number of nanos to send.
        /// Necessary if the `send_to` argument has been given.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `PublicAddress`.
        #[clap(name = "to")]
        to: String,
        /// The location of the wallet file.
        #[clap(name = "wallet-dir")]
        wallet_dir: PathBuf,
    },
}

pub(crate) async fn wallet_cmds(cmds: WalletCmds, client: &Client) -> Result<()> {
    match cmds {
        WalletCmds::Deposit {
            dbc_dir,
            wallet_dir,
        } => deposit(dbc_dir, wallet_dir).await?,
        WalletCmds::Send {
            amount,
            to,
            wallet_dir,
        } => send(amount, to, wallet_dir, client).await?,
    }
    Ok(())
}

async fn deposit(dbc_dir: PathBuf, wallet_dir: PathBuf) -> Result<()> {
    let mut wallet = LocalWallet::load_from(&wallet_dir).await?;

    let mut deposits = vec![];

    for entry in WalkDir::new(dbc_dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file_name = entry.file_name();
            info!("Reading deposited tokens from {file_name:?}.");
            println!("Reading deposited tokens from {file_name:?}.");

            let dbc_data = fs::read_to_string(entry.path()).await?;
            let dbc = match Dbc::from_hex(dbc_data.trim()) {
                Ok(dbc) => dbc,
                Err(_) => {
                    warn!(
                        "This file does not appear to have valid hex-encoded DBC data. \
                        Skipping it."
                    );
                    println!(
                        "This file does not appear to have valid hex-encoded DBC data. \
                        Skipping it."
                    );
                    continue;
                }
            };

            deposits.push(dbc);
        }
    }

    let previous_balance = wallet.balance();
    wallet.deposit(deposits);
    let new_balance = wallet.balance();
    let deposited = previous_balance.as_nano() - new_balance.as_nano();

    if deposited > 0 {
        if let Err(err) = wallet.store().await {
            warn!("Failed to store deposited amount: {:?}", err);
            println!("Failed to store deposited amount: {:?}", err);
        } else {
            info!("Deposited {:?}.", sn_dbc::Token::from_nano(deposited));
            println!("Deposited {:?}.", sn_dbc::Token::from_nano(deposited));
        }
    } else {
        info!("Nothing deposited.");
        println!("Nothing deposited.");
    }

    Ok(())
}

async fn send(amount: String, to: String, wallet_dir: PathBuf, client: &Client) -> Result<()> {
    let address = parse_public_address(to)?;
    let amount = parse_tokens_amount(&amount);

    if amount.as_nano() == 0 {
        return Ok(());
    }

    let wallet = LocalWallet::load_from(&wallet_dir).await?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);
    match wallet_client.send(amount, address).await {
        Ok(_new_dbcs) => {
            info!("Sent {amount:?} to {address:?}");
            println!("Sent {amount:?} to {address:?}");
            let wallet = wallet_client.into_wallet();
            let new_balance = wallet.balance();

            if let Err(err) = wallet.store().await {
                warn!("Failed to store wallet: {err:?}");
                println!("Failed to store wallet: {err:?}");
            } else {
                info!("Successfully stored wallet with new balance {new_balance:?}.");
                println!("Successfully stored wallet with new balance {new_balance:?}.");
            }
        }
        Err(err) => {
            warn!("Failed to send {amount:?} to {address:?} due to {err:?}.");
            println!("Failed to send {amount:?} to {address:?} due to {err:?}.");
        }
    }

    Ok(())
}

fn parse_tokens_amount(amount_str: &str) -> Token {
    use std::str::FromStr;
    match Token::from_str(amount_str) {
        Ok(amount) => return amount,
        Err(err) => match err {
            sn_dbc::Error::ExcessiveTokenValue => {
                warn!("Invalid amount to send: {amount_str:?}, it exceeds the maximum possible value.");
                println!("Invalid amount to send: {amount_str:?}, it exceeds the maximum possible value.");
            }
            sn_dbc::Error::LossOfTokenPrecision => {
                warn!("Invalid amount to send: '{amount_str}', the minimum possible amount is one nano token (0.000000001).");
                println!("Invalid amount to send: '{amount_str}', the minimum possible amount is one nano token (0.000000001).");
            }
            sn_dbc::Error::FailedToParseToken(msg) => {
                warn!("Invalid amount to send: '{amount_str}': {msg}.");
                println!("Invalid amount to send: '{amount_str}': {msg}.");
            }
            other_err => {
                warn!("Invalid amount to send: '{amount_str}': {other_err:?}.");
                println!("Invalid amount to send: '{amount_str}': {other_err:?}.");
            }
        },
    }

    Token::from_nano(0)
}
