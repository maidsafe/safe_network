// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{CreatedDbc, DepositWallet, Result, SendClient, SendWallet};

use crate::protocol::types::transfers::Outputs as TransferDetails;

use sn_dbc::{Dbc, DbcId, DbcIdSource, MainKey, PublicAddress, Token};

use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};

/// A wallet that can send tokens to other addresses.
pub struct LocalSendWallet<C: SendClient> {
    client: C,
    wallet: LocalDepositWallet,
}

/// A wallet that can only receive tokens.
pub struct LocalDepositWallet {
    /// The secret key with which we can access
    /// all the tokens in the available_dbcs.
    key: MainKey,
    /// The current balance of the wallet.
    balance: Token,
    /// These are dbcs we've owned, that have been
    /// spent when sending tokens to other addresses.
    spent_dbcs: BTreeMap<DbcId, Dbc>,
    /// These are the dbcs we own that are not yet spent.
    available_dbcs: BTreeMap<DbcId, Dbc>,
    /// These are the dbcs we've created by
    /// sending tokens to other addresses.
    /// They are not owned by us, but we
    /// keep them here so we can track our
    /// transfer history.
    dbcs_created_for_others: Vec<CreatedDbc>,
    // /// The path to the wallet file.
    // local_store: std::path::PathBuf,
}

impl DepositWallet for LocalDepositWallet {
    fn new(key: MainKey) -> Self {
        Self {
            key,
            balance: Token::zero(),
            spent_dbcs: BTreeMap::new(),
            available_dbcs: BTreeMap::new(),
            dbcs_created_for_others: vec![],
        }
    }

    // /// Loads a serialized wallet from a path.
    // fn load_from(path: &Path) -> Self {
    //     Self {
    //     }
    // }

    fn address(&self) -> PublicAddress {
        self.key.public_address()
    }

    fn balance(&self) -> Token {
        self.balance
    }

    fn new_dbc_address(&self) -> DbcIdSource {
        self.key.random_dbc_id_src(&mut rand::thread_rng())
    }

    fn deposit(&mut self, dbcs: Vec<Dbc>) {
        if dbcs.is_empty() {
            return;
        }

        let mut received_dbcs = dbcs
            .into_iter()
            .filter_map(|dbc| {
                let id = dbc.id();
                (!self.spent_dbcs.contains_key(&id)).then_some((id, dbc))
            })
            .filter_map(|(id, dbc)| dbc.derived_key(&self.key).is_ok().then_some((id, dbc)))
            .collect();

        self.available_dbcs.append(&mut received_dbcs);

        let new_balance = self
            .available_dbcs
            .iter()
            .flat_map(|(_, dbc)| {
                dbc.derived_key(&self.key)
                    .map(|derived_key| (dbc, derived_key))
            })
            .flat_map(|(dbc, derived_key)| dbc.revealed_input(&derived_key))
            .fold(0, |total, amount| total + amount.revealed_amount().value());

        self.balance = Token::from_nano(new_balance);
    }
}

#[async_trait]
impl<C: SendClient + Send + Sync + Clone> SendWallet<C> for LocalSendWallet<C> {
    fn new(key: MainKey, client: C) -> Self {
        Self {
            client,
            wallet: LocalDepositWallet::new(key),
        }
    }

    // /// Loads a serialized wallet from a path.
    // fn load_from(path: &Path) -> Self {
    //     Self {
    //     }
    // }

    fn address(&self) -> PublicAddress {
        self.wallet.address()
    }

    fn balance(&self) -> Token {
        self.wallet.balance()
    }

    fn new_dbc_address(&self) -> DbcIdSource {
        self.wallet.new_dbc_address()
    }

    fn deposit(&mut self, dbcs: Vec<Dbc>) {
        self.wallet.deposit(dbcs)
    }

    async fn send(&mut self, to: Vec<(Token, PublicAddress)>) -> Result<Vec<CreatedDbc>> {
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
            if let Ok(derived_key) = dbc.derived_key(&self.wallet.key) {
                available_dbcs.push((dbc.clone(), derived_key));
            } else {
                warn!(
                    "Skipping DBC {:?} because we don't have the key to spend it",
                    dbc.id()
                );
            }
        }

        let TransferDetails {
            change_dbc,
            created_dbcs,
        } = self.client.send(available_dbcs, to, self.address()).await?;

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

        Ok(created_dbcs)
    }
}

#[cfg(test)]
mod tests {
    use super::{DepositWallet, LocalDepositWallet, Result, SendClient};

    use crate::protocol::types::{
        dbc_genesis::{create_genesis_dbc, GenesisResult, GENESIS_DBC_AMOUNT},
        transfers::{create_transfer, Outputs as TransferDetails},
        wallet::{LocalSendWallet, SendWallet},
    };

    use sn_dbc::{Dbc, DbcIdSource, DerivedKey, MainKey, PublicAddress, Token};

    /// -----------------------------------
    /// <-------> DepositWallet <--------->
    /// -----------------------------------

    #[test]
    fn deposit_wallet_basics() -> Result<()> {
        let main_key = MainKey::random();
        let public_address = main_key.public_address();
        let local_wallet: LocalDepositWallet = DepositWallet::new(main_key);

        assert_eq!(public_address, local_wallet.address());
        assert_eq!(
            public_address,
            local_wallet.new_dbc_address().public_address
        );
        assert_eq!(Token::zero(), local_wallet.balance());

        assert!(local_wallet.available_dbcs.is_empty());
        assert!(local_wallet.dbcs_created_for_others.is_empty());
        assert!(local_wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[test]
    fn deposit_empty_list_does_nothing() -> Result<()> {
        let mut local_wallet: LocalDepositWallet = DepositWallet::new(MainKey::random());

        local_wallet.deposit(vec![]);

        assert_eq!(Token::zero(), local_wallet.balance());

        assert!(local_wallet.available_dbcs.is_empty());
        assert!(local_wallet.dbcs_created_for_others.is_empty());
        assert!(local_wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn deposit_adds_dbcs_that_belongs_to_the_wallet() -> GenesisResult<()> {
        let genesis_key = MainKey::random();
        let genesis = create_genesis_dbc(&genesis_key)?;

        let mut local_wallet: LocalDepositWallet = DepositWallet::new(genesis_key);

        local_wallet.deposit(vec![genesis]);

        assert_eq!(GENESIS_DBC_AMOUNT, local_wallet.balance().as_nano());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn deposit_does_not_add_dbcs_not_belonging_to_the_wallet() -> GenesisResult<()> {
        let genesis_key = MainKey::random();
        let genesis = create_genesis_dbc(&genesis_key)?;

        let wallet_key = MainKey::random();
        let mut local_wallet: LocalDepositWallet = DepositWallet::new(wallet_key);

        local_wallet.deposit(vec![genesis]);

        assert_eq!(Token::zero(), local_wallet.balance());

        Ok(())
    }

    /// --------------------------------
    /// <-------> SendWallet <--------->
    /// --------------------------------

    #[test]
    #[allow(clippy::result_large_err)]
    fn send_wallet_basics() -> GenesisResult<()> {
        let main_key = MainKey::random();
        let public_address = main_key.public_address();
        let client = MockSendClient;
        let send_wallet: LocalSendWallet<MockSendClient> = SendWallet::new(main_key, client);

        assert_eq!(public_address, send_wallet.address());
        assert_eq!(public_address, send_wallet.new_dbc_address().public_address);
        assert_eq!(Token::zero(), send_wallet.balance());

        assert!(send_wallet.wallet.available_dbcs.is_empty());
        assert!(send_wallet.wallet.dbcs_created_for_others.is_empty());
        assert!(send_wallet.wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::result_large_err)]
    async fn sending_decreases_balance() -> Result<()> {
        let sender_main_key = MainKey::random();
        let sender_dbc =
            create_genesis_dbc(&sender_main_key).expect("Genesis creation to succeed.");

        let client = MockSendClient;
        let mut send_wallet: LocalSendWallet<MockSendClient> =
            SendWallet::<MockSendClient>::new(sender_main_key, client);

        send_wallet.deposit(vec![sender_dbc]);

        assert_eq!(GENESIS_DBC_AMOUNT, send_wallet.balance().as_nano());

        // We send to a new address.
        let recipient_main_key = MainKey::random();
        let recipient_public_address = recipient_main_key.public_address();
        let to = vec![(Token::from_nano(100), recipient_public_address)];
        let created_dbcs = send_wallet.send(to).await?;

        assert_eq!(1, created_dbcs.len());
        assert_eq!(GENESIS_DBC_AMOUNT - 100, send_wallet.balance().as_nano());

        let recipient_dbc = &created_dbcs[0];
        assert_eq!(100, recipient_dbc.amount.value());
        assert_eq!(
            &recipient_public_address,
            recipient_dbc.dbc.public_address()
        );

        Ok(())
    }

    #[derive(Clone)]
    struct MockSendClient;

    #[async_trait::async_trait]
    impl SendClient for MockSendClient {
        async fn send(
            &self,
            dbcs: Vec<(Dbc, DerivedKey)>,
            to: Vec<(Token, DbcIdSource)>,
            change_to: PublicAddress,
        ) -> Result<TransferDetails> {
            // Here we just create a transfer, without network calls,
            // and without sending it to the network.
            let transfer = create_transfer(dbcs, to, change_to)
                .expect("There should be no issues creating this transfer.");

            Ok(transfer)
        }
    }
}
