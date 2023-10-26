// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::files::ChunkManager;
use bls::SecretKey;
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use sn_client::{Client, Error as ClientError, Files};
use sn_transfers::{
    parse_main_pubkey, Error as TransferError, LocalWallet, MainSecretKey, NanoTokens, Transfer,
    WalletError,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};
use url::Url;

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
    ///  - Linux: $HOME/.local/share/safe/wallet/cash_notes
    ///  - macOS: $HOME/Library/Application Support/safe/wallet/cash_notes
    ///  - Windows: C:\Users\{username}\AppData\Roaming\safe\wallet\cash_notes
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
    /// Create a local wallet from the given (hex-encoded) Secret Key.
    Create {
        /// Hex-encoded main secret key
        #[clap(name = "sk")]
        sk: String,
    },
    /// Get tokens from a faucet.
    GetFaucet {
        /// The http url of the faucet to get tokens from.
        #[clap(name = "url")]
        url: String,
    },
    /// Send a transfer.
    ///
    /// This command will create a new transfer and encrypt it for the recipient.
    /// This encrypted transfer can then be shared with the recipient, who can then
    /// use the 'receive' command to claim the funds.
    Send {
        /// The number of SafeNetworkTokens to send.
        #[clap(name = "amount")]
        amount: String,
        /// Hex-encoded public address of the recipient.
        #[clap(name = "to")]
        to: String,
    },
    /// Receive a transfer created by the 'send' command.
    Receive {
        /// Read the encrypted transfer from a file.
        #[clap(long, default_value = "false")]
        file: bool,
        /// Encrypted transfer.
        #[clap(name = "transfer")]
        transfer: String,
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
        WalletCmds::Create { sk } => {
            let main_sk = match SecretKey::from_hex(sk) {
                Ok(sk) => MainSecretKey::new(sk),
                Err(err) => return Err(eyre!("Failed to parse hex-encoded SK: {err:?}")),
            };
            let main_pubkey = main_sk.main_pubkey();
            let local_wallet = LocalWallet::load_from_main_key(root_dir, main_sk)?;
            let balance = local_wallet.balance();
            println!("Wallet created (balance {balance}) for main public key: {main_pubkey:?}.");

            Ok(())
        }
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
        WalletCmds::Receive { file, transfer } => receive(transfer, file, client, root_dir).await?,
        WalletCmds::GetFaucet { url } => get_faucet(root_dir, client, url.clone()).await?,
        WalletCmds::Pay {
            path,
            batch_size: _,
        } => {
            let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());

            let mut manager = ChunkManager::new(root_dir, file_api.clone());
            manager.chunk_path(&path)?;

            let all_chunks: Vec<_> = manager
                .get_chunks()
                .iter()
                .map(|(xor_name, _)| *xor_name)
                .collect();

            file_api.pay_for_chunks(all_chunks).await?;
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

fn balance(root_dir: &Path) -> Result<NanoTokens> {
    let wallet = LocalWallet::try_load_from(root_dir)?;
    let balance = wallet.balance();
    Ok(balance)
}

async fn get_faucet(root_dir: &Path, client: &Client, url: String) -> Result<()> {
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
        receive(body, false, client, root_dir).await?;
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

    wallet.try_load_cash_notes()?;

    let deposited =
        sn_transfers::NanoTokens::from(wallet.balance().as_nano() - previous_balance.as_nano());
    if deposited.is_zero() {
        println!("Nothing deposited.");
    } else if let Err(err) = wallet.deposit_and_store_to_disk(&vec![]) {
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
    wallet.deposit_and_store_to_disk(&vec![cash_note])?;
    let new_balance = wallet.balance();
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
    let from = LocalWallet::load_from(root_dir)?;
    let amount = match NanoTokens::from_str(&amount) {
        Ok(amount) => amount,
        Err(_) => {
            println!("The amount cannot be parsed. Nothing sent.");
            return Ok(());
        }
    };
    let to = match parse_main_pubkey(to) {
        Ok(to) => to,
        Err(err) => {
            println!("Error while parsing the recipient's 'to' key: {err:?}");
            return Ok(());
        }
    };

    let cash_note = match sn_client::send(from, amount, to, client, verify_store).await {
        Ok(cash_note) => {
            let wallet = LocalWallet::load_from(root_dir)?;
            println!("Sent {amount:?} to {to:?}");
            println!("New wallet balance is {}.", wallet.balance());
            cash_note
        }
        Err(err) => {
            match err {
                ClientError::AmountIsZero => {
                    println!("Zero amount passed in. Nothing sent.");
                }
                ClientError::Transfers(WalletError::Transfer(TransferError::NotEnoughBalance(
                    available,
                    required,
                ))) => {
                    println!("Could not send due to low balance.\nBalance: {available:?}\nRequired: {required:?}");
                }
                _ => {
                    println!("Failed to send {amount:?} to {to:?} due to {err:?}.");
                }
            }
            return Ok(());
        }
    };

    let transfer = Transfer::transfers_from_cash_note(cash_note)?.to_hex()?;
    println!("The encrypted transfer has been successfully created.");
    println!("Please share this to the recipient:\n\n{transfer}\n");
    println!("The recipient can then use the 'receive' command to claim the funds.");

    Ok(())
}

async fn receive(transfer: String, is_file: bool, client: &Client, root_dir: &Path) -> Result<()> {
    let transfer = if is_file {
        std::fs::read_to_string(transfer)?.trim().to_string()
    } else {
        transfer
    };

    let transfer = match Transfer::from_hex(&transfer) {
        Ok(transfer) => transfer,
        Err(err) => {
            println!("Failed to parse transfer: {err:?}");
            println!("Transfer: \"{transfer}\"");
            return Ok(());
        }
    };
    println!("Successfully parsed transfer.");

    println!("Verifying transfer with the Network...");
    let mut wallet = LocalWallet::load_from(root_dir)?;
    let cashnotes = match client.receive(&transfer, &wallet).await {
        Ok(cashnotes) => cashnotes,
        Err(err) => {
            println!("Failed to verify and redeem transfer: {err:?}");
            return Ok(());
        }
    };
    println!("Successfully verified transfer.");

    let old_balance = wallet.balance();
    wallet.deposit_and_store_to_disk(&cashnotes)?;
    let new_balance = wallet.balance();

    println!("Successfully stored cash_note to wallet dir. \nOld balance: {old_balance}\nNew balance: {new_balance}");
    Ok(())
}
