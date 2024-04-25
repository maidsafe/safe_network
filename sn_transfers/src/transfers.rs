// Copyright 2024 MaidSafe.net limited.
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

mod offline_transfer;
mod transfer;

pub use offline_transfer::{
    create_unsigned_transfer, CashNotesAndSecretKey, OfflineTransfer, TransferRecipientDetails,
};
pub use transfer::{CashNoteRedemption, Transfer};
