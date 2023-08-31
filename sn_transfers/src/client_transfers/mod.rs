// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! This module contains the functions for creating an online/offline transfer of tokens.
//! This is done by emptying the input dbcs, thereby rendering them spent, and creating
//! new dbcs to the recipients (and a change dbc if any) containing the transferred tokens.
//! When a transfer is created, it is not yet registered on the network. The signed spends of
//! the transfer is found in the new dbcs, and must be uploaded to the network to take effect.
//! The peers will validate each signed spend they receive, before accepting it.
//! Once enough peers have accepted all the spends of the transaction, and serve them upon request,
//! the transfer is completed and globally recognised.
//!
//! The transfer is created by selecting from the available input dbcs, and creating the necessary
//! spends to do so. The input dbcs are selected by the user, and the spends are created by this
//! module. The user can select the input dbcs by specifying the amount of tokens they want to
//! transfer, and the module will select the necessary dbcs to transfer that amount. The user can
//! also specify the amount of tokens they want to transfer to each recipient, and the module will
//! select the necessary dbcs to transfer that amount to each recipient.
//!
//! On the difference between a transfer and a transaction.
//! The difference is subtle, but very much there. A transfer is a higher level concept, it is the
//! sending of tokens from one address to another. Or many.
//! A dbc transaction is the lower layer concept where the blinded inputs and outputs are specified.

mod error;
mod transfer;

use std::collections::BTreeMap;

pub(crate) use self::error::{Error, Result};
pub use self::transfer::create_transfer;

use sn_dbc::{
    Dbc, DbcId, DbcTransaction, DerivationIndex, DerivedKey, PublicAddress, SignedSpend, Token,
};
use sn_protocol::NetworkAddress;

pub type ContentPaymentsIdMap = BTreeMap<NetworkAddress, Vec<DbcId>>;

/// The input details necessary to
/// carry out a transfer of tokens.
#[derive(Debug)]
pub struct Inputs {
    /// The selected dbcs to spend, with the necessary amounts contained
    /// to transfer the below specified amount of tokens to each recipients.
    pub dbcs_to_spend: Vec<(Dbc, DerivedKey)>,
    /// The amounts and dbc ids for the dbcs that will be created to hold the transferred tokens.
    pub recipients: Vec<(Token, PublicAddress, DerivationIndex)>,
    /// Any surplus amount after spending the necessary input dbcs.
    pub change: (Token, PublicAddress),
}

/// The created dbcs and change dbc from a transfer
/// of tokens from one or more dbcs, into one or more new dbcs.
#[derive(custom_debug::Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TransferOutputs {
    /// This is the transaction where all the below
    /// spends were made and dbcs created.
    pub tx: sn_dbc::DbcTransaction,
    /// The dbcs that were created containing
    /// the tokens sent to respective recipient.
    #[debug(skip)]
    pub created_dbcs: Vec<Dbc>,
    /// The dbc holding surplus tokens after
    /// spending the necessary input dbcs.
    #[debug(skip)]
    pub change_dbc: Option<Dbc>,
    /// The parameters necessary to send all spend requests to the network.
    pub all_spend_requests: Vec<SpendRequest>,
}

/// The parameters necessary to send a spend request to the network.
#[derive(
    custom_debug::Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct SpendRequest {
    /// The dbc to register in the network as spent.
    pub signed_spend: SignedSpend,
    /// The dbc transaction that the spent dbc was created in.
    #[debug(skip)]
    pub parent_tx: DbcTransaction,
}
