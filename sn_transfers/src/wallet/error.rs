// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::UniquePubkey;
use std::{collections::BTreeSet, path::PathBuf};
use thiserror::Error;
use xor_name::XorName;

/// Specialisation of `std::Result`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Transfer errors.
#[derive(Debug, Error)]
pub enum Error {
    /// The cashnotes that were attempted to be spent have already been spent to another address
    #[error("Attempted to reload a wallet from disk, but the disk wallet is not the same as the current wallet. Wallet path: {0}")]
    CurrentAndLoadedKeyMismatch(PathBuf),

    /// The cashnotes that were attempted to be spent have already been spent to another address
    #[error("Double spend attempted with cashnotes: {0:?}")]
    DoubleSpendAttemptedForCashNotes(BTreeSet<UniquePubkey>),

    /// Address provided is of the wrong type
    #[error("Invalid address type")]
    InvalidAddressType,
    /// CashNote add would overflow
    #[error("Total price exceed possible token amount")]
    TotalPriceTooHigh,
    /// A general error when a transfer fails
    #[error("Failed to send tokens due to {0}")]
    CouldNotSendMoney(String),
    /// Failed to sign a transaction
    #[error("Failed to sign a transaction: {0}")]
    CouldNotSignTransaction(String),
    /// A general error when receiving a transfer fails
    #[error("Failed to receive transfer due to {0}")]
    CouldNotReceiveMoney(String),
    /// A general error when verifying a transfer validity in the network
    #[error("Failed to verify transfer validity in the network {0}")]
    CouldNotVerifyTransfer(String),
    /// Failed to fetch spend from network
    #[error("Failed to fetch spend from network: {0}")]
    FailedToGetSpend(String),
    /// Failed to send spend for processing
    #[error("Failed to send spend for processing: {0}")]
    SpendProcessing(String),
    /// Failed to parse bytes into a bls key
    #[error("Unconfirmed transactions still persist even after retries")]
    UnconfirmedTxAfterRetries,
    /// Main pub key doesn't match the key found when loading wallet from path
    #[error("Main pub key doesn't match the key found when loading wallet from path: {0:#?}")]
    PubKeyMismatch(std::path::PathBuf),
    /// Main pub key not found when loading wallet from path
    #[error("Main pub key not found: {0:#?}")]
    PubkeyNotFound(std::path::PathBuf),
    /// Main secret key not found when loading wallet from path
    #[error("Main secret key not found: {0:#?}")]
    MainSecretKeyNotFound(std::path::PathBuf),
    /// Encrypted main secret key not found when loading wallet from path
    #[error("Encrypted main secret key not found: {0:#?}")]
    EncryptedMainSecretKeyNotFound(std::path::PathBuf),
    /// Encrypted main secret key requires a password to decrypt
    #[error("Encrypted main secret key requires a password")]
    EncryptedMainSecretKeyRequiresPassword,
    /// Failed to serialize encrypted secret key
    #[error("Failed to serialize encrypted secret key: {0}")]
    FailedToSerializeEncryptedKey(String),
    /// Failed to deserialize encrypted secret key
    #[error("Failed to deserialize encrypted secret key: {0}")]
    FailedToDeserializeEncryptedKey(String),
    /// Failed to encrypt a secret key
    #[error("Failed to encrypt secret key: {0}")]
    FailedToEncryptKey(String),
    /// Failed to decrypt a secret key
    #[error("Failed to decrypt secret key: {0}")]
    FailedToDecryptKey(String),
    /// Failed to parse bytes into a bls key
    #[error("Failed to parse bls key")]
    FailedToParseBlsKey,
    /// Failed to decode a hex string to a key
    #[error("Could not decode hex string to key")]
    FailedToDecodeHexToKey,
    /// Failed to serialize a main key to hex
    #[error("Could not serialize main key to hex: {0}")]
    FailedToHexEncodeKey(String),
    /// Failed to serialize a cashnote to a hex
    #[error("Could not encode cashnote to hex")]
    FailedToHexEncodeCashNote,
    /// Failed to decypher transfer with our key, maybe it was encrypted to another key
    #[error("Failed to decypher transfer with our key, maybe it was not for us")]
    FailedToDecypherTransfer,
    /// No cached payment found for address
    #[error("No ongoing payment found for address {0:?}")]
    NoPaymentForAddress(XorName),
    /// The payment Quote has expired.
    #[error("The payment quote made for {0:?} has expired")]
    QuoteExpired(XorName),

    /// DAG error
    #[error("DAG error: {0}")]
    Dag(String),
    /// Transfer error
    #[error("Transfer error: {0}")]
    Transfer(#[from] crate::TransferError),
    /// Bls error
    #[error("Bls error: {0}")]
    Bls(#[from] bls::error::Error),
    /// MsgPack serialisation error
    #[error("MsgPack serialisation error:: {0}")]
    Serialisation(#[from] rmp_serde::encode::Error),
    /// MsgPack deserialisation error
    #[error("MsgPack deserialisation error:: {0}")]
    Deserialisation(#[from] rmp_serde::decode::Error),
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Wallet password is incorrect
    #[error("Wallet password is incorrect")]
    WalletPasswordIncorrect,
    /// Wallet is password protected
    #[error("Wallet password required")]
    WalletPasswordRequired,
    /// Wallet password is only valid for a certain time until the user has to provide it again
    #[error("Wallet password expired")]
    WalletPasswordExpired,
    /// Wallet is already encrypted
    #[error("Wallet is already encrypted")]
    WalletAlreadyEncrypted,
}
