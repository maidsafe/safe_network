// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Hash, NanoTokens, UniquePubkey};
use thiserror::Error;

/// Specialisation of `std::Result`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug, Clone, PartialEq)]
#[non_exhaustive]
/// Node error variants.
pub enum Error {
    /// While parsing a `Nano`, precision would be lost.
    #[error("Lost precision on the number of coins during parsing.")]
    LossOfNanoPrecision,
    /// The amount would exceed the maximum value for `Nano` (u64::MAX).
    #[error("The token amount would exceed the maximum value (u64::MAX).")]
    ExcessiveNanoValue,
    /// Failed to parse a `NanoToken` from a string.
    #[error("Failed to parse: {0}")]
    FailedToParseNanoToken(String),

    #[error("Invalid Spend: value was tampered with {0:?}")]
    InvalidSpendValue(UniquePubkey),
    #[error("Invalid Spend Signature for {0:?}")]
    InvalidSpendSignature(UniquePubkey),
    #[error("Transaction hash is different from the hash in the the Spend: {0:?} != {1:?}")]
    TransactionHashMismatch(Hash, Hash),
    #[error("CashNote ciphers are not present in transaction outputs.")]
    CashNoteCiphersNotPresentInTransactionOutput,
    #[error("Output not found in transaction outputs.")]
    OutputNotFound,
    #[error("UniquePubkey is not unique across all transaction inputs and outputs.")]
    UniquePubkeyNotUniqueInTx,
    #[error("The number of SignedSpend ({got}) does not match the number of inputs ({expected}).")]
    SignedSpendInputLenMismatch { got: usize, expected: usize },
    #[error("A SignedSpend UniquePubkey does not match an MlsagSignature UniquePubkey.")]
    SignedSpendInputIdMismatch,
    #[error("SignedSpends for {0:?} have mismatching reasons.")]
    SignedSpendReasonMismatch(UniquePubkey),
    #[error("Decryption failed.")]
    DecryptionBySecretKeyFailed,
    #[error("UniquePubkey not found.")]
    UniquePubkeyNotFound,
    #[error("Main key does not match public address.")]
    MainSecretKeyDoesNotMatchMainPubkey,
    #[error("Main pub key does not match.")]
    MainPubkeyMismatch,
    #[error("Could not deserialize specified hex string to a CashNote: {0}")]
    HexDeserializationFailed(String),
    #[error("Could not serialize CashNote to hex: {0}")]
    HexSerializationFailed(String),
    #[error("The input and output amounts of the tx do not match.")]
    UnbalancedTransaction,
    #[error("The CashNote tx must have at least one input.")]
    MissingTxInputs,
    #[error("The spends don't match the inputs of the Transaction.")]
    SpendsDoNotMatchInputs,
    #[error("Overflow occurred while adding values")]
    NumericOverflow,

    /// Not enough balance to perform a transaction
    #[error("Not enough balance, {0} available, {1} required")]
    NotEnoughBalance(NanoTokens, NanoTokens),
    #[error("CashNoteHasNoParentSpends: {0}")]
    CashNoteReissueFailed(String),
    #[error("CashNote has no parent spends")]
    CashNoteHasNoParentSpends,
    #[error("CashNoteRedemption serialisation failed")]
    CashNoteRedemptionSerialisationFailed,
    #[error("CashNoteRedemption decryption failed")]
    CashNoteRedemptionDecryptionFailed,
    #[error("CashNoteRedemption encryption failed")]
    CashNoteRedemptionEncryptionFailed,
    #[error("We are not a recipient of this Transfer")]
    NotRecipient,
    #[error("Transfer serialisation failed")]
    TransferSerializationFailed,
    #[error("Transfer deserialisation failed")]
    TransferDeserializationFailed,

    #[error("Bls error: {0}")]
    Blsttc(#[from] bls::error::Error),
}
