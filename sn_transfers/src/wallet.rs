// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! An implementation of a local Wallet used by clients and nodes (the latter use them for their rewards).
//! There is one which is deposit only, and one which can also send tokens.
//!
//! Later, a network Wallet store can be implemented thusly:
//! 1. Chunk each CashNote, both spent and available.
//! 2. For a semi-public Wallet:
//!     a. Store a register with address of your `MainPubkey`.
//!    Then push these ops:
//!     b. self.address.encrypt(Deposit(ChunkAddress))
//!     c. self.address.encrypt(Spend(ChunkAddress))
//!    And when the register has used 1023 entries:
//!     d. self.address.encrypt(Extend(RegisterAddress))
//!     ... which would occupy the last entry, and thus link to a new register.
//! 3. For a private Wallet:
//!     a. Store a register with address of self.address.encrypt(self.address).
//!     ... then follow from b. in 2.
//! 4. Then, when a wallet is to be loaded from the network:
//!     a. Get the `MainPubkey` from your secret.
//!     b. Fetch the register with address of either the plaintext of or the encrypted `MainPubkey`.
//!     c. Decrypt all entries and apply the ops to your Wallet, to get the current state of it.
//!     d. If there is another register linked at the end of this one, follow that link and repeat steps b., c. and d.
//!
//! We will already now pave for that, by mimicing that flow for the local storage of a Wallet.
//! First though, a simpler local storage will be used. But after that a local register store can be implemented.
//!
//! ************************************************************************************************************
//!
//! When the client spends a cash_note, ie signs the tx, the cash_note must be marked locally as spent (ie pending).
//! Only then should the client broadcast it.
//!
//! The client stores the tx as pending until either
//!     a) all nodes respond with spent so the client locally changes it from pending to spent or
//!     b) no nodes respond with spent so the client locally changes it to unspent.
//!
//! The best heuristic here is clients are in charge of their state, and the network is the source
//! of truth for the state.
//! If there’s ever a conflict in those states, the client can update their local state.
//! Clients create events (are in charge), nodes store events (are source of truth).
//!
//! The bitcoin flow here is very useful: unspent, unconfirmed (in mempool), confirmed.
//! These three states are held by both the client and the node, and is easy for the client to check and resolve.
//!
//! The most difficult situation for a bitcoin client to resolve is a low-fee tx in mempool for a long time,
//! which eventually clears from the mempool and becomes spendable again.
//!

mod api;
mod authentication;
mod data_payments;
mod encryption;
mod error;
mod hot_wallet;
mod keys;
mod wallet_file;
mod watch_only;

pub use self::{
    api::{WalletApi, WALLET_DIR_NAME},
    data_payments::{Payment, PaymentQuote, QuotingMetrics, QUOTE_EXPIRATION_SECS},
    error::{Error, Result},
    hot_wallet::HotWallet,
    keys::bls_secret_from_hex,
    wallet_file::wallet_lockfile_name,
    watch_only::WatchOnlyWallet,
};
pub(crate) use keys::store_new_keypair;

use crate::{NanoTokens, UniquePubkey};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use wallet_file::wallet_file_name;

#[derive(Default, Serialize, Deserialize)]
pub struct KeyLessWallet {
    available_cash_notes: BTreeMap<UniquePubkey, NanoTokens>,
}

impl KeyLessWallet {
    /// Returns `Some(KeyLessWallet)` or None if file doesn't exist.
    /// If the file is being written to, it will wait until the write is complete before reading.
    pub fn load_from(wallet_dir: &Path) -> Result<Option<Self>> {
        let path = wallet_file_name(wallet_dir);
        if !path.is_file() {
            return Ok(None);
        }

        let mut attempts = 0;
        let mut wallet: Option<Self> = None;

        // Attempt to read the file and deserialize it. If the file is currently being written to,
        // it will wait and try again. After 10 attempts, it will return an error.
        while wallet.is_none() && attempts < 10 {
            info!("Attempting to read wallet file");
            match fs::read(&path) {
                Ok(data) => match rmp_serde::from_slice(&data) {
                    Ok(deserialized_wallet) => wallet = Some(deserialized_wallet),
                    Err(_) => {
                        attempts += 1;
                        info!("Attempt {attempts} to read wallet file failed... ");
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    attempts += 1;
                    info!("Attempt {attempts} to read wallet file failed... ");
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => return Err(Error::from(e)),
            }
        }

        // If the file could not be read and deserialized after 10 attempts, return an error.
        if wallet.is_none() {
            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not read and deserialize wallet file after multiple attempts",
            )));
        }

        Ok(wallet)
    }

    pub fn balance(&self) -> NanoTokens {
        let mut balance = 0;
        for (_unique_pubkey, value) in self.available_cash_notes.iter() {
            balance += value.as_nano();
        }
        NanoTokens::from(balance)
    }
}
