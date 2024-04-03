// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    audit::audit,
    helpers::{get_faucet, receive, verify_spend_at},
    WalletApiHelper,
};
use crate::get_stdin_response;
use bls::SecretKey;
use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use dialoguer::Confirm;
use sn_client::transfers::{
    HotWallet, MainPubkey, MainSecretKey, NanoTokens, Transfer, TransferError, UnsignedTransfer,
    WalletError,
};
use sn_client::{Client, Error as ClientError};
use std::{path::Path, str::FromStr};

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
    /// Create a hot wallet from the given (hex-encoded) key.
    Create {
        /// Hex-encoded main secret key.
        #[clap(name = "key")]
        key: String,
    },
    /// Get tokens from a faucet.
    GetFaucet {
        /// The http url of the faucet to get tokens from.
        #[clap(name = "url")]
        url: String,
        /// The maidsafecoin address to claim. Leave blank to receive a fixed
        /// amount of tokens.
        maid_address: Option<String>,
        /// A signature of the safe wallet address, made by the maidsafecoin
        /// address.
        signature: Option<String>,
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
    /// Signs a transaction to be then broadcasted to the network.
    Sign {
        /// Hex-encoded unsigned transaction. It requires a hot-wallet was created for CLI.
        #[clap(name = "tx")]
        tx: String,
        /// Avoid prompts by assuming `yes` as the answer.
        #[clap(long, name = "force", default_value = "false")]
        force: bool,
    },
    /// Receive a transfer created by the 'send' or 'broadcast' command.
    Receive {
        /// Read the encrypted transfer from a file.
        #[clap(long, default_value = "false")]
        file: bool,
        /// Encrypted transfer.
        #[clap(name = "transfer")]
        transfer: String,
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
        /// EXPERIMENTAL redeem all royalties
        #[clap(long, default_value = "false")]
        royalties: bool,
    },
}

pub(crate) async fn wallet_cmds_without_client(cmds: &WalletCmds, root_dir: &Path) -> Result<()> {
    match cmds {
        WalletCmds::Address => {
            let wallet = WalletApiHelper::load_from(root_dir)?;
            println!("{:?}", wallet.address());
            Ok(())
        }
        WalletCmds::Balance { peer_id } => {
            if peer_id.is_empty() {
                let wallet = WalletApiHelper::load_from(root_dir)?;
                println!("{}", wallet.balance());
            } else {
                let default_node_dir_path = dirs_next::data_dir()
                    .ok_or_else(|| eyre!("Failed to obtain data directory path"))?
                    .join("safe")
                    .join("node");

                for id in peer_id {
                    let path = default_node_dir_path.join(id);
                    let rewards = WalletApiHelper::load_from(&path)?.balance();
                    println!("Node's rewards wallet balance (PeerId: {id}): {rewards}");
                }
            }
            Ok(())
        }
        WalletCmds::Deposit { stdin, cash_note } => {
            let mut wallet = WalletApiHelper::load_from(root_dir)?;
            wallet.deposit(*stdin, cash_note.as_deref())
        }
        WalletCmds::Create { key } => {
            let sk = SecretKey::from_hex(key)
                .map_err(|err| eyre!("Failed to parse hex-encoded SK: {err:?}"))?;
            let main_sk = MainSecretKey::new(sk);
            // TODO: encrypt wallet file
            // check for existing wallet with balance
            let existing_balance = match WalletApiHelper::load_from(root_dir) {
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
                let new_location = HotWallet::stash(root_dir)?;
                println!("Old wallet stored at {}", new_location.display());
            }
            // Create the new wallet with the new key
            let main_pubkey = main_sk.main_pubkey();
            let local_wallet = HotWallet::create_from_key(root_dir, main_sk)?;
            let balance = local_wallet.balance();
            println!(
                "Hot Wallet created (balance {balance}) for main public key: {main_pubkey:?}."
            );
            Ok(())
        }
        WalletCmds::Sign { tx, force } => sign_transaction(tx, root_dir, *force),
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
        WalletCmds::GetFaucet {
            url,
            maid_address,
            signature,
        } => get_faucet(root_dir, client, url.clone(), maid_address, signature).await,
        WalletCmds::Audit { dot, royalties } => audit(client, dot, royalties, root_dir).await,
        WalletCmds::Verify {
            spend_address,
            genesis,
        } => verify_spend_at(spend_address, genesis, client).await,
        cmd => Err(eyre!(
            "{cmd:?} has to be processed before connecting to the network"
        )),
    }
}

async fn send(
    amount: String,
    to: String,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    let from = HotWallet::load_from(root_dir)?;
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
            let wallet = HotWallet::load_from(root_dir)?;
            println!("Sent {amount:?} to {to:?}");
            println!("New wallet balance is {}.", wallet.balance());
            cash_note
        }
        Err(err) => {
            match err {
                ClientError::AmountIsZero => {
                    println!("Zero amount passed in. Nothing sent.");
                }
                ClientError::Wallet(WalletError::Transfer(TransferError::NotEnoughBalance(
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

fn sign_transaction(tx: &str, root_dir: &Path, force: bool) -> Result<()> {
    let wallet = HotWallet::load_from(root_dir)?;
    let unsigned_transfer: UnsignedTransfer = rmp_serde::from_slice(&hex::decode(tx)?)?;

    println!("The unsigned transaction has been successfully decoded:");
    let mut spent_tx = None;
    for (i, (spend, _)) in unsigned_transfer.spends.iter().enumerate() {
        println!("\nSpending input #{i}:");
        println!("\tKey: {}", spend.unique_pubkey.to_hex());
        println!("\tAmount: {}", spend.amount);
        if let Some(ref tx) = spent_tx {
            if tx != &spend.spent_tx {
                bail!("Transaction seems corrupted, not all Spends (inputs) refer to the same transaction");
            }
        } else {
            spent_tx = Some(spend.spent_tx.clone());
        }
    }

    if let Some(ref tx) = spent_tx {
        for (i, output) in tx.outputs.iter().enumerate() {
            println!("\nOutput #{i}:");
            println!("\tKey: {}", output.unique_pubkey.to_hex());
            println!("\tAmount: {}", output.amount);
        }
    } else {
        bail!("Transaction is corrupted, no transaction information found.");
    }

    if !force {
        println!("\n** Please make sure the above information is correct before signing it. **\n");
        let confirmation = Confirm::new()
            .with_prompt("Do you want to sign the above transaction?")
            .interact()?;

        if !confirmation {
            println!("Transaction not signed.");
            return Ok(());
        }
    }

    println!("Signing the transaction with local hot-wallet...");
    let signed_spends = wallet.sign(unsigned_transfer.spends);

    for signed_spend in signed_spends.iter() {
        if let Err(err) = signed_spend.verify(signed_spend.spent_tx_hash()) {
            bail!("Signature or transaction generated is invalid: {err:?}");
        }
    }

    println!(
        "The transaction has been successfully signed:\n\n{}\n",
        hex::encode(rmp_serde::to_vec(&(
            &signed_spends,
            unsigned_transfer.output_details,
            unsigned_transfer.change_id
        ))?)
    );
    println!(
        "Please copy the above text, and broadcast it to the network with 'wallet broadcast' cmd."
    );

    Ok(())
}
