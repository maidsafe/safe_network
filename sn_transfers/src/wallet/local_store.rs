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
        create_received_dbcs_dir, get_unconfirmed_txs, get_wallet, load_dbc, load_received_dbcs,
        store_created_dbcs, store_unconfirmed_txs, store_wallet,
    },
    KeyLessWallet, Result,
};
use crate::client_transfers::{
    create_transfer, ContentPaymentsIdMap, SpendRequest, TransferOutputs,
};
use itertools::Itertools;
use sn_dbc::{
    random_derivation_index, Dbc, DbcId, DerivedKey, Hash, MainKey, PublicAddress, Token,
};
use sn_protocol::NetworkAddress;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const WALLET_DIR_NAME: &str = "wallet";

/// A wallet that can only receive tokens.
pub struct LocalWallet {
    /// The secret key with which we can access
    /// all the tokens in the available_dbcs.
    key: MainKey,
    /// The wallet containing all data.
    wallet: KeyLessWallet,
    /// The dir of the wallet file, main key, public address, and new dbcs.
    wallet_dir: PathBuf,
    /// These have not yet been successfully confirmed in
    /// the network and need to be republished, to reach network validity.
    /// We maintain the order they were added in, as to republish
    /// them in the correct order, in case any later spend was
    /// dependent on an earlier spend.
    unconfirmed_txs: BTreeSet<SpendRequest>,
}

impl LocalWallet {
    /// Stores the wallet to disk.
    pub fn store(&self) -> Result<()> {
        store_wallet(&self.wallet_dir, &self.wallet)
    }

    /// Stores the given dbc to the `created dbcs dir` in the wallet dir.
    /// These can then be sent to the recipients out of band, over any channel preferred.
    pub fn store_dbc(&mut self, dbc: Dbc) -> Result<()> {
        store_created_dbcs(vec![dbc], &self.wallet_dir)
    }
    /// Stores the given dbcs to the `created dbcs dir` in the wallet dir.
    /// These can then be sent to the recipients out of band, over any channel preferred.
    pub fn store_dbcs(&mut self, dbc: Vec<Dbc>) -> Result<()> {
        store_created_dbcs(dbc, &self.wallet_dir)
    }

    pub fn get_dbc(&mut self, dbc_id: &DbcId) -> Option<Dbc> {
        load_dbc(dbc_id, &self.wallet_dir)
    }

    /// Store unconfirmed_txs to disk.
    pub fn store_unconfirmed_txs(&mut self) -> Result<()> {
        store_unconfirmed_txs(&self.wallet_dir, self.unconfirmed_txs())
    }

    /// Unconfirmed txs exist
    pub fn unconfirmed_txs_exist(&self) -> bool {
        !self.unconfirmed_txs.is_empty()
    }

    /// Try to load any new dbcs from the `received dbcs dir` in the wallet dir.
    pub fn try_load_deposits(&mut self) -> Result<()> {
        let deposited = load_received_dbcs(&self.wallet_dir)?;
        self.deposit(deposited)?;
        Ok(())
    }

    /// Loads a serialized wallet from a path and given main key.
    pub fn load_from_main_key(root_dir: &Path, main_key: MainKey) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        // This creates the received_dbcs dir if it doesn't exist.
        std::fs::create_dir_all(&wallet_dir)?;
        // This creates the main_key file if it doesn't exist.
        let (key, wallet, unconfirmed_txs) = load_from_path(&wallet_dir, Some(main_key))?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
            unconfirmed_txs,
        })
    }

    /// Loads a serialized wallet from a path.
    pub fn load_from(root_dir: &Path) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        // This creates the received_dbcs dir if it doesn't exist.
        std::fs::create_dir_all(&wallet_dir)?;
        let (key, wallet, unconfirmed_txs) = load_from_path(&wallet_dir, None)?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
            unconfirmed_txs,
        })
    }

    /// Tries to loads a serialized wallet from a path, bailing out if it doesn't exist.
    pub fn try_load_from(root_dir: &Path) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        let (key, wallet, unconfirmed_txs) = load_from_path(&wallet_dir, None)?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
            unconfirmed_txs,
        })
    }

    pub fn address(&self) -> PublicAddress {
        self.key.public_address()
    }

    pub fn unconfirmed_txs(&self) -> &BTreeSet<SpendRequest> {
        &self.unconfirmed_txs
    }

    pub fn clear_unconfirmed_txs(&mut self) {
        self.unconfirmed_txs = Default::default();
    }

    pub fn balance(&self) -> Token {
        self.wallet.balance()
    }

    pub fn sign(&self, msg: &[u8]) -> bls::Signature {
        self.key.sign(msg)
    }

    pub fn available_dbcs(&self) -> Vec<(Dbc, DerivedKey)> {
        let mut available_dbcs = vec![];

        for (id, _token) in self.wallet.available_dbcs.iter() {
            let held_dbc = load_dbc(id, &self.wallet_dir);
            if let Some(dbc) = held_dbc {
                if let Ok(derived_key) = dbc.derived_key(&self.key) {
                    available_dbcs.push((dbc.clone(), derived_key));
                } else {
                    warn!(
                        "Skipping DBC {:?} because we don't have the key to spend it",
                        dbc.id()
                    );
                }
            } else {
                warn!("Skipping DBC {:?} because we don't have it", id);
            }
        }
        available_dbcs
    }

    /// Add given storage payment proofs to the wallet's cache,
    /// so they can be used when uploading the paid content.
    pub fn add_content_payments_map(&mut self, proofs: ContentPaymentsIdMap) {
        self.wallet.payment_transactions.extend(proofs);
    }

    /// Return the payment dbc ids for the given content address name if cached.
    pub fn get_payment_dbc_ids(&self, name: &NetworkAddress) -> Option<&Vec<DbcId>> {
        self.wallet.payment_transactions.get(name)
    }

    /// Return the payment dbc ids for the given content address name if cached.
    pub fn get_payment_dbcs(&self, name: &NetworkAddress) -> Vec<Dbc> {
        let ids = self.get_payment_dbc_ids(name);
        // now grab all those dbcs
        let mut dbcs = vec![];

        if let Some(ids) = ids {
            for id in ids {
                if let Some(dbc) = load_dbc(id, &self.wallet_dir) {
                    dbcs.push(dbc);
                }
            }
        }

        dbcs
    }

    /// Make a transfer and return all created dbcs
    pub fn local_send(
        &mut self,
        to: Vec<(Token, PublicAddress)>,
        reason_hash: Option<Hash>,
    ) -> Result<Vec<Dbc>> {
        let mut rng = &mut rand::rngs::OsRng;
        // create a unique key for each output
        let to_unique_keys: Vec<_> = to
            .into_iter()
            .map(|(amount, address)| (amount, address, random_derivation_index(&mut rng)))
            .collect();

        let available_dbcs = self.available_dbcs();
        trace!("Available DBCs for local send: {:#?}", available_dbcs);

        let reason_hash = reason_hash.unwrap_or_default();

        let transfer =
            create_transfer(available_dbcs, to_unique_keys, self.address(), reason_hash)?;

        let created_dbcs = transfer.created_dbcs.clone();

        self.update_local_wallet(transfer)?;

        Ok(created_dbcs)
    }

    /// Performs a DBC payment for each content address, returning outputs for each.
    pub fn local_send_storage_payment(
        &mut self,
        all_data_payments: BTreeMap<NetworkAddress, Vec<(PublicAddress, Token)>>,
        reason_hash: Option<Hash>,
    ) -> Result<()> {
        // create a unique key for each output
        let mut to_unique_keys = BTreeMap::default();
        let mut all_payees_only = vec![];
        for (content_addr, payees) in all_data_payments.clone().into_iter() {
            let mut rng = &mut rand::thread_rng();
            let unique_key_vec: Vec<(Token, PublicAddress, [u8; 32])> = payees
                .into_iter()
                .map(|(address, amount)| (amount, address, random_derivation_index(&mut rng)))
                .collect_vec();
            all_payees_only.extend(unique_key_vec.clone());
            to_unique_keys.insert(content_addr.clone(), unique_key_vec);
        }

        let reason_hash = reason_hash.unwrap_or_default();

        let available_dbcs = self.available_dbcs();
        trace!("Available DBCs: {:#?}", available_dbcs);
        let transfer_outputs =
            create_transfer(available_dbcs, all_payees_only, self.address(), reason_hash)?;

        let mut all_transfers_per_address = BTreeMap::default();

        let mut used_dbcs = std::collections::HashSet::new();

        for (content_addr, payees) in all_data_payments {
            for (payee, _token) in payees {
                if let Some(dbc) = &transfer_outputs.created_dbcs.iter().find(|dbc| {
                    dbc.public_address() == &payee && !used_dbcs.contains(&dbc.id().to_bytes())
                }) {
                    used_dbcs.insert(dbc.id().to_bytes());
                    let dbcs_for_content: &mut Vec<DbcId> = all_transfers_per_address
                        .entry(content_addr.clone())
                        .or_default();
                    dbcs_for_content.push(dbc.id());
                }
            }
        }

        self.update_local_wallet(transfer_outputs)?;
        println!("Transfers applied locally");

        self.wallet
            .payment_transactions
            .extend(all_transfers_per_address);

        Ok(())
    }

    fn update_local_wallet(&mut self, transfer: TransferOutputs) -> Result<()> {
        let TransferOutputs {
            change_dbc,
            created_dbcs,
            tx,
            all_spend_requests,
        } = transfer;

        // First of all, update client local state.
        let spent_dbc_ids: BTreeSet<_> = tx.inputs.iter().map(|input| input.dbc_id()).collect();

        // Use retain to remove spent DBCs in one pass, improving performance
        self.wallet
            .available_dbcs
            .retain(|k, _| !spent_dbc_ids.contains(k));
        for spent in spent_dbc_ids {
            self.wallet.spent_dbcs.insert(spent);
        }

        self.deposit(change_dbc.into_iter().collect())?;

        // Store created DBCs in a batch, improving IO performance
        let mut created_dbcs_batch = Vec::new();
        for dbc in created_dbcs {
            self.wallet.dbcs_created_for_others.insert(dbc.id());
            created_dbcs_batch.push(dbc);
        }
        self.store_dbcs(created_dbcs_batch)?;

        for request in all_spend_requests {
            self.unconfirmed_txs.insert(request);
        }
        Ok(())
    }

    pub fn deposit(&mut self, dbcs: Vec<Dbc>) -> Result<()> {
        if dbcs.is_empty() {
            return Ok(());
        }

        for dbc in dbcs {
            let id = dbc.id();

            if let Some(_dbc) = load_dbc(&id, &self.wallet_dir) {
                println!("dbc exists");
                return Ok(());
            }

            if self.wallet.spent_dbcs.contains(&id) {
                println!("dbc is spent");
                return Ok(());
            }

            if dbc.derived_key(&self.key).is_err() {
                continue;
            }

            let token = dbc.token()?;
            self.store_dbc(dbc)?;
            self.wallet.available_dbcs.insert(id, token);
        }

        Ok(())
    }
}

/// Loads a serialized wallet from a path.
fn load_from_path(
    wallet_dir: &Path,
    main_key: Option<MainKey>,
) -> Result<(MainKey, KeyLessWallet, BTreeSet<SpendRequest>)> {
    let key = match get_main_key(wallet_dir)? {
        Some(key) => key,
        None => {
            let key = main_key.unwrap_or(MainKey::random());
            store_new_keypair(wallet_dir, &key)?;
            key
        }
    };
    let unconfirmed_txs = match get_unconfirmed_txs(wallet_dir)? {
        Some(unconfirmed_txs) => unconfirmed_txs,
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
            create_received_dbcs_dir(wallet_dir)?;
            wallet
        }
    };

    Ok((key, wallet, unconfirmed_txs))
}

impl KeyLessWallet {
    fn new() -> Self {
        Self {
            available_dbcs: Default::default(),
            dbcs_created_for_others: Default::default(),
            spent_dbcs: Default::default(),
            payment_transactions: ContentPaymentsIdMap::default(),
        }
    }

    fn balance(&self) -> Token {
        // loop through avaiable bcs and get total token count
        let mut balance = 0;
        for (_dbc_id, token) in self.available_dbcs.iter() {
            balance += token.as_nano();
        }

        Token::from_nano(balance)
    }
}

#[cfg(test)]
mod tests {
    use super::{get_wallet, store_wallet, LocalWallet};
    use crate::{
        dbc_genesis::{create_first_dbc_from_key, GENESIS_DBC_AMOUNT},
        wallet::{local_store::WALLET_DIR_NAME, KeyLessWallet},
    };
    use assert_fs::TempDir;
    use eyre::Result;
    use sn_dbc::{MainKey, Token};
    use sn_protocol::storage::DbcAddress;

    #[tokio::test]
    async fn keyless_wallet_to_and_from_file() -> Result<()> {
        let key = MainKey::random();
        let mut wallet = KeyLessWallet::new();
        let genesis = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");

        let dir = create_temp_dir();
        let wallet_dir = dir.path().to_path_buf();

        wallet.available_dbcs.insert(genesis.id(), genesis.token()?);

        store_wallet(&wallet_dir, &wallet)?;

        let deserialized = get_wallet(&wallet_dir)?.expect("There to be a wallet on disk.");

        assert_eq!(GENESIS_DBC_AMOUNT, wallet.balance().as_nano());
        assert_eq!(GENESIS_DBC_AMOUNT, deserialized.balance().as_nano());

        Ok(())
    }

    #[test]
    fn wallet_basics() -> Result<()> {
        let key = MainKey::random();
        let public_address = key.public_address();
        let dir = create_temp_dir();

        let deposit_only = LocalWallet {
            key,
            unconfirmed_txs: vec![],

            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        assert_eq!(public_address, deposit_only.address());
        assert_eq!(Token::zero(), deposit_only.balance());

        assert!(deposit_only.wallet.available_dbcs.is_empty());
        assert!(deposit_only.wallet.dbcs_created_for_others.is_empty());
        assert!(deposit_only.wallet.spent_dbcs.is_empty());

        Ok(())
    }

    /// -----------------------------------
    /// <-------> DepositWallet <--------->
    /// -----------------------------------

    #[tokio::test]
    async fn deposit_empty_list_does_nothing() -> Result<()> {
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key: MainKey::random(),
            unconfirmed_txs: vec![],

            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit(vec![])?;

        assert_eq!(Token::zero(), deposit_only.balance());

        assert!(deposit_only.wallet.available_dbcs.is_empty());
        assert!(deposit_only.wallet.dbcs_created_for_others.is_empty());
        assert!(deposit_only.wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_adds_dbcs_that_belongs_to_the_wallet() -> Result<()> {
        let key = MainKey::random();
        let genesis = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key,
            unconfirmed_txs: vec![],
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit(vec![genesis])?;

        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_does_not_add_dbcs_not_belonging_to_the_wallet() -> Result<()> {
        let genesis =
            create_first_dbc_from_key(&MainKey::random()).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut local_wallet = LocalWallet {
            key: MainKey::random(),
            unconfirmed_txs: vec![],
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        local_wallet.deposit(vec![genesis])?;

        assert_eq!(Token::zero(), local_wallet.balance());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_is_idempotent() -> Result<()> {
        let key = MainKey::random();
        let genesis_0 = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");
        let genesis_1 = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key,
            wallet: KeyLessWallet::new(),
            unconfirmed_txs: vec![],
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit(vec![genesis_0.clone()])?;
        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        deposit_only.deposit(vec![genesis_0])?;
        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        deposit_only.deposit(vec![genesis_1])?;
        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_wallet_to_and_from_file() -> Result<()> {
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut depositor = LocalWallet::load_from(&root_dir)?;
        let genesis =
            create_first_dbc_from_key(&depositor.key).expect("Genesis creation to succeed.");
        depositor.deposit(vec![genesis])?;
        depositor.store()?;

        let deserialized = LocalWallet::load_from(&root_dir)?;

        assert_eq!(depositor.address(), deserialized.address());
        assert_eq!(GENESIS_DBC_AMOUNT, depositor.balance().as_nano());
        assert_eq!(GENESIS_DBC_AMOUNT, deserialized.balance().as_nano());

        assert_eq!(1, depositor.wallet.available_dbcs.len());
        assert_eq!(0, depositor.wallet.dbcs_created_for_others.len());
        assert_eq!(0, depositor.wallet.spent_dbcs.len());

        assert_eq!(1, deserialized.wallet.available_dbcs.len());
        assert_eq!(0, deserialized.wallet.dbcs_created_for_others.len());
        assert_eq!(0, deserialized.wallet.spent_dbcs.len());

        let a_available = depositor
            .wallet
            .available_dbcs
            .values()
            .last()
            .expect("There to be an available DBC.");
        let b_available = deserialized
            .wallet
            .available_dbcs
            .values()
            .last()
            .expect("There to be an available DBC.");
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
        let sender_dbc =
            create_first_dbc_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit(vec![sender_dbc])?;

        assert_eq!(GENESIS_DBC_AMOUNT, sender.balance().as_nano());

        // We send to a new address.
        let send_amount = 100;
        let recipient_key = MainKey::random();
        let recipient_public_address = recipient_key.public_address();
        let to = vec![(Token::from_nano(send_amount), recipient_public_address)];
        let transfer = sender.local_send(to, None)?;
        let created_dbcs = transfer.created_dbcs;

        assert_eq!(1, created_dbcs.len());
        assert_eq!(GENESIS_DBC_AMOUNT - send_amount, sender.balance().as_nano());

        let recipient_dbc = &created_dbcs[0];
        assert_eq!(Token::from_nano(send_amount), recipient_dbc.token()?);
        assert_eq!(&recipient_public_address, recipient_dbc.public_address());

        Ok(())
    }

    #[tokio::test]
    async fn send_wallet_to_and_from_file() -> Result<()> {
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&root_dir)?;
        let sender_dbc =
            create_first_dbc_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit(vec![sender_dbc])?;

        // We send to a new address.
        let send_amount = 100;
        let recipient_key = MainKey::random();
        let recipient_public_address = recipient_key.public_address();
        let to = vec![(Token::from_nano(send_amount), recipient_public_address)];
        let _created_dbcs = sender.local_send(to, None)?;

        sender.store()?;

        let deserialized = LocalWallet::load_from(&root_dir)?;

        assert_eq!(sender.address(), deserialized.address());
        assert_eq!(GENESIS_DBC_AMOUNT - send_amount, sender.balance().as_nano());
        assert_eq!(
            GENESIS_DBC_AMOUNT - send_amount,
            deserialized.balance().as_nano()
        );

        assert_eq!(1, sender.wallet.available_dbcs.len());
        assert_eq!(1, sender.wallet.dbcs_created_for_others.len());
        assert_eq!(1, sender.wallet.spent_dbcs.len());

        assert_eq!(1, deserialized.wallet.available_dbcs.len());
        assert_eq!(1, deserialized.wallet.dbcs_created_for_others.len());
        assert_eq!(1, deserialized.wallet.spent_dbcs.len());

        let a_available = sender
            .wallet
            .available_dbcs
            .values()
            .last()
            .expect("There to be an available DBC.");
        let b_available = deserialized
            .wallet
            .available_dbcs
            .values()
            .last()
            .expect("There to be an available DBC.");
        assert_eq!(a_available, b_available);

        let a_created_for_others = &sender.wallet.dbcs_created_for_others;
        let b_created_for_others = &deserialized.wallet.dbcs_created_for_others;
        assert_eq!(a_created_for_others, b_created_for_others);

        let a_spent = sender
            .wallet
            .spent_dbcs
            .iter()
            .last()
            .expect("There to be a spent DBC.");
        let b_spent = deserialized
            .wallet
            .spent_dbcs
            .iter()
            .last()
            .expect("There to be a spent DBC.");
        assert_eq!(a_spent, b_spent);

        Ok(())
    }

    #[tokio::test]
    async fn store_created_dbc_gives_file_that_try_load_deposits_can_use() -> Result<()> {
        let sender_root_dir = create_temp_dir();
        let sender_root_dir = sender_root_dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&sender_root_dir)?;
        let sender_dbc =
            create_first_dbc_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit(vec![sender_dbc])?;

        let send_amount = 100;

        // Send to a new address.
        let recipient_root_dir = create_temp_dir();
        let recipient_root_dir = recipient_root_dir.path().to_path_buf();
        let mut recipient = LocalWallet::load_from(&recipient_root_dir)?;
        let recipient_public_address = recipient.key.public_address();

        let to = vec![(Token::from_nano(send_amount), recipient_public_address)];
        let transfer = sender.local_send(to, None)?;
        let created_dbcs = transfer.created_dbcs;
        let dbc = created_dbcs[0].clone();
        let dbc_id = dbc.id();
        sender.store_dbc(dbc)?;

        let dbc_id_name = *DbcAddress::from_dbc_id(&dbc_id).xorname();
        let dbc_id_file_name = format!("{}.dbc", hex::encode(dbc_id_name));

        let created_dbcs_dir = sender_root_dir.join(WALLET_DIR_NAME).join("created_dbcs");
        let created_dbc_file = created_dbcs_dir.join(&dbc_id_file_name);

        let received_dbc_dir = recipient_root_dir
            .join(WALLET_DIR_NAME)
            .join("received_dbcs");

        std::fs::create_dir_all(&received_dbc_dir)?;
        let received_dbc_file = received_dbc_dir.join(&dbc_id_file_name);

        // Move the created dbc to the recipient's received_dbcs dir.
        std::fs::rename(created_dbc_file, received_dbc_file)?;

        assert_eq!(0, recipient.wallet.balance().as_nano());

        recipient.try_load_deposits()?;

        assert_eq!(1, recipient.wallet.available_dbcs.len());

        let available = recipient
            .wallet
            .available_dbcs
            .keys()
            .last()
            .expect("There to be an available DBC.");

        assert_eq!(available, &dbc_id);
        assert_eq!(send_amount, recipient.wallet.balance().as_nano());

        Ok(())
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Should be able to create a temp dir.")
    }
}
