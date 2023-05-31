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
        create_received_dbcs_dir, get_wallet, load_received_dbcs, store_created_dbcs, store_wallet,
    },
    DepositWallet, KeyLessWallet, Result, SendClient, SendWallet, SigningWallet, Wallet,
};

use crate::client_transfers::{create_transfer, CreatedDbc, Outputs as TransferDetails};

use sn_dbc::{Dbc, DbcIdSource, MainKey, PublicAddress, Token};

use async_trait::async_trait;
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
}

impl LocalWallet {
    /// Stores the wallet to disk.
    pub async fn store(&self) -> Result<()> {
        store_wallet(&self.wallet_dir, &self.wallet).await
    }

    /// Stores the given dbc to the `created dbcs dir` in the wallet dir.
    /// Each recipient has their own dir, containing all dbcs for them.
    /// These can then be sent to the recipients out of band, over any channel preferred.
    pub async fn store_created_dbc(&mut self, dbc: Dbc) -> Result<()> {
        store_created_dbcs(vec![dbc], &self.wallet_dir).await
    }

    /// Try to load any new dbcs from the `received dbcs dir` in the wallet dir.
    pub async fn try_load_deposits(&mut self) -> Result<()> {
        let deposited = load_received_dbcs(&self.wallet_dir).await?;
        self.wallet.deposit(deposited, &self.key);
        Ok(())
    }

    /// Loads a serialized wallet from a path.
    pub async fn load_from(root_dir: &Path) -> Result<Self> {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        // This creates the received_dbcs dir if it doesn't exist.
        tokio::fs::create_dir_all(&wallet_dir).await?;
        let (key, wallet) = load_from_path(&wallet_dir).await?;
        Ok(Self {
            key,
            wallet,
            wallet_dir: wallet_dir.to_path_buf(),
        })
    }
}

/// Loads a serialized wallet from a path.
async fn load_from_path(wallet_dir: &Path) -> Result<(MainKey, KeyLessWallet)> {
    let key = match get_main_key(wallet_dir).await? {
        Some(key) => key,
        None => {
            let key = MainKey::random();
            store_new_keypair(wallet_dir, &key).await?;
            key
        }
    };
    let wallet = match get_wallet(wallet_dir).await? {
        Some(wallet) => wallet,
        None => {
            let wallet = KeyLessWallet::new();
            store_wallet(wallet_dir, &wallet).await?;
            create_received_dbcs_dir(wallet_dir).await?;
            wallet
        }
    };

    Ok((key, wallet))
}

impl KeyLessWallet {
    fn new() -> Self {
        Self {
            balance: Token::zero(),
            spent_dbcs: BTreeMap::new(),
            available_dbcs: BTreeMap::new(),
            dbcs_created_for_others: vec![],
            unconfirmed_txs: vec![],
        }
    }

    fn balance(&self) -> Token {
        self.balance
    }

    fn deposit(&mut self, dbcs: Vec<Dbc>, key: &MainKey) {
        if dbcs.is_empty() {
            return;
        }

        let mut received_dbcs = dbcs
            .into_iter()
            .filter_map(|dbc| {
                let id = dbc.id();
                (!self.spent_dbcs.contains_key(&id)).then_some((id, dbc))
            })
            .filter_map(|(id, dbc)| dbc.derived_key(key).is_ok().then_some((id, dbc)))
            .collect();

        self.available_dbcs.append(&mut received_dbcs);

        let new_balance = self
            .available_dbcs
            .iter()
            .flat_map(|(_, dbc)| dbc.derived_key(key).map(|derived_key| (dbc, derived_key)))
            .flat_map(|(dbc, derived_key)| dbc.revealed_input(&derived_key))
            .fold(0, |total, amount| total + amount.revealed_amount().value());

        self.balance = Token::from_nano(new_balance);
    }
}

impl Wallet for LocalWallet {
    fn address(&self) -> PublicAddress {
        self.key.public_address()
    }

    fn balance(&self) -> Token {
        self.wallet.balance()
    }
}

impl SigningWallet for LocalWallet {
    fn sign(&self, msg: &[u8]) -> bls::Signature {
        self.key.sign(msg)
    }
}

impl DepositWallet for LocalWallet {
    fn new_dbc_address(&self) -> DbcIdSource {
        self.key.random_dbc_id_src(&mut rand::thread_rng())
    }

    fn deposit(&mut self, dbcs: Vec<Dbc>) {
        self.wallet.deposit(dbcs, &self.key);
    }
}

#[async_trait]
impl SendWallet for LocalWallet {
    async fn send<C: SendClient>(
        &mut self,
        to: Vec<(Token, PublicAddress)>,
        client: &C,
    ) -> Result<Vec<CreatedDbc>> {
        // First resend any pending txs. This is not guaranteed to succeed.
        // If the spend was invalid to start with then it will always fail here.
        // It can disrupt the use of the wallet, if we got change from that invalid
        // tx, that we try to spend later. So either we need to make sure that the
        // failing transfers are not failing due to being invalid, or we need a way
        // to check that later and clear those out from the pending txs list.
        // Since they could become "invalid" while being in the pending txs list
        // (the DBCs they spend, became spent in other txs in the meanwhile), the latter
        // solution of being able to check and clean out the list later seems to be necessary.
        resend_pending_txs(self, client).await;

        // do not make a pointless send to ourselves

        let to: Vec<_> = to
            .into_iter()
            .filter_map(|(amount, address)| {
                let dbc_id_src = address.random_dbc_id_src(&mut rand::thread_rng());
                (address != self.address()).then_some((amount, dbc_id_src))
            })
            .collect();
        if to.is_empty() {
            return Ok(vec![]);
        }

        let mut available_dbcs = vec![];
        for dbc in self.wallet.available_dbcs.values() {
            if let Ok(derived_key) = dbc.derived_key(&self.key) {
                available_dbcs.push((dbc.clone(), derived_key));
            } else {
                println!(
                    "Skipping DBC {:?} because we don't have the key to spend it",
                    dbc.id()
                );
            }
        }

        let transfer = create_transfer(available_dbcs, to, self.address())?;

        let TransferDetails {
            change_dbc,
            created_dbcs,
            ..
        } = transfer.clone();

        // First of all, update client local state.
        let spent_dbc_ids: BTreeSet<_> = created_dbcs
            .iter()
            .flat_map(|created| &created.dbc.signed_spends)
            .map(|spend| spend.dbc_id())
            .collect();

        let mut spent_dbcs = spent_dbc_ids
            .into_iter()
            .filter_map(|id| self.wallet.available_dbcs.remove(id).map(|dbc| (*id, dbc)))
            .collect();

        self.deposit(change_dbc.into_iter().collect());
        self.wallet.spent_dbcs.append(&mut spent_dbcs);
        self.wallet
            .dbcs_created_for_others
            .extend(created_dbcs.clone());

        // Last of all, register the spend in the network.
        println!("Sending transfer to the network: {transfer:#?}");
        if let Err(error) = client.send(transfer.clone()).await {
            println!("The transfer was not successfully registered in the network: {error:?}. It will be retried later.");
            let _ = self.wallet.unconfirmed_txs.push(transfer);
        }

        Ok(created_dbcs)
    }
}

async fn resend_pending_txs<C: SendClient>(local: &mut LocalWallet, client: &C) {
    for (index, transfer) in local.wallet.unconfirmed_txs.clone().into_iter().enumerate() {
        println!("Trying to republish pending tx: {:?}..", transfer.tx_hash);
        if client.send(transfer.clone()).await.is_ok() {
            println!("Tx {:?} was successfully republished!", transfer.tx_hash);
            let _ = local.wallet.unconfirmed_txs.remove(index);
            // We might want to be _really_ sure and do the below
            // as well, but it's not necessary.
            // use crate::domain::wallet::VerifyingClient;
            // client.verify(tx_hash).await.ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{get_wallet, store_wallet, LocalWallet};

    use crate::{
        client_transfers::Outputs as TransferDetails,
        dbc_genesis::{create_first_dbc_from_key, GENESIS_DBC_AMOUNT},
        wallet::{local_store::WALLET_DIR_NAME, public_address_name, KeyLessWallet, SendClient},
    };

    use sn_dbc::{MainKey, Token};
    use sn_protocol::storage::DbcAddress;

    use assert_fs::TempDir;
    use eyre::Result;

    #[tokio::test]
    async fn keyless_wallet_to_and_from_file() -> Result<()> {
        let key = MainKey::random();
        let mut wallet = KeyLessWallet::new();
        let genesis = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");

        let dir = create_temp_dir();
        let wallet_dir = dir.path().to_path_buf();

        wallet.deposit(vec![genesis], &key);

        store_wallet(&wallet_dir, &wallet).await?;

        let deserialized = get_wallet(&wallet_dir)
            .await?
            .expect("There to be a wallet on disk.");

        assert_eq!(GENESIS_DBC_AMOUNT, wallet.balance().as_nano());
        assert_eq!(GENESIS_DBC_AMOUNT, deserialized.balance().as_nano());

        Ok(())
    }

    #[test]
    fn wallet_basics() -> Result<()> {
        // Bring in the necessary traits.
        use super::{DepositWallet, Wallet};

        let key = MainKey::random();
        let public_address = key.public_address();
        let dir = create_temp_dir();

        let deposit_only = LocalWallet {
            key,
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        assert_eq!(public_address, deposit_only.address());
        assert_eq!(
            public_address,
            deposit_only.new_dbc_address().public_address
        );
        assert_eq!(Token::zero(), deposit_only.balance());

        assert!(deposit_only.wallet.available_dbcs.is_empty());
        assert!(deposit_only.wallet.dbcs_created_for_others.is_empty());
        assert!(deposit_only.wallet.spent_dbcs.is_empty());

        Ok(())
    }

    /// -----------------------------------
    /// <-------> DepositWallet <--------->
    /// -----------------------------------

    #[test]
    fn deposit_empty_list_does_nothing() -> Result<()> {
        // Bring in the necessary trait.
        use super::{DepositWallet, Wallet};

        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key: MainKey::random(),
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit(vec![]);

        assert_eq!(Token::zero(), deposit_only.balance());

        assert!(deposit_only.wallet.available_dbcs.is_empty());
        assert!(deposit_only.wallet.dbcs_created_for_others.is_empty());
        assert!(deposit_only.wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[test]
    fn deposit_adds_dbcs_that_belongs_to_the_wallet() -> Result<()> {
        // Bring in the necessary trait.
        use super::{DepositWallet, Wallet};

        let key = MainKey::random();
        let genesis = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key,
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit(vec![genesis]);

        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        Ok(())
    }

    #[test]
    fn deposit_does_not_add_dbcs_not_belonging_to_the_wallet() -> Result<()> {
        // Bring in the necessary traits.
        use super::{DepositWallet, Wallet};

        let genesis =
            create_first_dbc_from_key(&MainKey::random()).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut local_wallet = LocalWallet {
            key: MainKey::random(),
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        local_wallet.deposit(vec![genesis]);

        assert_eq!(Token::zero(), local_wallet.balance());

        Ok(())
    }

    #[test]
    fn deposit_is_idempotent() -> Result<()> {
        // Bring in the necessary trait.
        use super::{DepositWallet, Wallet};

        let key = MainKey::random();
        let genesis_0 = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");
        let genesis_1 = create_first_dbc_from_key(&key).expect("Genesis creation to succeed.");
        let dir = create_temp_dir();

        let mut deposit_only = LocalWallet {
            key,
            wallet: KeyLessWallet::new(),
            wallet_dir: dir.path().to_path_buf(),
        };

        deposit_only.deposit(vec![genesis_0.clone()]);
        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        deposit_only.deposit(vec![genesis_0]);
        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        deposit_only.deposit(vec![genesis_1]);
        assert_eq!(GENESIS_DBC_AMOUNT, deposit_only.balance().as_nano());

        Ok(())
    }

    #[tokio::test]
    async fn deposit_wallet_to_and_from_file() -> Result<()> {
        // Bring in the necessary traits.
        use super::{DepositWallet, Wallet};

        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut depositor = LocalWallet::load_from(&root_dir).await?;
        let genesis =
            create_first_dbc_from_key(&depositor.key).expect("Genesis creation to succeed.");
        depositor.deposit(vec![genesis]);
        depositor.store().await?;

        let deserialized = LocalWallet::load_from(&root_dir).await?;

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
        // Bring in the necessary traits.
        use super::{DepositWallet, SendWallet, Wallet};

        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&root_dir).await?;
        let sender_dbc =
            create_first_dbc_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit(vec![sender_dbc]);

        assert_eq!(GENESIS_DBC_AMOUNT, sender.balance().as_nano());

        // We send to a new address.
        let send_amount = 100;
        let recipient_key = MainKey::random();
        let recipient_public_address = recipient_key.public_address();
        let to = vec![(Token::from_nano(send_amount), recipient_public_address)];
        let created_dbcs = sender.send(to, &MockSendClient).await?;

        assert_eq!(1, created_dbcs.len());
        assert_eq!(GENESIS_DBC_AMOUNT - send_amount, sender.balance().as_nano());

        let recipient_dbc = &created_dbcs[0];
        assert_eq!(send_amount, recipient_dbc.amount.value());
        assert_eq!(
            &recipient_public_address,
            recipient_dbc.dbc.public_address()
        );

        Ok(())
    }

    #[tokio::test]
    async fn send_wallet_to_and_from_file() -> Result<()> {
        // Bring in the necessary traits.
        use super::{DepositWallet, SendWallet, Wallet};

        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&root_dir).await?;
        let sender_dbc =
            create_first_dbc_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit(vec![sender_dbc]);

        // We send to a new address.
        let send_amount = 100;
        let recipient_key = MainKey::random();
        let recipient_public_address = recipient_key.public_address();
        let to = vec![(Token::from_nano(send_amount), recipient_public_address)];
        let _created_dbcs = sender.send(to, &MockSendClient).await?;

        sender.store().await?;

        let deserialized = LocalWallet::load_from(&root_dir).await?;

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

        let a_created_for_others = &sender.wallet.dbcs_created_for_others[0];
        let b_created_for_others = &deserialized.wallet.dbcs_created_for_others[0];
        assert_eq!(a_created_for_others.dbc, b_created_for_others.dbc);
        assert_eq!(
            a_created_for_others.amount.value,
            b_created_for_others.amount.value
        );
        assert_eq!(
            a_created_for_others.amount.blinding_factor,
            b_created_for_others.amount.blinding_factor
        );

        let a_spent = sender
            .wallet
            .spent_dbcs
            .values()
            .last()
            .expect("There to be a spent DBC.");
        let b_spent = deserialized
            .wallet
            .spent_dbcs
            .values()
            .last()
            .expect("There to be a spent DBC.");
        assert_eq!(a_spent, b_spent);

        Ok(())
    }

    #[tokio::test]
    async fn store_created_dbc_gives_file_that_try_load_deposits_can_use() -> Result<()> {
        // Bring in the necessary traits.
        use super::{DepositWallet, SendWallet};

        let sender_root_dir = create_temp_dir();
        let sender_root_dir = sender_root_dir.path().to_path_buf();

        let mut sender = LocalWallet::load_from(&sender_root_dir).await?;
        let sender_dbc =
            create_first_dbc_from_key(&sender.key).expect("Genesis creation to succeed.");
        sender.deposit(vec![sender_dbc]);

        let send_amount = 100;

        // Send to a new address.
        let recipient_root_dir = create_temp_dir();
        let recipient_root_dir = recipient_root_dir.path().to_path_buf();
        let mut recipient = LocalWallet::load_from(&recipient_root_dir).await?;
        let recipient_public_address = recipient.key.public_address();

        let to = vec![(Token::from_nano(send_amount), recipient_public_address)];
        let created_dbcs = sender.send(to, &MockSendClient).await?;
        let dbc = created_dbcs[0].dbc.clone();
        let dbc_id = dbc.id();
        sender.store_created_dbc(dbc).await?;

        let public_address_name = public_address_name(&recipient_public_address);
        let public_address_dir = format!("public_address_{}", hex::encode(public_address_name));
        let dbc_id_name = *DbcAddress::from_dbc_id(&dbc_id).name();
        let dbc_id_file_name = format!("{}.dbc", hex::encode(dbc_id_name));

        let created_dbcs_dir = sender_root_dir.join(WALLET_DIR_NAME).join("created_dbcs");
        let created_dbc_file = created_dbcs_dir
            .join(&public_address_dir)
            .join(&dbc_id_file_name);

        let received_dbc_dir = recipient_root_dir
            .join(WALLET_DIR_NAME)
            .join("received_dbcs")
            .join(&public_address_dir);

        tokio::fs::create_dir_all(&received_dbc_dir).await?;
        let received_dbc_file = received_dbc_dir.join(&dbc_id_file_name);

        // Move the created dbc to the recipient's received_dbcs dir.
        tokio::fs::rename(created_dbc_file, &received_dbc_file).await?;

        assert_eq!(0, recipient.wallet.balance().as_nano());

        recipient.try_load_deposits().await?;

        assert_eq!(1, recipient.wallet.available_dbcs.len());

        let available = recipient
            .wallet
            .available_dbcs
            .values()
            .last()
            .expect("There to be an available DBC.");

        assert_eq!(available.id(), dbc_id);
        assert_eq!(send_amount, recipient.wallet.balance().as_nano());

        Ok(())
    }

    #[derive(Clone)]
    struct MockSendClient;

    #[async_trait::async_trait]
    impl SendClient for MockSendClient {
        async fn send(&self, _transfer: TransferDetails) -> super::Result<()> {
            // Here we just return Ok(()), without network calls,
            // and without sending it to the network.
            Ok(())
        }
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Should be able to create a temp dir.")
    }
}
