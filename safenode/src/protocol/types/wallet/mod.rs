// Copyright 2023 MaidSafe.net limited.
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
//! 1. Chunk each Dbc, both spent and available.
//! 2. For a semi-public Wallet:
//!     a. Store a register with address of your `PublicAddress`.
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
//!     a. Get the `PublicAddress` from your secret.
//!     b. Fetch the register with address of either the plaintext of or the encrypted `PublicAddress`.
//!     c. Decrypt all entries and apply the ops to your Wallet, to get the current state of it.
//!     d. If there is another register linked at the end of this one, follow that link and repeat steps b., c. and d.
//!
//! We will already now pave for that, by mimicing that flow for the local storage of a Wallet.
//! First though, a simpler local storage will be used. But after that a local register store can be implemented.

mod error;
mod local_store;
mod network_store;

pub use self::{
    error::{Error, Result},
    local_store::{LocalDepositWallet, LocalSendWallet},
    // network_store::NetworkWallet,
};

use super::transfers::{CreatedDbc, Outputs as TransferDetails};

use sn_dbc::{Dbc, DbcIdSource, DerivedKey, MainKey, PublicAddress, Token};

use async_trait::async_trait;

/// A SendClient is used to transfer tokens to other addresses.
///
/// It does so by creating a transfer and returning that to the caller.
/// It is expected that the implementation of this trait is a network client,
/// that will also upload the transfer to the network before returning it.
/// The network will validate the transfer upon receiving it. Once enough peers have accepted it,
/// the transfer is completed.
///  
/// For tests the implementation can be a local client with no network connection,
/// that will just return the transfer to the caller.
#[async_trait]
pub trait SendClient {
    ///
    async fn send(
        &self,
        dbcs: Vec<(Dbc, DerivedKey)>,
        to: Vec<(Token, DbcIdSource)>,
        change_to: PublicAddress,
    ) -> Result<TransferDetails>;
}

/// A send wallet is a wallet that can send tokens to other addresses.
/// It is also a deposit wallet, so it can receive tokens from other wallets.
#[async_trait]
pub trait SendWallet<C: SendClient> {
    // /// Loads a wallet from the given path.
    // fn load_from(path: &Path) -> Self;
    /// Creates a new wallet with the given key.
    fn new(key: MainKey, client: C) -> Self;
    /// The address of the wallet, to which others send tokens.
    fn address(&self) -> PublicAddress;
    /// The current balance of the wallet.
    fn balance(&self) -> Token;
    /// Used to generate a new dbc id for receiving tokens.
    fn new_dbc_address(&self) -> DbcIdSource;
    /// Will only deposit those that are actually accessible by this wallet.
    fn deposit(&mut self, dbcs: Vec<Dbc>);
    /// Sends the given tokens to the given addresses.
    /// Returns the new dbcs that were created.
    /// Depending on the implementation of the send client, this may
    /// also register the transaction with the network.
    async fn send(&mut self, to: Vec<(Token, PublicAddress)>) -> Result<Vec<CreatedDbc>>;
}

/// A deposit wallet is a wallet that can receive tokens from other wallets.
/// It can however not send tokens to other addresses.
pub trait DepositWallet {
    // /// Loads a wallet from the given path.
    // fn load_from(path: &Path) -> Self;
    /// Creates a new wallet with the given key.
    fn new(key: MainKey) -> Self;
    /// The address of the wallet, to which others send tokens.
    fn address(&self) -> PublicAddress;
    /// The current balance of the wallet.
    fn balance(&self) -> Token;
    /// Used to generate a new dbc id for receiving tokens.
    fn new_dbc_address(&self) -> DbcIdSource;
    /// Will only deposit those that are actually accessible by this wallet.
    fn deposit(&mut self, dbcs: Vec<Dbc>);
}
