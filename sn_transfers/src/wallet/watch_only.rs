// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    keys::{get_main_pubkey, store_new_pubkey},
    local_store::WalletExclusiveAccess,
    wallet_file::{
        get_wallet, load_created_cash_note, store_created_cash_notes, store_wallet,
        wallet_lockfile_name,
    },
    KeyLessWallet,
};

use crate::{
    transfers::create_unsigned_transfer, wallet::data_payments::PaymentDetails, CashNote,
    DerivationIndex, Hash, MainPubkey, NanoTokens, UniquePubkey, UnsignedTransfer,
};
#[cfg(not(target_arch = "wasm32"))]
use fs2::FileExt;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};
use xor_name::XorName;

const PAYMENTS_DIR_NAME: &str = "payments";

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

    pub fn get_payment_transaction(&self, chunk_name: &XorName) -> Result<PaymentDetails> {
        let created_payments_dir = self.wallet_dir.join(PAYMENTS_DIR_NAME);
        let unique_file_name = format!("{}.payment", hex::encode(*chunk_name));
        let payment_file_path = created_payments_dir.join(unique_file_name);

        debug!("Getting payment from {payment_file_path:?}");
        let file = fs::File::open(&payment_file_path)?;
        let payment = rmp_serde::from_read(&file)?;

        Ok(payment)
    }

    pub fn insert_payment_transaction(
        &self,
        chunk_name: XorName,
        payment: PaymentDetails,
    ) -> Result<()> {
        let created_payments_dir = self.wallet_dir.join(PAYMENTS_DIR_NAME);
        let unique_file_name = format!("{}.payment", hex::encode(chunk_name));

        fs::create_dir_all(&created_payments_dir)?;

        let payment_file_path = created_payments_dir.join(unique_file_name);
        debug!("Writing payment to {payment_file_path:?}");

        let mut file = fs::File::create(payment_file_path)?;
        let mut serialiser = rmp_serde::encode::Serializer::new(&mut file);
        payment.serialize(&mut serialiser)?;
        Ok(())
    }

    pub fn remove_payment_transaction(&self, chunk_name: &XorName) {
        let created_payments_dir = self.wallet_dir.join(PAYMENTS_DIR_NAME);
        let unique_file_name = format!("{}.payment", hex::encode(*chunk_name));
        let payment_file_path = created_payments_dir.join(unique_file_name);

        debug!("Removing payment from {payment_file_path:?}");
        let _ = fs::remove_file(payment_file_path);
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
    pub fn deposit<'a, T>(&mut self, received_cash_notes: T) -> Result<()>
    where
        T: IntoIterator<Item = &'a CashNote>,
    {
        for cash_note in received_cash_notes {
            let id = cash_note.unique_pubkey();

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

        std::fs::create_dir_all(&self.wallet_dir)?;

        // lock and load from disk to make sure we're up to date and others can't modify the wallet concurrently
        let exclusive_access = self.lock()?;
        self.reload()?;
        trace!("Wallet locked and loaded!");

        for cash_note in received_cash_notes {
            let id = cash_note.unique_pubkey();

            if cash_note.derived_pubkey(&self.main_pubkey).is_err() {
                debug!("skipping: cash_note is not our key");
                continue;
            }

            let value = cash_note.value()?;
            self.keyless_wallet.available_cash_notes.insert(id, value);

            store_created_cash_notes([cash_note], &self.wallet_dir)?;
        }

        self.store(exclusive_access)
    }

    /// Reloads the wallet from disk.
    /// Note: this will drop any data held in memory and completely replaced with what's read fom disk.
    pub fn reload(&mut self) -> Result<()> {
        *self = Self::load_from(&self.wallet_dir, self.main_pubkey)?;
        Ok(())
    }

    /// Attempts to reload the wallet from disk.
    pub fn reload_from_disk_or_recreate(&mut self) -> Result<()> {
        std::fs::create_dir_all(&self.wallet_dir)?;
        let _exclusive_access = self.lock()?;
        self.reload()?;
        Ok(())
    }

    /// Return UniquePubkeys of cash_notes we own that are not yet spent.
    pub fn available_cash_notes(&self) -> &BTreeMap<UniquePubkey, NanoTokens> {
        &self.keyless_wallet.available_cash_notes
    }

    /// Remove referenced CashNotes from available_cash_notes
    pub fn mark_notes_as_spent<'a, T>(&mut self, unique_pubkeys: T)
    where
        T: IntoIterator<Item = &'a UniquePubkey>,
    {
        for k in unique_pubkeys {
            self.keyless_wallet.available_cash_notes.remove(k);
        }
    }

    pub fn build_unsigned_transaction(
        &mut self,
        to: Vec<(NanoTokens, MainPubkey)>,
        reason_hash: Option<Hash>,
    ) -> Result<UnsignedTransfer> {
        let mut rng = &mut rand::rngs::OsRng;
        // create a unique key for each output
        let to_unique_keys: Vec<_> = to
            .into_iter()
            .map(|(amount, address)| (amount, address, DerivationIndex::random(&mut rng)))
            .collect();

        trace!("Trying to lock wallet to get available cash_notes...");
        // lock and load from disk to make sure we're up to date and others can't modify the wallet concurrently
        let exclusive_access = self.lock()?;
        self.reload()?;
        trace!("Wallet locked and loaded!");

        // get the available cash_notes
        let mut available_cash_notes = vec![];
        let wallet_dir = self.wallet_dir().to_path_buf();
        for (id, _token) in self.available_cash_notes().iter() {
            if let Some(cash_note) = load_created_cash_note(id, &wallet_dir) {
                available_cash_notes.push((cash_note.clone(), None));
            } else {
                warn!("Skipping CashNote {:?} because we don't have it", id);
            }
        }
        debug!(
            "Available CashNotes for local send: {:#?}",
            available_cash_notes
        );

        let reason_hash = reason_hash.unwrap_or_default();

        let unsigned_transfer = create_unsigned_transfer(
            available_cash_notes,
            to_unique_keys,
            self.address(),
            reason_hash,
        )?;

        trace!("Releasing wallet lock"); // by dropping exclusive_access
        std::mem::drop(exclusive_access);

        Ok(unsigned_transfer)
    }

    // Helpers

    // Stores the wallet to disk.
    // This requires having exclusive access to the wallet to prevent concurrent processes from writing to it
    pub(super) fn store(&self, exclusive_access: WalletExclusiveAccess) -> Result<()> {
        store_wallet(&self.wallet_dir, &self.keyless_wallet)?;
        trace!("Releasing wallet lock");
        std::mem::drop(exclusive_access);
        Ok(())
    }

    // Locks the wallet and returns exclusive access to the wallet
    // This lock prevents any other process from locking the wallet dir, effectively acts as a mutex for the wallet
    pub(super) fn lock(&self) -> Result<WalletExclusiveAccess> {
        let lock = wallet_lockfile_name(&self.wallet_dir);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock)?;

        #[cfg(not(target_arch = "wasm32"))]
        file.lock_exclusive()?;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::WatchOnlyWallet;
    use crate::{
        genesis::{create_first_cash_note_from_key, GENESIS_CASHNOTE_AMOUNT},
        wallet::KeyLessWallet,
        MainSecretKey, NanoTokens,
    };
    use assert_fs::TempDir;
    use eyre::Result;

    #[test]
    fn watchonly_wallet_basics() -> Result<()> {
        let main_sk = MainSecretKey::random();
        let main_pubkey = main_sk.main_pubkey();
        let wallet_dir = TempDir::new()?;
        let wallet = WatchOnlyWallet::new(main_pubkey, &wallet_dir, KeyLessWallet::default());

        assert_eq!(wallet_dir.path(), wallet.wallet_dir());
        assert_eq!(main_pubkey, wallet.address());
        assert_eq!(NanoTokens::zero(), wallet.balance());
        assert!(wallet.available_cash_notes().is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn watchonly_wallet_to_and_from_file() -> Result<()> {
        let main_sk = MainSecretKey::random();
        let main_pubkey = main_sk.main_pubkey();
        let cash_note = create_first_cash_note_from_key(&main_sk)?;
        let wallet_dir = TempDir::new()?;

        let mut wallet = WatchOnlyWallet::new(main_pubkey, &wallet_dir, KeyLessWallet::default());
        wallet.deposit_and_store_to_disk(&vec![cash_note])?;

        let deserialised = WatchOnlyWallet::load_from(&wallet_dir, main_pubkey)?;

        assert_eq!(deserialised.wallet_dir(), wallet.wallet_dir());
        assert_eq!(deserialised.address(), wallet.address());

        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deserialised.balance().as_nano());

        assert_eq!(1, wallet.available_cash_notes().len());
        assert_eq!(1, deserialised.available_cash_notes().len());
        assert_eq!(
            deserialised.available_cash_notes(),
            wallet.available_cash_notes()
        );

        Ok(())
    }

    #[tokio::test]
    async fn watchonly_wallet_deposit_cash_notes() -> Result<()> {
        let main_sk = MainSecretKey::random();
        let main_pubkey = main_sk.main_pubkey();
        let wallet_dir = TempDir::new()?;
        let mut wallet = WatchOnlyWallet::new(main_pubkey, &wallet_dir, KeyLessWallet::default());

        // depositing owned cash note shall be deposited and increase the balance
        let owned_cash_note = create_first_cash_note_from_key(&main_sk)?;
        wallet.deposit(&vec![owned_cash_note.clone()])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());

        // depositing non-owned cash note shall be dropped and not change the balance
        let non_owned_cash_note = create_first_cash_note_from_key(&MainSecretKey::random())?;
        wallet.deposit(&vec![non_owned_cash_note])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());

        // depositing is idempotent
        wallet.deposit(&vec![owned_cash_note])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn watchonly_wallet_reload() -> Result<()> {
        let main_sk = MainSecretKey::random();
        let main_pubkey = main_sk.main_pubkey();
        let wallet_dir = TempDir::new()?;
        let mut wallet = WatchOnlyWallet::new(main_pubkey, &wallet_dir, KeyLessWallet::default());

        let cash_note = create_first_cash_note_from_key(&main_sk)?;
        wallet.deposit(&vec![cash_note.clone()])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());

        wallet.reload()?;
        assert_eq!(NanoTokens::zero(), wallet.balance());

        wallet.deposit_and_store_to_disk(&vec![cash_note])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());
        wallet.reload()?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());

        Ok(())
    }
}
