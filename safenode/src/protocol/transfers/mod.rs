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
mod offline;
mod online;

#[cfg(test)]
pub(crate) use self::offline::create_transfer as create_offline_transfer;

pub(crate) use self::{
    error::{Error, Result},
    online::create_transfer as create_online_transfer,
};

use super::fees::{FeeCiphers, RequiredFee};

use crate::node::NodeId;

use sn_dbc::{
    Dbc, DbcId, DbcIdSource, DbcTransaction, DerivedKey, PublicAddress, RevealedAmount,
    SignedSpend, Token,
};

use std::collections::BTreeMap;

type FeeCiphersParams = BTreeMap<DbcId, BTreeMap<NodeId, (RequiredFee, DbcIdSource)>>;

/// The input details necessary to
/// carry out a transfer of tokens.
#[derive(Debug)]
pub struct Inputs {
    /// The selected dbcs to spend, with the necessary amounts contained
    /// to transfer the below specified amount of tokens to each recipients.
    pub dbcs_to_spend: Vec<(Dbc, DerivedKey)>,
    /// The amounts and dbc ids for the dbcs that will be created to hold the transferred tokens.
    pub recipients: Vec<(Token, DbcIdSource)>,
    /// Any surplus amount after spending the necessary input dbcs.
    pub change: (Token, PublicAddress),
    /// This is the set of input dbc keys, each having a set of
    /// node ids and their respective fee ciphers.
    /// Used to produce the fee ciphers for the spends.
    pub inputs_fee_cipher_params: FeeCiphersParams,
}

/// The created dbcs and change dbc from a transfer
/// of tokens from one or more dbcs, into one or more new dbcs.
#[derive(Debug)]
pub struct Outputs {
    /// The dbcs that were created containing
    /// the tokens sent to respective recipient.
    pub created_dbcs: Vec<CreatedDbc>,
    /// The dbc holding surplus tokens after
    /// spending the necessary input dbcs.
    pub change_dbc: Option<Dbc>,
    /// The parameters necessary to send all spend requests to the network.
    pub all_spend_request_params: Vec<SpendRequestParams>,
}

/// The parameters necessary to send a spend request to the network.
#[derive(Debug, Clone)]
pub struct SpendRequestParams {
    /// The dbc to register in the network as spent.
    pub signed_spend: SignedSpend,
    /// The dbc transaction that the spent dbc was created in.
    pub parent_tx: DbcTransaction,
    /// This is the set of node ids and their respective fee ciphers.
    /// Sent together with spends, so that nodes can verify their fee payments.
    pub fee_ciphers: BTreeMap<NodeId, FeeCiphers>,
}

/// A resulting dbc from a token transfer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatedDbc {
    /// The dbc that was created.
    pub dbc: Dbc,
    /// This is useful for the sender to know how much they sent to each recipient.
    /// They can't know this from the dbc itself, as the amount is encrypted.
    pub amount: RevealedAmount,
}
