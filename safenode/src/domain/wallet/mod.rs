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
//!
//! ************************************************************************************************************
//!
//! When the client spends a dbc, ie signs the tx, the dbc must be marked locally as spent (ie pending).
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
//! which eventually clears from the mempool and becomes spendable again (which for us is a tx with too few nodes
//! storing it, and eventually is not stored anywhere, which should be extremely rare and not expected ever happen).
//!
//! However, we also have a "mempool", our priority queue. The great difference there though, is that a client _can_
//! update the fee on such a queued spend, and thus can always avoid a spend getting stuck in the queue!
//! Removing it though is not allowed, but we could allow that. If some nodes have already processed it, then
//! the client can't change that fact. But in the case that no node actually had processed it, then the client
//! can successfully remove it from the queue, and then re-add it with a completely different transaction.
//!
//! Note: We do not represent at client the spend being in the priority queue, which we might want to do.
//! In that case, before resending a transfer, we could just check if it has already been stored to the nodes.
//! TBD.

mod error;
mod keys;
mod local_store;
mod network_store;
mod wallet_file;

pub use self::{
    error::{Error, Result},
    keys::parse_public_address,
    local_store::LocalWallet,
    // network_store::NetworkWallet,
};

use super::client_transfers::{CreatedDbc, Outputs as TransferDetails};

use crate::protocol::messages::FeeCiphers;

use sn_dbc::{Dbc, DbcId, DbcIdSource, DerivedKey, PublicAddress, RevealedAmount, Token};

use async_trait::async_trait;
use std::collections::BTreeMap;

/// A SendClient is used to transfer tokens to other addresses.
///
/// It does so by creating a transfer and returning that to the caller.
/// It is expected that the implementation of this trait is a network client,
/// that will also upload the transfer to the network before returning it.
/// The network will validate the transfer upon receiving it. Once enough peers have accepted it,
/// the transfer is completed.
///  
/// For unit tests the implementation can be without network connection,
/// and just return the transfer to the caller.
#[async_trait]
pub trait SendClient: Send + Sync + Clone {
    /// Creates a transfer to send the given tokens to the given addresses,
    /// using the given dbcs as inputs, from which to collect
    /// the necessary number of dbcs, to cover the amounts to send.
    /// This also adds the necessary fee payments to the transfer.
    /// It will return the new dbcs that were created, and the change.
    /// Within the newly created dbcs, there will be the signed spends,
    /// which represent each input dbc that was spent. By that the caller
    /// also knows which of the inputs were spent, and which were not.
    /// The caller can then use this information to update its own state.
    ///
    /// NB: Nothing is registered in the network before sending this transfer to it,
    /// using the `send` api of this trait.
    async fn create_transfer(
        &self,
        dbcs: Vec<(Dbc, DerivedKey)>,
        to: Vec<(Token, DbcIdSource)>,
        change_to: PublicAddress,
    ) -> Result<TransferDetails>;
    /// Registers a created transfer in the network.
    async fn send(&self, transfer: TransferDetails) -> Result<()>;
}

/// A VerifyingClient is used to verify the validity of dbcs on the network.
///
/// It does so by querying for the necessary info of nodes in the network
/// and returning the result to the caller.
/// It is expected that the implementation of this trait is a network client,
/// that will connect to the network before returning the result.
///  
/// For unit tests the implementation can be without network connection,
/// and just return the transfer to the caller.
#[async_trait]
pub trait VerifyingClient: Send + Sync + Clone {
    ///
    async fn verify(&self, dbc: &Dbc) -> Result<()>;
}

/// A wallet has an address and a balance.
pub trait Wallet {
    /// The address of the wallet, to which others send tokens.
    fn address(&self) -> PublicAddress;
    /// The current balance of the wallet.
    fn balance(&self) -> Token;
}

/// A wallet that can sign msgs.
pub trait SigningWallet {
    /// Signs the given msg.
    fn sign(&self, msg: &[u8]) -> bls::Signature;
    /// Decrypts the given fee ciphers.
    /// TODO: Referencing fee ciphers, dbc id and revealed amount here is not ideal.
    fn decrypt(&self, fee_ciphers: &FeeCiphers) -> Result<(DbcId, RevealedAmount)>;
}

/// A send wallet is a wallet that, in addition to the capabilities
/// of a deposit wallet, can also send tokens to other addresses.
#[async_trait]
pub trait SendWallet: DepositWallet {
    /// Sends the given tokens to the given addresses.
    /// Returns the new dbcs that were created.
    /// Depending on the implementation of the send client, this may
    /// also register the transaction with the network.
    async fn send<C: SendClient>(
        &mut self,
        to: Vec<(Token, PublicAddress)>,
        client: &C,
    ) -> Result<Vec<CreatedDbc>>;
}

/// A deposit wallet is a wallet that can receive tokens from other wallets.
/// It can however not send tokens to other addresses.
pub trait DepositWallet: Wallet {
    /// Used to generate a new dbc id for receiving tokens.
    fn new_dbc_address(&self) -> DbcIdSource;
    /// Will only deposit those that are actually accessible by this wallet.
    fn deposit(&mut self, dbcs: Vec<Dbc>);
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct KeyLessWallet {
    /// The current balance of the wallet.
    balance: Token,
    /// These are dbcs we've owned, that have been
    /// spent when sending tokens to other addresses.
    spent_dbcs: BTreeMap<DbcId, Dbc>,
    /// These have not yet been successfully confirmed in
    /// the network and need to be republished, to reach network validity.
    /// We maintain the order they were added in, as to republish
    /// them in the correct order, in case any later spend was
    /// dependent on an earlier spend.
    unconfirmed_txs: Vec<TransferDetails>,
    /// These are the dbcs we own that are not yet spent.
    available_dbcs: BTreeMap<DbcId, Dbc>,
    /// These are the dbcs we've created by
    /// sending tokens to other addresses.
    /// They are not owned by us, but we
    /// keep them here so we can track our
    /// transfer history.
    dbcs_created_for_others: Vec<CreatedDbc>,
}

/// Return the name of a PublicAddress.
pub fn public_address_name(public_address: &PublicAddress) -> xor_name::XorName {
    xor_name::XorName::from_content(&public_address.to_bytes())
}
