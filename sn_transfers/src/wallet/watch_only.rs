// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    data_payments::PaymentDetails,
    error::{Error, Result},
    keys::{get_main_pubkey, store_new_pubkey},
    local_store::WalletExclusiveAccess,
    wallet_file::{get_wallet, store_created_cash_notes, store_wallet, wallet_lockfile_name},
    KeyLessWallet,
};

use crate::{CashNote, MainPubkey, NanoTokens, UniquePubkey};
use fs2::FileExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    path::{Path, PathBuf},
};
use xor_name::XorName;

#[derive(serde::Serialize, serde::Deserialize)]
/// This assumes the CashNotes are stored on disk
pub struct WatchOnlyWallet {
    /// Main public key which owns the cash notes.
    main_pubkey: MainPubkey,
    /// The dir of the wallet file, main key, public address, and new cash_notes.
    wallet_dir: PathBuf,
    /// The wallet containing all data, cash notes & transactions data that gets serialised and stored on disk.
    keyless_wallet: KeyLessWallet,
}

impl WatchOnlyWallet {
    #[cfg(test)]
    // Creates a new instance (only in memory) with provided info
    pub(super) fn new(
        main_pubkey: MainPubkey,
        wallet_dir: &Path,
        keyless_wallet: KeyLessWallet,
    ) -> Self {
        Self {
            main_pubkey,
            wallet_dir: wallet_dir.to_path_buf(),
            keyless_wallet,
        }
    }

    /// Loads a serialized wallet from a given path and main pub key.
    pub fn load_from(wallet_dir: &Path, main_pubkey: MainPubkey) -> Result<Self> {
        let main_pubkey = match get_main_pubkey(wallet_dir)? {
            Some(pk) if pk != main_pubkey => {
                return Err(Error::PubKeyMismatch(wallet_dir.to_path_buf()))
            }
            Some(pk) => pk,
            None => {
                warn!("No main pub key found when loading wallet from path, storing it now: {main_pubkey:?}");
                std::fs::create_dir_all(wallet_dir)?;
                store_new_pubkey(wallet_dir, &main_pubkey)?;
                main_pubkey
            }
        };
        let keyless_wallet = match get_wallet(wallet_dir)? {
            Some(keyless_wallet) => {
                debug!(
                    "Loaded wallet from {wallet_dir:#?} with balance {:?}",
                    keyless_wallet.balance()
                );
                keyless_wallet
            }
            None => {
                let keyless_wallet = KeyLessWallet::default();
                store_wallet(wallet_dir, &keyless_wallet)?;
                keyless_wallet
            }
        };

        Ok(Self {
            main_pubkey,
            wallet_dir: wallet_dir.to_path_buf(),
            keyless_wallet,
        })
    }

    pub fn address(&self) -> MainPubkey {
        self.main_pubkey
    }

    pub fn balance(&self) -> NanoTokens {
        self.keyless_wallet.balance()
    }

    pub fn wallet_dir(&self) -> &Path {
        &self.wallet_dir
    }

    /// Deposit the given cash_notes onto the wallet (without storing them to disk).
    pub fn deposit(&mut self, received_cash_notes: &Vec<CashNote>) -> Result<()> {
        for cash_note in received_cash_notes {
            let id = cash_note.unique_pubkey();

            if self.keyless_wallet.spent_cash_notes.contains(&id) {
                debug!("skipping: cash_note is spent");
                continue;
            }

            if cash_note.derived_pubkey(&self.main_pubkey).is_err() {
                debug!("skipping: cash_note is not our key");
                continue;
            }

            let value = cash_note.value()?;
            self.keyless_wallet.available_cash_notes.insert(id, value);
        }

        Ok(())
    }

    /// Store the given cash_notes to the `cash_notes` dir in the wallet dir.
    /// Update and store the updated wallet to disk
    /// This function locks the wallet to prevent concurrent processes from writing to it
    pub fn deposit_and_store_to_disk(&mut self, received_cash_notes: &Vec<CashNote>) -> Result<()> {
        if received_cash_notes.is_empty() {
            return Ok(());
        }

        // lock and load from disk to make sure we're up to date and others can't modify the wallet concurrently
        let exclusive_access = self.lock()?;
        self.reload()?;
        trace!("Wallet locked and loaded!");

        for cash_note in received_cash_notes {
            let id = cash_note.unique_pubkey();

            if self.keyless_wallet.spent_cash_notes.contains(&id) {
                debug!("skipping: cash_note is spent");
                continue;
            }

            if cash_note.derived_pubkey(&self.main_pubkey).is_err() {
                debug!("skipping: cash_note is not our key");
                continue;
            }

            let value = cash_note.value()?;
            self.keyless_wallet.available_cash_notes.insert(id, value);

            store_created_cash_notes(&[cash_note], &self.wallet_dir)?;
        }

        self.store(exclusive_access)
    }

    /// Attempts to reload the wallet from disk.
    pub fn reload_from_disk_or_recreate(&mut self) -> Result<()> {
        std::fs::create_dir_all(&self.wallet_dir)?;
        // lock and load from disk to make sure we're up to date and others can't modify the wallet concurrently
        trace!("Trying to lock wallet to get available cash_notes...");
        let _exclusive_access = self.lock()?;
        self.reload()?;
        Ok(())
    }

    /// Return UniquePubkeys of cash_notes we own that are not yet spent.
    pub fn available_cash_notes(&self) -> &BTreeMap<UniquePubkey, NanoTokens> {
        &self.keyless_wallet.available_cash_notes
    }

    ///
    pub fn available_cash_notes_mut(&mut self) -> &mut BTreeMap<UniquePubkey, NanoTokens> {
        &mut self.keyless_wallet.available_cash_notes
    }

    /// Return the set of UnniquePubjkey's of spent cash notes.
    pub fn spent_cash_notes(&self) -> &BTreeSet<UniquePubkey> {
        &self.keyless_wallet.spent_cash_notes
    }

    /// Insert provided UniquePubkey's into the set of spent cash notes.
    pub fn insert_spent_cash_notes<'a, T>(&mut self, spent_cash_notes: T)
    where
        T: IntoIterator<Item = &'a UniquePubkey>,
    {
        for pk in spent_cash_notes {
            let _ = self.keyless_wallet.spent_cash_notes.insert(*pk);
        }
    }

    ///
    pub fn cash_notes_created_for_others(&self) -> &BTreeSet<UniquePubkey> {
        &self.keyless_wallet.cash_notes_created_for_others
    }

    ///
    pub fn cash_notes_created_for_others_mut(&mut self) -> &mut BTreeSet<UniquePubkey> {
        &mut self.keyless_wallet.cash_notes_created_for_others
    }

    ///
    pub fn get_payment_transaction(&self, name: &XorName) -> Option<&PaymentDetails> {
        self.keyless_wallet.payment_transactions.get(name)
    }

    ///
    pub fn insert_payment_transaction(&mut self, name: XorName, payment: PaymentDetails) {
        self.keyless_wallet
            .payment_transactions
            .insert(name, payment);
    }

    // Helpers

    /// Stores the wallet to disk.
    /// This requires having exclusive access to the wallet to prevent concurrent processes from writing to it
    pub(super) fn store(&self, exclusive_access: WalletExclusiveAccess) -> Result<()> {
        store_wallet(&self.wallet_dir, &self.keyless_wallet)?;
        trace!("Releasing wallet lock");
        std::mem::drop(exclusive_access);
        Ok(())
    }

    /// Locks the wallet and returns exclusive access to the wallet
    /// This lock prevents any other process from locking the wallet dir, effectively acts as a mutex for the wallet
    pub(super) fn lock(&self) -> Result<WalletExclusiveAccess> {
        let lock = wallet_lockfile_name(&self.wallet_dir);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock)?;
        file.lock_exclusive()?;
        Ok(file)
    }

    /// reloads the wallet from disk.
    fn reload(&mut self) -> Result<()> {
        *self = Self::load_from(&self.wallet_dir, self.main_pubkey)?;
        Ok(())
    }
}
