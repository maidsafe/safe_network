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

mod error;
mod keys;
mod local_store;
mod transfer;
mod wallet_file;

use crate::transfers::ContentPaymentsIdMap;

pub use self::{
    error::{Error, Result},
    keys::{bls_secret_from_hex, parse_main_pubkey},
    local_store::LocalWallet,
};
pub(crate) use keys::store_new_keypair;
pub use transfer::{CashNoteRedemption, Transfer};

use crate::{NanoTokens, UniquePubkey};
use std::collections::{BTreeMap, BTreeSet};

#[derive(serde::Serialize, serde::Deserialize)]
/// This assumes the CashNotes are stored on disk
pub(super) struct KeyLessWallet {
    /// These are the UniquePubkeys of cash_notes we've owned, that have been
    /// spent when sending tokens to other addresses.
    spent_cash_notes: BTreeSet<UniquePubkey>,
    /// These are the UniquePubkeys of cash_notes we own that are not yet spent.
    available_cash_notes: BTreeMap<UniquePubkey, NanoTokens>,
    /// These are the UniquePubkeys of cash_notes we've created by
    /// sending tokens to other addresses.
    /// They are not owned by us, but we
    /// keep them here so we can track our
    /// transfer history.
    cash_notes_created_for_others: BTreeSet<UniquePubkey>,
    /// Cached proofs of storage transactions made to be used for uploading the paid content.
    payment_transactions: ContentPaymentsIdMap,
}
