// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{NanoTokens, UniquePubkey};
use thiserror::Error;

/// Specialisation of `std::Result`.
pub type Result<T, E = TransferError> = std::result::Result<T, E>;

#[derive(Error, Debug, Clone, PartialEq)]
#[non_exhaustive]
/// Transfer errors
pub enum TransferError {
    #[error("Lost precision on the number of coins during parsing.")]
    LossOfNanoPrecision,
    #[error("The token amount would exceed the maximum value (u64::MAX).")]
    ExcessiveNanoValue,
    #[error("Failed to parse: {0}")]
    FailedToParseNanoToken(String),
    #[error("Invalid Spend: value was tampered with {0:?}")]
    InvalidSpendValue(UniquePubkey),
    #[error("Invalid parent spend: {0}")]
    InvalidParentSpend(String),
    #[error("Parent spend was double spent")]
    DoubleSpentParent,
    #[error("Invalid Spend Signature for {0:?}")]
    InvalidSpendSignature(UniquePubkey),
    #[error("Main key does not match public address.")]
    MainSecretKeyDoesNotMatchMainPubkey,
    #[error("Main pub key does not match.")]
    MainPubkeyMismatch,
    #[error("Could not deserialize specified hex string to a CashNote: {0}")]
    HexDeserializationFailed(String),
    #[error("Could not serialize CashNote to hex: {0}")]
    HexSerializationFailed(String),
    #[error("CashNote must have at least one ancestor.")]
    CashNoteMissingAncestors,
    #[error("The spends don't match the inputs of the Transaction.")]
    SpendsDoNotMatchInputs,
    #[error("Overflow occurred while adding values")]
    NumericOverflow,
    #[error("Not enough balance, {0} available, {1} required")]
    NotEnoughBalance(NanoTokens, NanoTokens),

    #[error("CashNoteRedemption serialisation failed")]
    CashNoteRedemptionSerialisationFailed,
    #[error("CashNoteRedemption decryption failed")]
    CashNoteRedemptionDecryptionFailed,
    #[error("CashNoteRedemption encryption failed")]
    CashNoteRedemptionEncryptionFailed,

    #[error("Transaction serialization error: {0}")]
    TransactionSerialization(String),
    #[error("Unsigned transaction is invalid: {0}")]
    InvalidUnsignedTransaction(String),
    #[error("Cannot create a Transaction with outputs equal to zero")]
    ZeroOutputs,

    #[error("Transfer serialisation failed")]
    TransferSerializationFailed,
    #[error("Transfer deserialisation failed")]
    TransferDeserializationFailed,

    #[error("Bls error: {0}")]
    Blsttc(#[from] bls::error::Error),
    #[error("User name decryption failed")]
    UserNameDecryptFailed,
    #[error("Using invalid decryption key")]
    InvalidDecryptionKey,
    #[error("User name encryption failed")]
    DiscordNameCipherTooBig,
}
