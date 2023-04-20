// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    client::{Client, WalletClient},
    domain::wallet::{parse_public_address, DepositWallet, LocalWallet, Wallet},
};

use sn_dbc::{Dbc, Token};

use clap::Parser;
use eyre::Result;
use std::path::PathBuf;
use tokio::fs;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
pub enum WalletCmds {
    Deposit {
        /// Tries to load a hex encoded `Dbc` from the
        /// given path and deposit it to the wallet.
        #[clap(name = "dbc-dir")]
        dbc_dir: PathBuf,
    },
    Send {
        /// This shall be the number of nanos to send.
        /// Necessary if the `send_to` argument has been given.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `PublicAddress`.
        #[clap(name = "to")]
        to: String,
    },
}

pub(crate) async fn wallet_cmds(cmds: WalletCmds, client: &Client) -> Result<()> {
    match cmds {
        WalletCmds::Deposit { dbc_dir } => deposit(dbc_dir).await?,
        WalletCmds::Send { amount, to } => send(amount, to, client).await?,
    }
    Ok(())
}

async fn deposit(dbc_dir: PathBuf) -> Result<()> {
    let root_dir = get_client_dir().await?;
    let mut wallet = LocalWallet::load_from(&root_dir).await?;

    let mut deposits = vec![];

    for entry in WalkDir::new(dbc_dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file_name = entry.file_name();
            println!("Reading deposited tokens from {file_name:?}.");

            let dbc_data = fs::read_to_string(entry.path()).await?;
            let dbc = match Dbc::from_hex(dbc_data.trim()) {
                Ok(dbc) => dbc,
                Err(_) => {
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
            println!("Failed to store deposited amount: {:?}", err);
        } else {
            println!("Deposited {:?}.", sn_dbc::Token::from_nano(deposited));
        }
    } else {
        println!("Nothing deposited.");
    }

    Ok(())
}

async fn send(amount: String, to: String, client: &Client) -> Result<()> {
    let address = parse_public_address(to)?;

    use std::str::FromStr;
    let amount = Token::from_str(&amount)?;
    if amount.as_nano() == 0 {
        panic!("This should be unreachable. An amount is expected when sending.");
    }

    let root_dir = get_client_dir().await?;
    let wallet = LocalWallet::load_from(&root_dir).await?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);
    match wallet_client.send(amount, address).await {
        Ok(_new_dbcs) => {
            println!("Sent {amount:?} to {address:?}");
            let wallet = wallet_client.into_wallet();
            let new_balance = wallet.balance();

            if let Err(err) = wallet.store().await {
                println!("Failed to store wallet: {err:?}");
            } else {
                println!("Successfully stored wallet with new balance {new_balance:?}.");
            }
        }
        Err(err) => {
            println!("Failed to send {amount:?} to {address:?} due to {err:?}.");
        }
    }

    Ok(())
}

async fn get_client_dir() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("client");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}
