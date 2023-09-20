// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use sn_client::{Client, Files, WalletClient};
use sn_transfers::wallet::{parse_main_pubkey, LocalWallet};
use sn_transfers::Nano;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use url::Url;
use xor_name::XorName;

use super::files::chunk_path;

// Defines the size of batch for the parallel uploading of chunks and correspondent payments.
pub(crate) const BATCH_SIZE: usize = 20;

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser, Debug)]
pub enum WalletCmds {
    /// Print the wallet address.
    Address,
    /// Print the wallet balance.
    Balance {
        /// Instead of checking CLI local wallet balance, the PeerId of a node can be used
        /// to check the balance of its rewards local wallet. Multiple ids can be provided
        /// in order to read the balance of multiple nodes at once.
        #[clap(long)]
        peer_id: Vec<String>,
    },
    /// Deposit CashNotes from the received directory to the local wallet.
    /// Or Read a hex encoded CashNote from stdin.
    ///
    /// The default received directory is platform specific:
    ///  - Linux: $HOME/.local/share/safe/wallet/received_cash_notes
    ///  - macOS: $HOME/Library/Application Support/safe/wallet/received_cash_notes
    ///  - Windows: C:\Users\{username}\AppData\Roaming\safe\wallet\received_cash_notes
    ///
    /// If you find the default path unwieldy, you can also set the RECEIVED_CASHNOTES_PATH environment
    /// variable to a path you would prefer to work with.
    #[clap(verbatim_doc_comment)]
    Deposit {
        /// Read a hex encoded CashNote from stdin.
        #[clap(long, default_value = "false")]
        stdin: bool,
        /// The hex encoded CashNote.
        #[clap(long)]
        cash_note: Option<String>,
    },
    /// Get tokens from a faucet.
    GetFaucet {
        /// The http url of the faucet to get tokens from.
        #[clap(name = "url")]
        url: String,
    },
    /// Send a CashNote.
    Send {
        /// The number of SafeNetworkTokens to send.
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
        /// The batch_size to split chunks into parallely handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE)]
        batch_size: usize,
    },
}

pub(crate) async fn wallet_cmds_without_client(cmds: &WalletCmds, root_dir: &Path) -> Result<()> {
    match cmds {
        WalletCmds::Address => address(root_dir),
        WalletCmds::Balance { peer_id } => {
            if peer_id.is_empty() {
                let balance = balance(root_dir)?;
                println!("{balance}");
            } else {
                let default_node_dir_path = dirs_next::data_dir()
                    .ok_or_else(|| eyre!("Failed to obtain data directory path"))?
                    .join("safe")
                    .join("node");

                for id in peer_id {
                    let path = default_node_dir_path.join(id);
                    let rewards = balance(&path)?;
                    println!("Node's rewards wallet balance (PeerId: {id}): {rewards}");
                }
            }
            Ok(())
        }
        WalletCmds::Deposit { stdin, cash_note } => deposit(root_dir, *stdin, cash_note.clone()),
        WalletCmds::GetFaucet { url } => get_faucet(root_dir, url.clone()).await,
        cmd => Err(eyre!("{cmd:?} requires us to be connected to the Network")),
    }
}

pub(crate) async fn wallet_cmds(
    cmds: WalletCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        WalletCmds::Send { amount, to } => send(amount, to, client, root_dir, verify_store).await?,
        WalletCmds::Pay {
            path,
            batch_size: _,
        } => {
            let chunked_files = chunk_path(client, root_dir, &path).await?;

            let all_chunks: Vec<_> = chunked_files
                .values()
                .flat_map(|chunked_file| &chunked_file.chunks)
                .map(|(n, _)| *n)
                .collect();

            let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());
            // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
            file_api.pay_for_chunks(all_chunks, verify_store).await?;
        }
        cmd => {
            return Err(eyre!(
                "{cmd:?} has to be processed before connecting to the network"
            ))
        }
    }
    Ok(())
}

fn address(root_dir: &Path) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir)?;
    let address_hex = hex::encode(wallet.address().to_bytes());
    println!("{address_hex}");
    Ok(())
}

fn balance(root_dir: &Path) -> Result<Nano> {
    let wallet = LocalWallet::try_load_from(root_dir)?;
    let balance = wallet.balance();
    Ok(balance)
}

async fn get_faucet(root_dir: &Path, url: String) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir)?;
    let address_hex = hex::encode(wallet.address().to_bytes());
    let url = if !url.contains("://") {
        format!("{}://{}", "http", url)
    } else {
        url
    };
    let req_url = Url::parse(&format!("{}/{}", url, address_hex))?;
    println!("Requesting token for wallet address: {address_hex}...");

    let response = reqwest::get(req_url).await?;
    let is_ok = response.status().is_success();
    let body = response.text().await?;
    if is_ok {
        deposit_from_cash_note_hex(root_dir, body)?;
        println!("Successfully got tokens from faucet.");
    } else {
        println!(
            "Failed to get tokens from faucet, server responded with: {}",
            body
        );
    }
    Ok(())
}

fn deposit(root_dir: &Path, read_from_stdin: bool, cash_note: Option<String>) -> Result<()> {
    if read_from_stdin {
        return read_cash_note_from_stdin(root_dir);
    }

    if let Some(cash_note_hex) = cash_note {
        return deposit_from_cash_note_hex(root_dir, cash_note_hex);
    }

    let mut wallet = LocalWallet::load_from(root_dir)?;

    let previous_balance = wallet.balance();

    wallet.try_load_deposits()?;

    let deposited =
        sn_transfers::Nano::from(wallet.balance().as_nano() - previous_balance.as_nano());
    if deposited.is_zero() {
        println!("Nothing deposited.");
    } else if let Err(err) = wallet.store() {
        println!("Failed to store deposited ({deposited}) amount: {:?}", err);
    } else {
        println!("Deposited {deposited}.");
    }

    Ok(())
}

fn read_cash_note_from_stdin(root_dir: &Path) -> Result<()> {
    println!("Please paste your CashNote below:");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    deposit_from_cash_note_hex(root_dir, input)
}

fn deposit_from_cash_note_hex(root_dir: &Path, input: String) -> Result<()> {
    let mut wallet = LocalWallet::load_from(root_dir)?;
    let cash_note = sn_transfers::CashNote::from_hex(input.trim())?;

    let old_balance = wallet.balance();
    wallet.deposit(&vec![cash_note])?;
    let new_balance = wallet.balance();
    wallet.store()?;

    println!("Successfully stored cash_note to wallet dir. \nOld balance: {old_balance}\nNew balance: {new_balance}");

    Ok(())
}

async fn send(
    amount: String,
    to: String,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    let address = parse_main_pubkey(to)?;

    use std::str::FromStr;
    let amount = Nano::from_str(&amount)?;
    if amount.as_nano() == 0 {
        println!("Invalid format or zero amount passed in. Nothing sent.");
        return Ok(());
    }

    let wallet = LocalWallet::load_from(root_dir)?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);

    match wallet_client.send(amount, address, verify_store).await {
        Ok(new_cash_note) => {
            println!("Sent {amount:?} to {address:?}");
            let mut wallet = wallet_client.into_wallet();
            let new_balance = wallet.balance();

            if let Err(err) = wallet.store() {
                println!("Failed to store wallet: {err:?}");
            } else {
                println!("Successfully stored wallet with new balance {new_balance}.");
            }

            wallet.store_cash_note(&new_cash_note)?;
            println!("Successfully stored new cash_note to wallet dir. It can now be sent to the recipient, using any channel of choice.");
        }
        Err(err) => {
            println!("Failed to send {amount:?} to {address:?} due to {err:?}.");
        }
    }

    Ok(())
}

pub(super) struct ChunkedFile {
    pub file_name: String,
    pub chunks: Vec<(XorName, PathBuf)>,
}
