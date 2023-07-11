// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_client::{Client, Files, PaymentProofsMap, WalletClient};
use sn_dbc::Token;
use sn_transfers::wallet::{parse_public_address, LocalWallet};

use bytes::Bytes;
use clap::Parser;
use color_eyre::Result;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use xor_name::XorName;

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser, Debug)]
pub enum WalletCmds {
    /// Print the wallet address.
    Address,
    /// Print the wallet balance.
    Balance,
    /// Deposit DBCs from the received directory to the local wallet.
    /// Or Read a hex encoded DBC from stdin.
    ///
    /// The default received directory is platform specific:
    ///  - Linux: $HOME/.local/share/safe/wallet/received_dbcs
    ///  - macOS: $HOME/Library/Application Support/safe/wallet/received_dbcs
    ///  - Windows: C:\Users\{username}\AppData\Roaming\safe\wallet\received_dbcs
    ///
    /// If you find the default path unwieldy, you can also set the RECEIVED_DBCS_PATH environment
    /// variable to a path you would prefer to work with.
    #[clap(verbatim_doc_comment)]
    Deposit {
        /// Read a hex encoded DBC from stdin.
        #[clap(long, default_value = "false")]
        stdin: bool,
    },
    /// Send a DBC.
    Send {
        /// The number of nanos to send.
        #[clap(name = "amount")]
        amount: String,
        /// Hex-encoded public address of the recipient.
        #[clap(name = "to")]
        to: String,
    },
    /// Make a payment for chunk storage based on files to be stored.
    ///
    /// Right now this command is highly experimental and doesn't really do anything functional.
    Pay {
        /// Location of the files to be stored.
        #[clap(name = "path", value_name = "DIRECTORY")]
        path: PathBuf,
    },
}

pub(crate) async fn wallet_cmds(cmds: WalletCmds, client: &Client, root_dir: &Path) -> Result<()> {
    match cmds {
        WalletCmds::Address => address(root_dir).await?,
        WalletCmds::Balance => balance(root_dir).await?,
        WalletCmds::Deposit { stdin } => deposit(root_dir, stdin).await?,
        WalletCmds::Send { amount, to } => send(amount, to, client, root_dir).await?,
        WalletCmds::Pay { path } => {
            chunk_and_pay_for_storage(client, root_dir, &path, true).await?;
        }
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

async fn deposit(root_dir: &Path, read_from_stdin: bool) -> Result<()> {
    if read_from_stdin {
        return read_dbc_from_stdin(root_dir).await;
    }

    let mut wallet = LocalWallet::load_from(root_dir).await?;

    let previous_balance = wallet.balance();

    wallet.try_load_deposits().await?;

    let deposited =
        sn_dbc::Token::from_nano(wallet.balance().as_nano() - previous_balance.as_nano());
    if deposited.is_zero() {
        println!("Nothing deposited.");
    } else if let Err(err) = wallet.store().await {
        println!("Failed to store deposited ({deposited}) amount: {:?}", err);
    } else {
        println!("Deposited {deposited}.");
    }

    Ok(())
}

async fn read_dbc_from_stdin(root_dir: &Path) -> Result<()> {
    let mut wallet = LocalWallet::load_from(root_dir).await?;

    println!("Please paste your DBC below:");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let dbc = sn_dbc::Dbc::from_hex(input.trim())?;

    let old_balance = wallet.balance();
    wallet.deposit(vec![dbc]);
    let new_balance = wallet.balance();
    wallet.store().await?;

    println!("Successfully stored dbc to wallet dir. \nOld balance: {old_balance}\nNew balance: {new_balance}");

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
                println!("Successfully stored wallet with new balance {new_balance}.");
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

pub(super) struct ChunkedFile {
    pub file_name: String,
    pub size: usize,
    pub chunks: Vec<(XorName, PathBuf)>,
}

pub(super) async fn chunk_and_pay_for_storage(
    client: &Client,
    root_dir: &Path,
    files_path: &Path,
    pay: bool, // TODO: to be removed; temporarily payment is optional
) -> Result<(BTreeMap<XorName, ChunkedFile>, PaymentProofsMap)> {
    let wallet = LocalWallet::load_from(root_dir).await?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);
    let file_api: Files = Files::new(client.clone());

    // Get the list of Chunks addresses from the files found at 'files_path'
    println!(
        "Preparing (chunking) files at '{}'...",
        files_path.display()
    );
    let chunks_dir = std::env::temp_dir();
    let mut files_to_pay = BTreeMap::new();
    let mut num_of_chunks = 0;
    for entry in WalkDir::new(files_path).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file_name = if let Some(file_name) = entry.file_name().to_str() {
                file_name.to_string()
            } else {
                println!(
                    "Skipping file {:?} as it is not valid UTF-8.",
                    entry.file_name()
                );
                continue;
            };

            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            // we need all chunks addresses not just the data-map addr
            let (file_addr, chunks) = file_api.chunk_bytes(bytes.clone())?;
            let mut chunks_paths = vec![];
            for c in chunks.iter() {
                num_of_chunks += 1;
                let xorname = *c.name();
                // let's store the chunk on temp file for the user
                // to be able to upload it to the network after making the payment,
                // without needing to chunk the files again.
                let path = chunks_dir.join(hex::encode(xorname));
                fs::write(&path, c.value())?;
                chunks_paths.push((xorname, path));
            }

            files_to_pay.insert(
                file_addr,
                ChunkedFile {
                    file_name,
                    size: bytes.len(),
                    chunks: chunks_paths,
                },
            );
        }
    }

    // For now, we make the payment for Chunks storage only if requested by the user
    let proofs = if pay {
        println!(
            "Making payment for {num_of_chunks} Chunks (belonging to {} files)...",
            files_to_pay.len()
        );

        let proofs = wallet_client
            .pay_for_storage(
                files_to_pay
                    .values()
                    .flat_map(|chunked_file| &chunked_file.chunks)
                    .map(|(name, _)| name),
            )
            .await?;

        let wallet = wallet_client.into_wallet();
        let new_balance = wallet.balance();

        if let Err(err) = wallet.store().await {
            println!("Failed to store wallet: {err:?}");
        } else {
            println!("Successfully stored wallet with new balance {new_balance}.");
        }

        println!("Successfully paid for storage and generated the proofs. They can now be sent to the storage nodes when uploading paid chunks.");
        proofs
    } else {
        PaymentProofsMap::default()
    };

    Ok((files_to_pay, proofs))
}
