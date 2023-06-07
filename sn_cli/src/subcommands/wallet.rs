// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_client::{Client, Files, WalletClient};
use sn_dbc::{Dbc, Token};
use sn_transfers::{
    payment_proof::PaymentProofsMap,
    wallet::{parse_public_address, LocalWallet},
};

use bytes::Bytes;
use clap::Parser;
use color_eyre::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
pub enum WalletCmds {
    /// Print the address of the wallet.
    Address,
    /// Print the balance of the wallet.
    Balance,
    /// Deposit `Dbc`s to the local wallet.
    /// Tries to load any `Dbc`s from the `received_dbcs`
    /// path in the wallet dir, and deposit it to the wallet.
    /// The user has to manually place received dbc files to
    /// that dir, for example by choosing that path when downloading
    /// the dbc file from email or browser.
    Deposit,
    Send {
        /// This shall be the number of nanos to send.
        /// Necessary if the `to` argument has been given.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `PublicAddress`.
        #[clap(name = "to")]
        to: String,
    },
    Pay {
        /// The location of the files to pay the storage for.
        #[clap(name = "path", value_name = "DIRECTORY")]
        path: PathBuf,
    },
}

pub(crate) async fn wallet_cmds(cmds: WalletCmds, client: &Client, root_dir: &Path) -> Result<()> {
    match cmds {
        WalletCmds::Address => address(root_dir).await?,
        WalletCmds::Balance => balance(root_dir).await?,
        WalletCmds::Deposit => deposit(root_dir).await?,
        WalletCmds::Send { amount, to } => send(amount, to, client, root_dir).await?,
        WalletCmds::Pay { path } => pay_for_storage(client, root_dir, &path).await.map(|_| ())?,
    }
    Ok(())
}

async fn address(root_dir: &Path) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir).await?;
    let address_hex = hex::encode(wallet.address().to_bytes());
    println!("{address_hex}");
    Ok(())
}

async fn balance(root_dir: &Path) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir).await?;
    let balance = wallet.balance();
    println!("{balance}");
    Ok(())
}

async fn deposit(root_dir: &Path) -> Result<()> {
    let mut wallet = LocalWallet::load_from(root_dir).await?;

    let previous_balance = wallet.balance();

    wallet.try_load_deposits().await?;

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

async fn send(amount: String, to: String, client: &Client, root_dir: &Path) -> Result<()> {
    let address = parse_public_address(to)?;

    use std::str::FromStr;
    let amount = Token::from_str(&amount)?;
    if amount.as_nano() == 0 {
        println!("Invalid format or zero amount passed in. Nothing sent.");
        return Ok(());
    }

    let wallet = LocalWallet::load_from(root_dir).await?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);

    match wallet_client.send(amount, address).await {
        Ok(new_dbc) => {
            println!("Sent {amount:?} to {address:?}");
            let mut wallet = wallet_client.into_wallet();
            let new_balance = wallet.balance();

            if let Err(err) = wallet.store().await {
                println!("Failed to store wallet: {err:?}");
            } else {
                println!("Successfully stored wallet with new balance {new_balance:?}.");
            }

            wallet.store_created_dbc(new_dbc).await?;
            println!("Successfully stored new dbc to wallet dir. It can now be sent to the recipient, using any channel of choice.");
        }
        Err(err) => {
            println!("Failed to send {amount:?} to {address:?} due to {err:?}.");
        }
    }

    Ok(())
}

pub(super) async fn pay_for_storage(
    client: &Client,
    root_dir: &Path,
    files_path: &Path,
) -> Result<(Dbc, PaymentProofsMap)> {
    let wallet = LocalWallet::load_from(root_dir).await?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);
    let file_api: Files = Files::new(client.clone());

    // Get the list of Chunks addresses from the files found at 'files_path'
    let mut chunks_addrs = vec![];
    for entry in WalkDir::new(files_path).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            // we need all chunks addresses not just the data-map addr
            let (_, chunks) = file_api.chunk_bytes(bytes)?;
            chunks.iter().for_each(|c| chunks_addrs.push(*c.name()));
        }
    }

    println!("Making payment for {} Chunks...", chunks_addrs.len());
    let (new_dbc, proofs) = wallet_client.pay_for_storage(chunks_addrs.iter()).await?;

    let mut wallet = wallet_client.into_wallet();
    let new_balance = wallet.balance();

    if let Err(err) = wallet.store().await {
        println!("Failed to store wallet: {err:?}");
    } else {
        println!("Successfully stored wallet with new balance {new_balance:?}.");
    }

    let dbc_id = new_dbc.id();
    wallet.store_created_dbc(new_dbc.clone()).await?;
    println!("Successfully stored new dbc ({dbc_id:?}) to wallet dir. It can now be sent to the storage nodes when uploading paid chunks.");

    Ok((new_dbc, proofs))
}
