// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
use super::{
    keys::{get_main_key, store_new_keypair},
    wallet_file::{
        get_unconfirmed_spend_requests, get_wallet, load_cash_notes_from_disk,
        load_created_cash_note, store_created_cash_notes, store_unconfirmed_spend_requests,
        store_wallet, wallet_lockfile_name,
    },
    Error, KeyLessWallet, Result,
};

use crate::{
    transfers::{create_offline_transfer, ContentPaymentsIdMap, OfflineTransfer, PaymentDetails},
    CashNoteRedemption, SignedSpend, Transfer,
};
use crate::{
    CashNote, DerivationIndex, DerivedSecretKey, Hash, MainPubkey, MainSecretKey, NanoTokens,
    UniquePubkey,
};
use fs2::FileExt;
use itertools::Itertools;
use xor_name::XorName;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

const WALLET_DIR_NAME: &str = "wallet";

/// A locked file handle, that when dropped releases the lock.
pub type WalletExclusiveAccess = File;

/// A wallet that can only receive tokens.
pub struct LocalWallet {
    /// The secret key with which we can access
    /// all the tokens in the available_cash_notes.
    key: MainSecretKey,
    /// The wallet containing all data.
    wallet: KeyLessWallet,
    /// The dir of the wallet file, main key, public address, and new cash_notes.
    wallet_dir: PathBuf,
    /// These have not yet been successfully sent to the network
    /// and need to be, to reach network validity.
    unconfirmed_spend_requests: BTreeSet<SignedSpend>,
}

impl LocalWallet {
    /// Stores the wallet to disk.
    /// This requires having exclusive access to the wallet to prevent concurrent processes from writing to it
    fn store(&self, exclusive_access: WalletExclusiveAccess) -> Result<()> {
        store_wallet(&self.wallet_dir, &self.wallet)?;
        trace!("Releasing wallet lock");
        std::mem::drop(exclusive_access);
        Ok(())
    }

    /// reloads the wallet from disk.
    fn reload(&mut self) -> Result<()> {
        // placeholder random MainSecretKey to take it out
        let current_key = std::mem::replace(&mut self.key, MainSecretKey::random());
        let (key, wallet, unconfirmed_spend_requests) =
            load_from_path(&self.wallet_dir, Some(current_key))?;

        // and move the original back in
        *self = Self {
            key,
            wallet,
            wallet_dir: self.wallet_dir.to_path_buf(),
            unconfirmed_spend_requests,
        };
        Ok(())
    }

    /// Locks the wallet and returns exclusive access to the wallet
    /// This lock prevents any other process from locking the wallet dir, effectively acts as a mutex for the wallet
    pub fn lock(&self) -> Result<WalletExclusiveAccess> {
        let lock = wallet_lockfile_name(&self.wallet_dir);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock)?;
        file.lock_exclusive()?;
        Ok(file)
    }

    /// Stores the given cash_notes to the `created cash_notes dir` in the wallet dir.
    /// These can then be sent to the recipients out of band, over any channel preferred.
    pub fn store_cash_notes_to_disk(&self, cash_note: Vec<&CashNote>) -> Result<()> {
        store_created_cash_notes(cash_note, &self.wallet_dir)
    }

    /// Store unconfirmed_spend_requests to disk.
    pub fn store_unconfirmed_spend_requests(&mut self) -> Result<()> {
        store_unconfirmed_spend_requests(&self.wallet_dir, self.unconfirmed_spend_requests())
    }

    /// Remove CashNote from available_cash_notes and add it to spent_cash_notes.
    pub fn mark_note_as_spent(&mut self, cash_note_id: UniquePubkey) {
        self.wallet.available_cash_notes.remove(&cash_note_id);
        self.wallet.spent_cash_notes.insert(cash_note_id);
    }

    pub fn unconfirmed_spend_requests_exist(&self) -> bool {
        !self.unconfirmed_spend_requests.is_empty()
    }

    /// Try to load any new cash_notes from the `cash_notes dir` in the wallet dir.
    pub fn try_load_cash_notes(&mut self) -> Result<()> {
        let deposited = load_cash_notes_from_disk(&self.wallet_dir)?;
        self.deposit_and_store_to_disk(&deposited)?;
        Ok(())
    }

    /// Loads a serialized wallet from a path and given main key.
    pub fn load_from_main_key(root_dir: &Path, main_key: MainSecretKey) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        // This creates the received_cash_notes dir if it doesn't exist.
        std::fs::create_dir_all(&wallet_dir)?;
        // This creates the main_key file if it doesn't exist.
        let (key, wallet, unconfirmed_spend_requests) =
            load_from_path(&wallet_dir, Some(main_key))?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
            unconfirmed_spend_requests,
        })
    }

    /// Loads a serialized wallet from a path.
    pub fn load_from(root_dir: &Path) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        // This creates the received_cash_notes dir if it doesn't exist.
        std::fs::create_dir_all(&wallet_dir)?;
        let (key, wallet, unconfirmed_spend_requests) = load_from_path(&wallet_dir, None)?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
            unconfirmed_spend_requests,
        })
    }

    /// Tries to loads a serialized wallet from a path, bailing out if it doesn't exist.
    pub fn try_load_from(root_dir: &Path) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        let (key, wallet, unconfirmed_spend_requests) = load_from_path(&wallet_dir, None)?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
            unconfirmed_spend_requests,
        })
    }

    pub fn address(&self) -> MainPubkey {
        self.key.main_pubkey()
    }

    pub fn unconfirmed_spend_requests(&self) -> &BTreeSet<SignedSpend> {
        &self.unconfirmed_spend_requests
    }

    /// To remove a specific spend from the requests, if eg, we see one spend is _bad_
    pub fn clear_specific_spend_request(&mut self, unique_pub_key: UniquePubkey) {
        self.unconfirmed_spend_requests
            .retain(|signed_spend| signed_spend.spend.unique_pubkey.ne(&unique_pub_key))
    }

    pub fn clear_unconfirmed_spend_requests(&mut self) {
        self.unconfirmed_spend_requests = Default::default();
    }

    pub fn balance(&self) -> NanoTokens {
        self.wallet.balance()
    }

    pub fn sign(&self, msg: &[u8]) -> bls::Signature {
        self.key.sign(msg)
    }

    /// Returns all available cash_notes and an exclusive access to the wallet so no concurrent processes can
    /// get available cash_notes while we're modifying the wallet
    /// once the updated wallet is stored to disk it is safe to drop the WalletExclusiveAccess
    pub fn available_cash_notes(
        &mut self,
    ) -> Result<(Vec<(CashNote, DerivedSecretKey)>, WalletExclusiveAccess)> {
        trace!("Trying to lock wallet to get available cash_notes...");
        // lock and load from disk to make sure we're up to date and others can't modify the wallet concurrently
        let exclusive_access = self.lock()?;
        self.reload()?;
        trace!("Wallet locked and loaded!");

        // get the available cash_notes
        let mut available_cash_notes = vec![];
        for (id, _token) in self.wallet.available_cash_notes.iter() {
            let held_cash_note = load_created_cash_note(id, &self.wallet_dir);
            if let Some(cash_note) = held_cash_note {
                if let Ok(derived_key) = cash_note.derived_key(&self.key) {
                    available_cash_notes.push((cash_note.clone(), derived_key));
                } else {
                    warn!(
                        "Skipping CashNote {:?} because we don't have the key to spend it",
                        cash_note.unique_pubkey()
                    );
                }
            } else {
                warn!("Skipping CashNote {:?} because we don't have it", id);
            }
        }

        Ok((available_cash_notes, exclusive_access))
    }

    /// Return the payment cash_note ids for the given content address name if cached.
    pub fn get_payment_unique_pubkeys_and_values(
        &self,
        name: &XorName,
    ) -> Option<&Vec<PaymentDetails>> {
        self.wallet.payment_transactions.get(name)
    }

    /// Return the payment cash_note ids for the given content address name if cached.
    pub fn get_payment_cash_notes(&self, name: &XorName) -> Vec<CashNote> {
        let ids = self.get_payment_unique_pubkeys_and_values(name);
        // now grab all those cash_notes
        let mut cash_notes: Vec<CashNote> = vec![];

        if let Some(ids) = ids {
            for (id, _main_pub_key, _value) in ids {
                if let Some(cash_note) = load_created_cash_note(id, &self.wallet_dir) {
                    trace!(
                        "Current cash_note of chunk {name:?} is paying {:?} tokens.",
                        cash_note.value()
                    );
                    cash_notes.push(cash_note);
                }
            }
        }

        cash_notes
    }

    /// Make a transfer and return all created cash_notes
    pub fn local_send(
        &mut self,
        to: Vec<(NanoTokens, MainPubkey)>,
        reason_hash: Option<Hash>,
    ) -> Result<Vec<CashNote>> {
        let mut rng = &mut rand::rngs::OsRng;
        // create a unique key for each output
        let to_unique_keys: Vec<_> = to
            .into_iter()
            .map(|(amount, address)| {
                (
                    amount,
                    address,
                    UniquePubkey::random_derivation_index(&mut rng),
                )
            })
            .collect();

        let (available_cash_notes, exclusive_access) = self.available_cash_notes()?;
        debug!(
            "Available CashNotes for local send: {:#?}",
            available_cash_notes
        );

        let reason_hash = reason_hash.unwrap_or_default();

        let transfer = create_offline_transfer(
            available_cash_notes,
            to_unique_keys,
            self.address(),
            reason_hash,
        )?;

        let created_cash_notes = transfer.created_cash_notes.clone();

        self.update_local_wallet(transfer, exclusive_access)?;

        trace!("Releasing wallet lock"); // by dropping _exclusive_access
        Ok(created_cash_notes)
    }

    /// Performs a CashNote payment for each content address, returning outputs for each.
    pub fn local_send_storage_payment(
        &mut self,
        all_data_payments: BTreeMap<XorName, Vec<(MainPubkey, NanoTokens)>>,
        reason_hash: Option<Hash>,
    ) -> Result<()> {
        // create a unique key for each output
        let mut all_payees_only = vec![];
        for (_content_addr, payees) in all_data_payments.clone().into_iter() {
            let mut rng = &mut rand::thread_rng();
            let unique_key_vec: Vec<(NanoTokens, MainPubkey, [u8; 32])> = payees
                .into_iter()
                .map(|(address, amount)| {
                    (
                        amount,
                        address,
                        UniquePubkey::random_derivation_index(&mut rng),
                    )
                })
                .collect_vec();
            all_payees_only.extend(unique_key_vec.clone());
        }

        let reason_hash = reason_hash.unwrap_or_default();

        let (available_cash_notes, exclusive_access) = self.available_cash_notes()?;
        debug!("Available CashNotes: {:#?}", available_cash_notes);
        let offline_transfer = create_offline_transfer(
            available_cash_notes,
            all_payees_only,
            self.address(),
            reason_hash,
        )?;

        let mut all_transfers_per_address = BTreeMap::default();

        let mut used_cash_notes = std::collections::HashSet::new();

        for (content_addr, payees) in all_data_payments {
            for (payee, token) in payees {
                if let Some(cash_note) =
                    &offline_transfer
                        .created_cash_notes
                        .iter()
                        .find(|cash_note| {
                            cash_note.main_pubkey() == &payee
                                && !used_cash_notes.contains(&cash_note.unique_pubkey().to_bytes())
                        })
                {
                    trace!("Created transaction regarding {content_addr:?} paying {:?}(origin {token:?}) to payee {payee:?}.",
                        cash_note.value());
                    used_cash_notes.insert(cash_note.unique_pubkey().to_bytes());
                    let cash_notes_for_content: &mut Vec<PaymentDetails> =
                        all_transfers_per_address.entry(content_addr).or_default();
                    cash_notes_for_content.push((
                        cash_note.unique_pubkey(),
                        *cash_note.main_pubkey(),
                        cash_note.value()?,
                    ));
                }
            }
        }

        self.wallet
            .payment_transactions
            .extend(all_transfers_per_address);

        self.update_local_wallet(offline_transfer, exclusive_access)?;
        Ok(())
    }

    fn update_local_wallet(
        &mut self,
        transfer: OfflineTransfer,
        exclusive_access: WalletExclusiveAccess,
    ) -> Result<()> {
        // First of all, update client local state.
        let spent_unique_pubkeys: BTreeSet<_> = transfer
            .tx
            .inputs
            .iter()
            .map(|input| input.unique_pubkey())
            .collect();

        // Use retain to remove spent CashNotes in one pass, improving performance
        self.wallet
            .available_cash_notes
            .retain(|k, _| !spent_unique_pubkeys.contains(k));
        for spent in spent_unique_pubkeys {
            self.wallet.spent_cash_notes.insert(spent);
        }

        if let Some(cash_note) = transfer.change_cash_note {
            let id = cash_note.unique_pubkey();
            let value = cash_note.value()?;
            self.wallet.available_cash_notes.insert(id, value);
            self.store_cash_notes_to_disk(vec![&cash_note])?;
        }

        for cash_note in &transfer.created_cash_notes {
            self.wallet
                .cash_notes_created_for_others
                .insert(cash_note.unique_pubkey());
        }
        // Store created CashNotes in a batch, improving IO performance
        self.store_cash_notes_to_disk(transfer.created_cash_notes.iter().collect())?;

        for request in transfer.all_spend_requests {
            self.unconfirmed_spend_requests.insert(request);
        }

        // store wallet to disk
        self.store(exclusive_access)?;
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

            if self.wallet.spent_cash_notes.contains(&id) {
                debug!("skipping: cash_note is spent");
                continue;
            }

            if cash_note.derived_key(&self.key).is_err() {
                debug!("skipping: cash_note is not our key");
                continue;
            }

            let value = cash_note.value()?;
            self.wallet.available_cash_notes.insert(id, value);

            self.store_cash_notes_to_disk(vec![cash_note])?;
        }

        self.store(exclusive_access)?;

        Ok(())
    }

    pub fn unwrap_transfer(&self, transfer: &Transfer) -> Result<Vec<CashNoteRedemption>> {
        transfer
            .cashnote_redemptions(&self.key)
            .map_err(|_| Error::FailedToDecypherTransfer)
    }

    pub fn derive_key(&self, derivation_index: &DerivationIndex) -> DerivedSecretKey {
        self.key.derive_key(derivation_index)
    }

    /// Lookup previous payments and subtract them from the payments we're about to do.
    /// In essence this will make sure we don't overpay.
    pub fn adjust_payment_map(
        &self,
        payment_map: &mut BTreeMap<XorName, Vec<(MainPubkey, NanoTokens)>>,
    ) {
        // For each target address
        for (xor, payments) in payment_map.iter_mut() {
            // All payments done for an address, should be multiple nodes.
            if let Some(notes) = self.get_payment_unique_pubkeys_and_values(xor) {
                for (_note_main_key, main_pub_key, note_value) in notes {
                    // Find new payment to a node, for which we have a cashnote already
                    if let Some((_main_pub_key, payment_tokens)) = payments
                        .iter_mut()
                        .find(|(pubkey, _)| pubkey.public_key() == main_pub_key.public_key())
                    {
                        // Subtract what we already paid from what we're about to pay.
                        // `checked_sub` overflows if would be negative, so we set it to 0.
                        *payment_tokens = payment_tokens
                            .checked_sub(*note_value)
                            .unwrap_or(NanoTokens::zero());
                    }
                }
            }

            // Remove all payments that are 0.
            payments.retain(|(_mainpk, payment_tokens)| payment_tokens > &NanoTokens::zero());
        }

        // Remove entries that do not have payments associated anymore.
        payment_map.retain(|_xor, payments| !payments.is_empty());
    }
}

/// Loads a serialized wallet from a path.
fn load_from_path(
    wallet_dir: &Path,
    main_key: Option<MainSecretKey>,
) -> Result<(MainSecretKey, KeyLessWallet, BTreeSet<SignedSpend>)> {
    let key = match get_main_key(wallet_dir)? {
        Some(key) => key,
        None => {
            let key = main_key.unwrap_or(MainSecretKey::random());
            store_new_keypair(wallet_dir, &key)?;
            warn!("No main key found when loading wallet from path, generating a new one with pubkey: {:?}", key.main_pubkey());
            key
        }
    };
    let unconfirmed_spend_requests = match get_unconfirmed_spend_requests(wallet_dir)? {
        Some(unconfirmed_spend_requests) => unconfirmed_spend_requests,
        None => Default::default(),
    };
    let wallet = match get_wallet(wallet_dir)? {
        Some(wallet) => {
            debug!(
                "Loaded wallet from {:#?} with balance {:?}",
                wallet_dir,
                wallet.balance()
            );
            wallet
        }
        None => {
            let wallet = KeyLessWallet::new();
            store_wallet(wallet_dir, &wallet)?;
            wallet
        }
    };

    Ok((key, wallet, unconfirmed_spend_requests))
}

impl KeyLessWallet {
    fn new() -> Self {
        Self {
            available_cash_notes: Default::default(),
            cash_notes_created_for_others: Default::default(),
            spent_cash_notes: Default::default(),
            payment_transactions: ContentPaymentsIdMap::default(),
        }
    }

    fn balance(&self) -> NanoTokens {
        // loop through avaiable bcs and get total token count
        let mut balance = 0;
        for (_unique_pubkey, value) in self.available_cash_notes.iter() {
            balance += value.as_nano();
        }

        NanoTokens::from(balance)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{get_wallet, store_wallet, LocalWallet};
    use crate::{
        genesis::{create_first_cash_note_from_key, GENESIS_CASHNOTE_AMOUNT},
        wallet::{local_store::WALLET_DIR_NAME, KeyLessWallet},
        MainSecretKey, NanoTokens, SpendAddress,
    };
    use assert_fs::TempDir;
    use eyre::Result;
    use xor_name::XorName;

    #[tokio::test]
    async fn keyless_wallet_to_and_from_file() -> Result<()> {
        let key = MainSecretKey::random();
        let mut wallet = KeyLessWallet::new();
        let genesis = create_first_cash_note_from_key(&key).expect("Genesis creation to succeed.");

        let dir = create_temp_dir();
        let wallet_dir = dir.path().to_path_buf();

        wallet
            .available_cash_notes
            .insert(genesis.unique_pubkey(), genesis.value()?);

        store_wallet(&wallet_dir, &wallet)?;

        let deserialized = get_wallet(&wallet_dir)?.expect("There to be a wallet on disk.");

        assert_eq!(GENESIS_CASHNOTE_AMOUNT, wallet.balance().as_nano());
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deserialized.balance().as_nano());

        Ok(())
    }

    #[test]
    fn wallet_basics() -> Result<()> {
        let key = MainSecretKey::random();
        let main_pubkey = key.main_pubkey();
        let dir = create_temp_dir();

        let deposit_only = LocalWallet {
            key,
            unconfirmed_spend_requests: Default::default(),

            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        assert_eq!(main_pubkey, deposit_only.address());
        assert_eq!(NanoTokens::zero(), deposit_only.balance());

        assert!(deposit_only.wallet.available_cash_notes.is_empty());
        assert!(deposit_only.wallet.cash_notes_created_for_others.is_empty());
        assert!(deposit_only.wallet.spent_cash_notes.is_empty());

        Ok(())
    }

    /// -----------------------------------
    /// <-------> DepositWallet <--------->
    /// -----------------------------------

    #[tokio::test]
    async fn deposit_empty_list_does_nothing() -> Result<()> {
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key: MainSecretKey::random(),
            unconfirmed_spend_requests: Default::default(),

            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit_and_store_to_disk(&vec![])?;

        assert_eq!(NanoTokens::zero(), deposit_only.balance());

        assert!(deposit_only.wallet.available_cash_notes.is_empty());
        assert!(deposit_only.wallet.cash_notes_created_for_others.is_empty());
        assert!(deposit_only.wallet.spent_cash_notes.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_adds_cash_notes_that_belongs_to_the_wallet() -> Result<()> {
        let key = MainSecretKey::random();
        let genesis = create_first_cash_note_from_key(&key).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key,
            unconfirmed_spend_requests: Default::default(),

            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit_and_store_to_disk(&vec![genesis])?;

        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deposit_only.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_does_not_add_cash_notes_not_belonging_to_the_wallet() -> Result<()> {
        let genesis = create_first_cash_note_from_key(&MainSecretKey::random())
            .expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut local_wallet = LocalWallet {
            key: MainSecretKey::random(),
            unconfirmed_spend_requests: Default::default(),

            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        local_wallet.deposit_and_store_to_disk(&vec![genesis])?;

        assert_eq!(NanoTokens::zero(), local_wallet.balance());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_is_idempotent() -> Result<()> {
        let key = MainSecretKey::random();
        let genesis_0 =
            create_first_cash_note_from_key(&key).expect("Genesis creation to succeed.");
        let genesis_1 =
            create_first_cash_note_from_key(&key).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key,
            wallet: KeyLessWallet::new(),
            unconfirmed_spend_requests: Default::default(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit_and_store_to_disk(&vec![genesis_0.clone()])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deposit_only.balance().as_nano());

        deposit_only.deposit_and_store_to_disk(&vec![genesis_0])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deposit_only.balance().as_nano());

        deposit_only.deposit_and_store_to_disk(&vec![genesis_1])?;
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deposit_only.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_wallet_to_and_from_file() -> Result<()> {
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut depositor = LocalWallet::load_from(&root_dir)?;
        let genesis =
            create_first_cash_note_from_key(&depositor.key).expect("Genesis creation to succeed.");
        depositor.deposit_and_store_to_disk(&vec![genesis])?;

        let deserialized = LocalWallet::load_from(&root_dir)?;

        assert_eq!(depositor.address(), deserialized.address());
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, depositor.balance().as_nano());
        assert_eq!(GENESIS_CASHNOTE_AMOUNT, deserialized.balance().as_nano());

        assert_eq!(1, depositor.wallet.available_cash_notes.len());
        assert_eq!(0, depositor.wallet.cash_notes_created_for_others.len());
        assert_eq!(0, depositor.wallet.spent_cash_notes.len());

        assert_eq!(1, deserialized.wallet.available_cash_notes.len());
        assert_eq!(0, deserialized.wallet.cash_notes_created_for_others.len());
        assert_eq!(0, deserialized.wallet.spent_cash_notes.len());

        let a_available = depositor
            .wallet
            .available_cash_notes
            .values()
            .last()
            .expect("There to be an available CashNote.");
        let b_available = deserialized
            .wallet
            .available_cash_notes
            .values()
            .last()
            .expect("There to be an available CashNote.");
        assert_eq!(a_available, b_available);

        Ok(())
    }

    /// --------------------------------
    /// <-------> SendWallet <--------->
    /// --------------------------------

    #[tokio::test]
    async fn sending_decreases_balance() -> Result<()> {
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&root_dir)?;
        let sender_cash_note =
            create_first_cash_note_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit_and_store_to_disk(&vec![sender_cash_note])?;

        assert_eq!(GENESIS_CASHNOTE_AMOUNT, sender.balance().as_nano());

        // We send to a new address.
        let send_amount = 100;
        let recipient_key = MainSecretKey::random();
        let recipient_main_pubkey = recipient_key.main_pubkey();
        let to = vec![(NanoTokens::from(send_amount), recipient_main_pubkey)];
        let created_cash_notes = sender.local_send(to, None)?;

        assert_eq!(1, created_cash_notes.len());
        assert_eq!(
            GENESIS_CASHNOTE_AMOUNT - send_amount,
            sender.balance().as_nano()
        );

        let recipient_cash_note = &created_cash_notes[0];
        assert_eq!(NanoTokens::from(send_amount), recipient_cash_note.value()?);
        assert_eq!(&recipient_main_pubkey, recipient_cash_note.main_pubkey());

        Ok(())
    }

    #[tokio::test]
    async fn send_wallet_to_and_from_file() -> Result<()> {
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&root_dir)?;
        let sender_cash_note =
            create_first_cash_note_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit_and_store_to_disk(&vec![sender_cash_note])?;

        // We send to a new address.
        let send_amount = 100;
        let recipient_key = MainSecretKey::random();
        let recipient_main_pubkey = recipient_key.main_pubkey();
        let to = vec![(NanoTokens::from(send_amount), recipient_main_pubkey)];
        let _created_cash_notes = sender.local_send(to, None)?;

        let deserialized = LocalWallet::load_from(&root_dir)?;

        assert_eq!(sender.address(), deserialized.address());
        assert_eq!(
            GENESIS_CASHNOTE_AMOUNT - send_amount,
            sender.balance().as_nano()
        );
        assert_eq!(
            GENESIS_CASHNOTE_AMOUNT - send_amount,
            deserialized.balance().as_nano()
        );

        assert_eq!(1, sender.wallet.available_cash_notes.len());
        assert_eq!(1, sender.wallet.cash_notes_created_for_others.len());
        assert_eq!(1, sender.wallet.spent_cash_notes.len());

        assert_eq!(1, deserialized.wallet.available_cash_notes.len());
        assert_eq!(1, deserialized.wallet.cash_notes_created_for_others.len());
        assert_eq!(1, deserialized.wallet.spent_cash_notes.len());

        let a_available = sender
            .wallet
            .available_cash_notes
            .values()
            .last()
            .expect("There to be an available CashNote.");
        let b_available = deserialized
            .wallet
            .available_cash_notes
            .values()
            .last()
            .expect("There to be an available CashNote.");
        assert_eq!(a_available, b_available);

        let a_created_for_others = &sender.wallet.cash_notes_created_for_others;
        let b_created_for_others = &deserialized.wallet.cash_notes_created_for_others;
        assert_eq!(a_created_for_others, b_created_for_others);

        let a_spent = sender
            .wallet
            .spent_cash_notes
            .iter()
            .last()
            .expect("There to be a spent CashNote.");
        let b_spent = deserialized
            .wallet
            .spent_cash_notes
            .iter()
            .last()
            .expect("There to be a spent CashNote.");
        assert_eq!(a_spent, b_spent);

        Ok(())
    }

    #[tokio::test]
    async fn store_created_cash_note_gives_file_that_try_load_cash_notes_can_use() -> Result<()> {
        let sender_root_dir = create_temp_dir();
        let sender_root_dir = sender_root_dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&sender_root_dir)?;
        let sender_cash_note =
            create_first_cash_note_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit_and_store_to_disk(&vec![sender_cash_note])?;

        let send_amount = 100;

        // Send to a new address.
        let recipient_root_dir = create_temp_dir();
        let recipient_root_dir = recipient_root_dir.path().to_path_buf();
        let mut recipient = LocalWallet::load_from(&recipient_root_dir)?;
        let recipient_main_pubkey = recipient.key.main_pubkey();

        let to = vec![(NanoTokens::from(send_amount), recipient_main_pubkey)];
        let created_cash_notes = sender.local_send(to, None)?;
        let cash_note = created_cash_notes[0].clone();
        let unique_pubkey = cash_note.unique_pubkey();
        sender.store_cash_notes_to_disk(vec![&cash_note])?;

        let unique_pubkey_name = *SpendAddress::from_unique_pubkey(&unique_pubkey).xorname();
        let unique_pubkey_file_name = format!("{}.cash_note", hex::encode(unique_pubkey_name));

        let created_cash_notes_dir = sender_root_dir.join(WALLET_DIR_NAME).join("cash_notes");
        let created_cash_note_file = created_cash_notes_dir.join(&unique_pubkey_file_name);

        let received_cash_note_dir = recipient_root_dir.join(WALLET_DIR_NAME).join("cash_notes");

        std::fs::create_dir_all(&received_cash_note_dir)?;
        let received_cash_note_file = received_cash_note_dir.join(&unique_pubkey_file_name);

        // Move the created cash_note to the recipient's received_cash_notes dir.
        std::fs::rename(created_cash_note_file, received_cash_note_file)?;

        assert_eq!(0, recipient.wallet.balance().as_nano());

        recipient.try_load_cash_notes()?;

        assert_eq!(1, recipient.wallet.available_cash_notes.len());

        let available = recipient
            .wallet
            .available_cash_notes
            .keys()
            .last()
            .expect("There to be an available CashNote.");

        assert_eq!(available, &unique_pubkey);
        assert_eq!(send_amount, recipient.wallet.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn reuse_previous_cashnotes_for_payment_to_same_address() -> Result<()> {
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&root_dir)?;
        let sender_cash_note =
            create_first_cash_note_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit_and_store_to_disk(&vec![sender_cash_note])?;

        let mut rng = bls::rand::thread_rng();
        let xor1 = XorName::random(&mut rng);
        let xor2 = XorName::random(&mut rng);
        let xor3 = XorName::random(&mut rng);
        let xor4 = XorName::random(&mut rng);

        let key1a = MainSecretKey::random().main_pubkey();
        let key1b = MainSecretKey::random().main_pubkey();
        let key2a = MainSecretKey::random().main_pubkey();
        let key2b = MainSecretKey::random().main_pubkey();
        let key3a = MainSecretKey::random().main_pubkey();
        let key3b = MainSecretKey::random().main_pubkey();
        let key4a = MainSecretKey::random().main_pubkey();
        let key4b = MainSecretKey::random().main_pubkey();

        // Test 4 scenarios. 1) No change in cost. 2) Increase in cost. 3) Decrease in cost. 4) Both increase and decrease.

        let mut map = BTreeMap::from([
            (xor1, vec![(key1a, 100.into()), (key1b, 101.into())]),
            (xor2, vec![(key2a, 200.into()), (key2b, 201.into())]),
            (xor3, vec![(key3a, 300.into()), (key3b, 301.into())]),
            (xor4, vec![(key4a, 400.into()), (key4b, 401.into())]),
        ]);

        sender.local_send_storage_payment(map.clone(), None)?;

        map.get_mut(&xor2).expect("to have value")[0].1 = 210.into(); // increase: 200 -> 210
        map.get_mut(&xor3).expect("to have value")[0].1 = 290.into(); // decrease: 300 -> 290
        map.get_mut(&xor4).expect("to have value")[0].1 = 390.into(); // decrease: 400 -> 390
        map.get_mut(&xor4).expect("to have value")[1].1 = 410.into(); // increase: 401 -> 410

        sender.adjust_payment_map(&mut map);

        // The map should now only have the entries where store costs increased:
        assert_eq!(
            map,
            BTreeMap::from([
                (xor2, vec![(key2a, 10.into())]),
                (xor4, vec![(key4b, 9.into())]),
            ]),
            "payment map should only have the adjusted prices"
        );

        Ok(())
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Should be able to create a temp dir.")
    }
}
