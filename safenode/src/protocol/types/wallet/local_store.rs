// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Result;

use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};

// use crate::dbcs::DbcReason as Reason; // Later also use Reason in send api.
use sn_dbc::{Dbc, DbcId, DbcIdSource, MainKey, PublicAddress, Token};

///
#[async_trait]
pub trait SendWallet<C: SendClient> {
    ///
    fn new(key: MainKey, client: C) -> Self;
    // fn load_from(path: &Path) -> Self;
    ///
    fn address(&self) -> PublicAddress;
    ///
    fn balance(&self) -> Token;
    /// Used to generate a new dbc id for receiving tokens.
    fn new_dbc_address(&self) -> DbcIdSource;
    /// Will only deposit those that are actually accessible by this wallet.
    fn deposit(&mut self, dbcs: Vec<Dbc>);
    ///
    async fn send(&mut self, to: Vec<(Token, PublicAddress)>) -> Result<Vec<NewDbc>>;
}

///
pub trait DepositWallet {
    ///
    fn new(key: MainKey) -> Self;
    // fn load_from(path: &Path) -> Self;
    ///
    fn address(&self) -> PublicAddress;
    ///
    fn balance(&self) -> Token;
    /// Used to generate a new dbc id for receiving tokens.
    fn new_dbc_address(&self) -> DbcIdSource;
    /// Will only deposit those that are actually accessible by this wallet.
    fn deposit(&mut self, dbcs: Vec<Dbc>);
}

///
pub struct LocalSendWallet<C: SendClient> {
    client: C,
    wallet: LocalDepositWallet,
}

///
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
    output_history: Vec<NewDbc>,
    // local_store: std::path::PathBuf,
}

impl DepositWallet for LocalDepositWallet {
    fn new(key: MainKey) -> Self {
        Self {
            key,
            balance: Token::zero(),
            spent_dbcs: BTreeMap::new(),
            available_dbcs: BTreeMap::new(),
            output_history: vec![],
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

        let mut new_dbcs = dbcs
            .into_iter()
            .filter_map(|dbc| {
                let id = dbc.id();
                (!self.spent_dbcs.contains_key(&id)).then_some((id, dbc))
            })
            .filter_map(|(id, dbc)| {
                dbc.as_revealed_input(&self.key)
                    .is_ok()
                    .then_some((id, dbc))
            })
            .collect();

        self.available_dbcs.append(&mut new_dbcs);

        let new_balance = self
            .available_dbcs
            .iter()
            .flat_map(|(_, dbc)| dbc.as_revealed_input(&self.key))
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

    async fn send(&mut self, to: Vec<(Token, PublicAddress)>) -> Result<Vec<NewDbc>> {
        // do not make a pointless send to ourselves
        let to: Vec<_> = to
            .into_iter()
            .filter_map(|(amount, key)| (key != self.address()).then_some((amount, key)))
            .collect();
        if to.is_empty() {
            return Ok(vec![]);
        }

        let available_dbcs = self.wallet.available_dbcs.values().cloned().collect();

        let client = self.client.clone();
        let SpendInfo {
            change,
            spent_dbcs,
            mut new_dbcs,
        } = client.send(available_dbcs, to).await?; // this should return Result and which dbcs that were spent

        let mut spent_dbcs = spent_dbcs
            .into_iter()
            .filter_map(|id| self.wallet.available_dbcs.remove(&id).map(|dbc| (id, dbc)))
            .collect();

        self.deposit(change.into_iter().collect());
        self.wallet.spent_dbcs.append(&mut spent_dbcs);
        self.wallet.output_history.append(&mut new_dbcs);

        Ok(new_dbcs)
    }
}

///
#[async_trait]
pub trait SendClient {
    ///
    async fn send(&self, dbcs: Vec<Dbc>, to: Vec<(Token, PublicAddress)>) -> Result<SpendInfo>;
}

///
pub struct NewDbc {
    ///
    pub dbc: Dbc,
    ///
    pub recipient: PublicAddress,
    ///
    pub amount: sn_dbc::RevealedAmount,
}

///
pub struct SpendInfo {
    /// Any surplus tokens from the last spent dbc.
    pub change: Option<Dbc>,
    /// The dbcs that were spent when sending
    /// the tokens.
    pub spent_dbcs: BTreeSet<DbcId>,
    /// The dbcs that were created containing
    /// the tokens sent to respective recipient.
    pub new_dbcs: Vec<NewDbc>,
}

#[cfg(test)]
mod tests {
    use super::{DepositWallet, LocalDepositWallet, Result, SendClient, SpendInfo};

    use crate::protocol::types::{dbc_genesis::{
        create_genesis_dbc, Result as GenesisResult, GENESIS_DBC_AMOUNT,
    }, wallet::{LocalSendWallet, SendWallet}};

    use sn_dbc::{Dbc, MainKey, PublicAddress, Token};

    use bls::SecretKey;
    use std::collections::BTreeSet;

    /// -----------------------------------
    /// <-------> DepositWallet <--------->
    /// -----------------------------------

    #[test]
    fn deposit_wallet_basics() -> Result<()> {
        let main_key = MainKey::new(SecretKey::random());
        let public_address = main_key.public_address();
        let local_wallet: LocalDepositWallet = DepositWallet::new(main_key);

        assert_eq!(public_address, local_wallet.address());
        assert_eq!(
            public_address,
            local_wallet.new_dbc_address().public_address
        );
        assert_eq!(Token::zero(), local_wallet.balance());

        assert!(local_wallet.available_dbcs.is_empty());
        assert!(local_wallet.output_history.is_empty());
        assert!(local_wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[test]
    fn deposit_empty_list_does_nothing() -> Result<()> {
        let mut local_wallet: LocalDepositWallet =
            DepositWallet::new(MainKey::new(SecretKey::random()));

        local_wallet.deposit(vec![]);

        assert_eq!(Token::zero(), local_wallet.balance());

        assert!(local_wallet.available_dbcs.is_empty());
        assert!(local_wallet.output_history.is_empty());
        assert!(local_wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn deposit_adds_dbcs_that_belongs_to_the_wallet() -> GenesisResult<()> {
        let genesis_key = MainKey::new(SecretKey::random());
        let genesis = create_genesis_dbc(&genesis_key)?;

        let mut local_wallet: LocalDepositWallet = DepositWallet::new(genesis_key);

        local_wallet.deposit(vec![genesis]);

        assert_eq!(GENESIS_DBC_AMOUNT, local_wallet.balance().as_nano());

        Ok(())
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn deposit_does_not_add_dbcs_not_belonging_to_the_wallet() -> GenesisResult<()> {
        let genesis_key = MainKey::new(SecretKey::random());
        let genesis = create_genesis_dbc(&genesis_key)?;

        let wallet_key = MainKey::new(SecretKey::random());
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
        let main_key = MainKey::new(SecretKey::random());
        let public_address = main_key.public_address();
        let client = MockSendClient::new();
        let send_wallet: LocalSendWallet::<MockSendClient> = SendWallet::<MockSendClient>::new(main_key, client);

        assert_eq!(public_address, send_wallet.address());
        assert_eq!(
            public_address,
            send_wallet.new_dbc_address().public_address
        );
        assert_eq!(Token::zero(), send_wallet.balance());

        assert!(send_wallet.wallet.available_dbcs.is_empty());
        assert!(send_wallet.wallet.output_history.is_empty());
        assert!(send_wallet.wallet.spent_dbcs.is_empty());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::result_large_err)]
    async fn sending_decreases_balance() -> Result<()> {
        let genesis_key = MainKey::new(SecretKey::random());
        let genesis = create_genesis_dbc(&genesis_key).unwrap();

        let client = MockSendClient::new();
        let mut send_wallet: LocalSendWallet::<MockSendClient> = SendWallet::<MockSendClient>::new(genesis_key, client);

        send_wallet.deposit(vec![genesis]);

        assert_eq!(GENESIS_DBC_AMOUNT, send_wallet.balance().as_nano());

        // We send to ourselves.
        let to = vec![(Token::from_nano(100), genesis_key.public_address())];
        let new_dbcs = send_wallet.send(to).await?;

        assert_eq!(GENESIS_DBC_AMOUNT - 100, send_wallet.balance().as_nano());
        assert_eq!(1, new_dbcs.len());
        for n in new_dbcs {
            assert_eq!(100, n.amount.value());
            assert_eq!(genesis_key.public_address(), n.recipient);
        }

        send_wallet.deposit(new_dbcs.iter().map(|n| n.dbc.clone()).collect());

        assert_eq!(GENESIS_DBC_AMOUNT, send_wallet.balance().as_nano());

        Ok(())
    }

    #[derive(Clone)]
    struct MockSendClient {}

    impl MockSendClient {
        fn new() -> Self {
            Self {}
        }
    }

    #[async_trait::async_trait]
    impl SendClient for MockSendClient {
        async fn send(
            &self,
            _dbcs: Vec<Dbc>,
            _to: Vec<(Token, PublicAddress)>,
        ) -> Result<SpendInfo> {
            Ok(SpendInfo {
                change: None,
                spent_dbcs: BTreeSet::new(),
                new_dbcs: vec![],
            })
        }
    }
}
