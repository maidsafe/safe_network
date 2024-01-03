// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::get_stdin_response;
use bls::{PublicKey, SecretKey, PK_SIZE};
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use sn_client::{Client, ClientEvent, Error as ClientError};
use sn_transfers::{
    CashNoteRedemption, Error as TransferError, LocalWallet, MainPubkey, MainSecretKey, NanoTokens,
    SpendAddress, Transfer, UniquePubkey, WalletError, WatchOnlyWallet, GENESIS_CASHNOTE,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};
use url::Url;

const DEFAULT_RECEIVE_ONLINE_WALLET_DIR: &str = "receive_online";
const ROYALTY_TRANSFER_NOTIF_TOPIC: &str = "ROYALTY_TRANSFER_NOTIFICATION";

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
    /// DEPRECATED will be removed in future versions.
    /// Prefer using the send and receive commands instead.
    ///
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
    /// Create a hot or watch-only wallet from the given (hex-encoded) key.
    Create {
        /// Hex-encoded main secret or public key. If the key is a secret key a hot-wallet will be created
        /// which can be used to sign and broadcast transfers. Otherwise, if the passed key is a public key,
        /// then a watch-only wallet is created.
        #[clap(name = "key")]
        key: String,
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
    /// Builds an unsigned transaction to be signed offline.
    Transaction {
        /// The number of SafeNetworkTokens to transfer.
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
    /// Listen for transfer notifications from the network over gossipsub protocol.
    ///
    /// Transfers will be deposited to a local (watch-only) wallet.
    ///
    /// Only cash notes owned by the provided public key will be accepted, verified to be valid
    /// against the network, and deposited onto a locally stored watch-only wallet.
    ReceiveOnline {
        /// Hex-encoded main public key
        #[clap(name = "pk")]
        pk: String,
        /// Optional path where to store the wallet
        #[clap(name = "path")]
        path: Option<PathBuf>,
    },
    /// Verify a spend on the Network.
    Verify {
        /// The Network address or hex encoded UniquePubkey of the Spend to verify
        #[clap(name = "spend")]
        spend_address: String,
        /// Verify all the way to Genesis
        ///
        /// Used for auditing, note that this might take a very long time
        /// Analogous to verifying an UTXO through the entire blockchain in Bitcoin
        #[clap(long, default_value = "false")]
        genesis: bool,
    },
    /// Audit the Currency
    /// Note that this might take a very long time
    /// Analogous to verifying the entire blockchain in Bitcoin
    Audit {
        /// EXPERIMENTAL Dump Audit DAG in dot format on stdout
        #[clap(long, default_value = "false")]
        dot: bool,
        /// EXPERIMENTAL Find and redeem all Network Royalties
        /// only works if the wallet has the Network Royalties private key
        #[clap(long, default_value = "false")]
        royalties: bool,
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
        WalletCmds::Deposit { stdin, cash_note } => deposit(root_dir, *stdin, cash_note.as_deref()),
        WalletCmds::Create { key } => {
            match SecretKey::from_hex(key) {
                Ok(sk) => {
                    let main_sk = MainSecretKey::new(sk);
                    // TODO: encrypt wallet file
                    // check for existing wallet with balance
                    let existing_balance = match LocalWallet::load_from(root_dir) {
                        Ok(wallet) => wallet.balance(),
                        Err(_) => NanoTokens::zero(),
                    };
                    // if about to overwrite an existing balance, confirm operation
                    if existing_balance > NanoTokens::zero() {
                        let prompt = format!("Existing wallet has balance of {existing_balance}. Replace with new wallet? [y/N]");
                        let response = get_stdin_response(&prompt);
                        if response.trim() != "y" {
                            // Do nothing, return ok and prevent any further operations
                            println!("Exiting without creating new wallet");
                            return Ok(());
                        }
                        // remove existing wallet
                        let new_location = LocalWallet::clear(root_dir)?;
                        println!("Old wallet stored at {}", new_location.display());
                    }
                    // Create the new wallet with the new key
                    let main_pubkey = main_sk.main_pubkey();
                    let local_wallet = LocalWallet::create_from_key(root_dir, main_sk)?;
                    let balance = local_wallet.balance();
                    println!(
                        "Wallet created (balance {balance}) for main public key: {main_pubkey:?}."
                    );
                }
                Err(_err) => {
                    let main_pk = match PublicKey::from_hex(key) {
                        Ok(pk) => MainPubkey::new(pk),
                        Err(err) => return Err(eyre!("Failed to parse hex-encoded PK: {err:?}")),
                    };
                    let pk_hex = main_pk.to_hex();
                    let folder_name =
                        format!("pk_{}_{}", &pk_hex[..6], &pk_hex[pk_hex.len() - 6..]);
                    let wallet_dir = root_dir.join(folder_name);
                    let main_pubkey = main_pk.public_key();
                    let watch_only_wallet = WatchOnlyWallet::load_from(&wallet_dir, main_pk)?;
                    let balance = watch_only_wallet.balance();
                    println!("Watch-only wallet created (balance {balance}) for main public key: {main_pubkey:?}.");
                }
            };
            Ok(())
        }
        WalletCmds::Transaction { amount, to } => {
            build_unsigned_transaction(amount, to, root_dir).await
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
        WalletCmds::Send { amount, to } => send(amount, to, client, root_dir, verify_store).await,
        WalletCmds::Receive { file, transfer } => receive(transfer, file, client, root_dir).await,
        WalletCmds::GetFaucet { url } => get_faucet(root_dir, client, url.clone()).await,
        WalletCmds::ReceiveOnline { pk, path } => {
            let wallet_dir = path.unwrap_or(root_dir.join(DEFAULT_RECEIVE_ONLINE_WALLET_DIR));
            listen_notifs_and_deposit(&wallet_dir, client, pk).await
        }
        WalletCmds::Audit { dot, royalties } => audit(client, dot, royalties, root_dir).await,
        WalletCmds::Verify {
            spend_address,
            genesis,
        } => verify(spend_address, genesis, client).await,
        cmd => Err(eyre!(
            "{cmd:?} has to be processed before connecting to the network"
        )),
    }
}

fn parse_pubkey_address(str_addr: &str) -> Result<SpendAddress> {
    let pk_res = UniquePubkey::from_hex(str_addr);
    let addr_res = SpendAddress::from_hex(str_addr);

    match (pk_res, addr_res) {
        (Ok(pk), _) => Ok(SpendAddress::from_unique_pubkey(&pk)),
        (_, Ok(addr)) => Ok(addr),
        _ => Err(eyre!("Failed to parse address: {str_addr}")),
    }
}

/// Verify a spend on the Network.
/// if genesis is true, verify all the way to Genesis, note that this might take A VERY LONG TIME
async fn verify(spend_address: String, genesis: bool, client: &Client) -> Result<()> {
    if genesis {
        println!("Verifying spend all the way to Genesis, note that this might take a while...");
    } else {
        println!("Verifying spend...");
    }

    let addr = parse_pubkey_address(&spend_address)?;
    match client.verify_spend(addr, genesis).await {
        Ok(()) => println!("Spend verified to be stored and unique at {addr:?}"),
        Err(e) => println!("Failed to verify spend at {addr:?}: {e}"),
    }

    Ok(())
}

async fn audit(client: &Client, to_dot: bool, find_royalties: bool, root_dir: &Path) -> Result<()> {
    let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey());

    if to_dot {
        let dag = client.build_spend_dag_from(genesis_addr).await?;
        println!("{}", dag.dump_dot_format());
    } else {
        println!("Auditing the Currency, note that this might take a very long time...");
        client
            .follow_spend(genesis_addr, find_royalties, root_dir)
            .await?;
    }

    Ok(())
}

fn address(root_dir: &Path) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir)?;
    println!("{:?}", wallet.address());
    Ok(())
}

fn balance(root_dir: &Path) -> Result<NanoTokens> {
    let wallet = LocalWallet::try_load_from(root_dir)?;
    let balance = wallet.balance();
    Ok(balance)
}

async fn get_faucet(root_dir: &Path, client: &Client, url: String) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir)?;
    let address_hex = wallet.address().to_hex();
    let url = if !url.contains("://") {
        format!("{}://{}", "http", url)
    } else {
        url
    };
    let req_url = Url::parse(&format!("{url}/{address_hex}"))?;
    println!("Requesting token for wallet address: {address_hex}");

    let response = reqwest::get(req_url).await?;
    let is_ok = response.status().is_success();
    let body = response.text().await?;
    if is_ok {
        receive(body, false, client, root_dir).await?;
        println!("Successfully got tokens from faucet.");
    } else {
        println!("Failed to get tokens from faucet, server responded with: {body:?}");
    }
    Ok(())
}

fn deposit(root_dir: &Path, read_from_stdin: bool, cash_note: Option<&str>) -> Result<()> {
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
        println!("Failed to store deposited ({deposited}) amount: {err:?}");
    } else {
        println!("Deposited {deposited}.");
    }

    Ok(())
}

fn read_cash_note_from_stdin(root_dir: &Path) -> Result<()> {
    println!("Please paste your CashNote below:");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    deposit_from_cash_note_hex(root_dir, &input)
}

fn deposit_from_cash_note_hex(root_dir: &Path, input: &str) -> Result<()> {
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
        Err(err) => {
            println!("The amount cannot be parsed. Nothing sent.");
            return Err(err.into());
        }
    };
    let to = match MainPubkey::from_hex(to) {
        Ok(to) => to,
        Err(err) => {
            println!("Error while parsing the recipient's 'to' key: {err:?}");
            return Err(err.into());
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
            return Err(err.into());
        }
    };

    let transfer = Transfer::transfer_from_cash_note(&cash_note)?.to_hex()?;
    println!("The encrypted transfer has been successfully created.");
    println!("Please share this to the recipient:\n\n{transfer}\n");
    println!("The recipient can then use the 'receive' command to claim the funds.");

    Ok(())
}

async fn build_unsigned_transaction(amount: &str, to: &str, root_dir: &Path) -> Result<()> {
    let mut wallet = LocalWallet::load_from(root_dir)?;
    let amount = match NanoTokens::from_str(amount) {
        Ok(amount) => amount,
        Err(err) => {
            println!("The amount cannot be parsed. Nothing sent.");
            return Err(err.into());
        }
    };
    let to = match MainPubkey::from_hex(to) {
        Ok(to) => to,
        Err(err) => {
            println!("Error while parsing the recipient's 'to' key: {err:?}");
            return Err(err.into());
        }
    };

    let unsigned_spends = wallet.build_unsigned_transaction(vec![(amount, to)], None)?;

    println!(
        "The unsigned transaction has been successfully created:\n\n{}\n",
        hex::encode(rmp_serde::to_vec(&unsigned_spends)?)
    );
    println!("Please copy the above text, sign it offline with 'wallet sign' cmd, and then use the signed transaction to broadcast it with 'wallet broadcast' cmd.");

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
            return Err(err.into());
        }
    };
    println!("Successfully parsed transfer. ");

    println!("Verifying transfer with the Network...");
    let mut wallet = LocalWallet::load_from(root_dir)?;
    let cashnotes = match client.receive(&transfer, &wallet).await {
        Ok(cashnotes) => cashnotes,
        Err(err) => {
            println!("Failed to verify and redeem transfer: {err:?}");
            return Err(err.into());
        }
    };
    println!("Successfully verified transfer.");

    let old_balance = wallet.balance();
    wallet.deposit_and_store_to_disk(&cashnotes)?;
    let new_balance = wallet.balance();

    println!("Successfully stored cash_note to wallet dir. \nOld balance: {old_balance}\nNew balance: {new_balance}");
    Ok(())
}

async fn listen_notifs_and_deposit(root_dir: &Path, client: &Client, pk_hex: String) -> Result<()> {
    let mut wallet = match MainPubkey::from_hex(&pk_hex) {
        Ok(main_pk) => {
            let folder_name = format!("pk_{}_{}", &pk_hex[..6], &pk_hex[pk_hex.len() - 6..]);
            let wallet_dir = root_dir.join(folder_name);
            println!("Loading local wallet from: {}", wallet_dir.display());
            WatchOnlyWallet::load_from(&wallet_dir, main_pk)?
        }
        Err(err) => return Err(eyre!("Failed to parse hex-encoded public key: {err:?}")),
    };

    let main_pk = wallet.address();
    let pk = main_pk.public_key();

    client.subscribe_to_topic(ROYALTY_TRANSFER_NOTIF_TOPIC.to_string())?;
    let mut events_receiver = client.events_channel();

    println!("Current balance in local wallet: {}", wallet.balance());
    println!("Listening to transfers notifications for {pk:?}... (press Ctrl+C to exit)");
    println!();

    while let Ok(event) = events_receiver.recv().await {
        let cash_notes = match event {
            ClientEvent::GossipsubMsg { topic, msg } => {
                // we assume it's a notification of a transfer as that's the only topic we've subscribed to
                match try_decode_transfer_notif(&msg) {
                    Err(err) => {
                        println!("GossipsubMsg received on topic '{topic}' couldn't be decoded as transfer notif: {err:?}");
                        continue;
                    }
                    Ok((key, _)) if key != pk => continue,
                    Ok((key, cashnote_redemptions)) => {
                        println!("New transfer notification received for {key:?}, containing {} CashNoteRedemption/s.", cashnote_redemptions.len());
                        match client
                            .verify_cash_notes_redemptions(main_pk, &cashnote_redemptions)
                            .await
                        {
                            Err(err) => {
                                println!("At least one of the CashNoteRedemptions received is invalid, dropping them: {err:?}");
                                continue;
                            }
                            Ok(cash_notes) => cash_notes,
                        }
                    }
                }
            }
            _other_event => continue,
        };

        cash_notes.iter().for_each(|cn| {
            let value = match cn.value() {
                Ok(value) => value.to_string(),
                Err(err) => {
                    println!("Failed to obtain cash note value: {err}");
                    "unknown".to_string()
                }
            };
            println!(
                "CashNote received with {:?}, value: {value}",
                cn.unique_pubkey(),
            );
        });

        match wallet.deposit_and_store_to_disk(&cash_notes) {
            Ok(()) => {}
            Err(err @ WalletError::Io(_)) => {
                println!("ERROR: Failed to deposit the received cash notes: {err}");
                println!();
                println!("WARNING: we'll try to reload/recreate the local wallet now, but if it was corrupted there could have been lost funds.");
                println!();
                wallet.reload_from_disk_or_recreate()?;
                wallet.deposit_and_store_to_disk(&cash_notes)?;
            }
            Err(other_err) => return Err(other_err.into()),
        }

        println!(
            "New balance after depositing received CashNote/s: {}",
            wallet.balance()
        );
        println!();
    }

    Ok(())
}

fn try_decode_transfer_notif(msg: &[u8]) -> Result<(PublicKey, Vec<CashNoteRedemption>)> {
    let mut key_bytes = [0u8; PK_SIZE];
    key_bytes.copy_from_slice(
        msg.get(0..PK_SIZE)
            .ok_or_else(|| eyre!("msg doesn't have enough bytes"))?,
    );
    let key = PublicKey::from_bytes(key_bytes)?;
    let cashnote_redemptions: Vec<CashNoteRedemption> = rmp_serde::from_slice(&msg[PK_SIZE..])?;
    Ok((key, cashnote_redemptions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sn_transfers::SpendAddress;

    #[test]
    fn test_parse_pubkey_address() -> eyre::Result<()> {
        let public_key = SecretKey::random().public_key();
        let unique_pk = UniquePubkey::new(public_key);
        let spend_address = SpendAddress::from_unique_pubkey(&unique_pk);
        let addr_hex = spend_address.to_hex();
        let unique_pk_hex = unique_pk.to_hex();

        let addr = parse_pubkey_address(&addr_hex)?;
        assert_eq!(addr, spend_address);

        let addr2 = parse_pubkey_address(&unique_pk_hex)?;
        assert_eq!(addr2, spend_address);
        Ok(())
    }
}
