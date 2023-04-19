// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    client::{Client, ClientEvent},
    log::init_node_logging,
    protocol::wallet::{DepositWallet, LocalWallet, Wallet},
};

use clap::Parser;
use dirs_next::home_dir;
use eyre::Result;
use sn_dbc::Dbc;
use std::{fs, path::PathBuf};
use tracing::{info, warn};
use walkdir::WalkDir;

#[derive(Parser, Debug, Clone)]
#[clap(name = "safeclient cli")]
struct Opt {
    /// The location of the wallet file.
    #[clap(long)]
    wallet_dir: Option<PathBuf>,
    /// The location of the log file.
    #[clap(long)]
    log_dir: Option<PathBuf>,
    /// Tries to load a hex encoded `Dbc` from the
    /// given path and deposit it to the wallet.
    #[clap(long)]
    deposit: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    info!("Instantiating a SAFE Wallet Client...");

    let secret_key = bls::SecretKey::random();
    let client = Client::new(secret_key)?;

    let mut client_events_rx = client.events_channel();
    if let Ok(event) = client_events_rx.recv().await {
        match event {
            ClientEvent::ConnectedToNetwork => {
                info!("Wallet Client connected to the Network");
            }
        }
    }

    wallet(&opt).await?;

    Ok(())
}

async fn wallet(opt: &Opt) -> Result<()> {
    let wallet_dir = opt.wallet_dir.clone().unwrap_or(get_client_dir().await?);
    let mut wallet = LocalWallet::load_from(&wallet_dir).await?;

    if let Some(deposit_path) = &opt.deposit {
        let mut deposits = vec![];

        for entry in WalkDir::new(deposit_path).into_iter().flatten() {
            if entry.file_type().is_file() {
                let file_name = entry.file_name();
                info!("Reading deposited tokens from {file_name:?}.");
                println!("Reading deposited tokens from {file_name:?}.");

                let dbc_data = fs::read_to_string(entry.path())?;
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
    }

    Ok(())
}

async fn get_client_dir() -> Result<PathBuf> {
    let mut home_dirs = home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("client");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}
