// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! This module contains the functions for creating an online/offline transfer of tokens.
//! This is done by emptying the input cash_notes, thereby rendering them spent, and creating
//! new cash_notes to the recipients (and a change cash_note if any) containing the transferred tokens.
//! When a transfer is created, it is not yet registered on the network. The signed spends of
//! the transfer is found in the new cash_notes, and must be uploaded to the network to take effect.
//! The peers will validate each signed spend they receive, before accepting it.
//! Once enough peers have accepted all the spends of the transaction, and serve them upon request,
//! the transfer is completed and globally recognised.
//!
//! The transfer is created by selecting from the available input cash_notes, and creating the necessary
//! spends to do so. The input cash_notes are selected by the user, and the spends are created by this
//! module. The user can select the input cash_notes by specifying the amount of tokens they want to
//! transfer, and the module will select the necessary cash_notes to transfer that amount. The user can
//! also specify the amount of tokens they want to transfer to each recipient, and the module will
//! select the necessary cash_notes to transfer that amount to each recipient.
//!
//! On the difference between a transfer and a transaction.
//! The difference is subtle, but very much there. A transfer is a higher level concept, it is the
//! sending of tokens from one address to another. Or many.
//! A cash_note transaction is the lower layer concept where the blinded inputs and outputs are specified.

mod error;
mod transfer;

use std::collections::BTreeMap;

use xor_name::XorName;

use crate::{CashNote, SignedSpend, Transaction, UniquePubkey};

pub use self::error::{Error, Result};
pub type ContentPaymentsIdMap = BTreeMap<XorName, Vec<UniquePubkey>>;

/// Utility function to create an offline transfer
pub use self::transfer::create_offline_transfer;

/// Offline Transfer
/// This struct contains all the necessary information to carry out the transfer.
/// The created cash_notes and change cash_note from a transfer
/// of tokens from one or more cash_notes, into one or more new cash_notes.
#[derive(custom_debug::Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OfflineTransfer {
    /// This is the transaction where all the below
    /// spends were made and cash_notes created.
    pub tx: Transaction,
    /// The cash_notes that were created containing
    /// the tokens sent to respective recipient.
    #[debug(skip)]
    pub created_cash_notes: Vec<CashNote>,
    /// The cash_note holding surplus tokens after
    /// spending the necessary input cash_notes.
    #[debug(skip)]
    pub change_cash_note: Option<CashNote>,
    /// The parameters necessary to send all spend requests to the network.
    pub all_spend_requests: Vec<SpendRequest>,
}

/// The parameters necessary to send a spend request to the network.
#[derive(
    custom_debug::Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct SpendRequest {
    /// The cash_note to register in the network as spent.
    pub signed_spend: SignedSpend,
    /// The cash_note transaction that the spent cash_note was created in.
    #[debug(skip)]
    pub parent_tx: Transaction,
}
