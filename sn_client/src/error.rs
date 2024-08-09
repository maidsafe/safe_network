// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(crate) type Result<T> = std::result::Result<T, Error>;

use crate::UploadSummary;

use super::ClientEvent;
use sn_protocol::NetworkAddress;
use sn_registers::{Entry, EntryHash};
use sn_transfers::SpendAddress;
use std::collections::BTreeSet;
use thiserror::Error;
use tokio::time::Duration;
use xor_name::XorName;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Genesis disbursement failed")]
    GenesisDisbursement,

    #[error("Genesis error {0}")]
    GenesisError(#[from] sn_transfers::GenesisError),

    #[error("Wallet Error {0}.")]
    Wallet(#[from] sn_transfers::WalletError),

    #[error("Transfer Error {0}.")]
    Transfer(#[from] sn_transfers::TransferError),

    #[error("Network Error {0}.")]
    Network(#[from] sn_networking::NetworkError),

    #[error("Protocol error {0}.")]
    Protocol(#[from] sn_protocol::error::Error),

    #[error("Register error {0}.")]
    Register(#[from] sn_registers::Error),

    #[error("Chunks error {0}.")]
    Chunks(#[from] super::chunks::Error),

    #[error("No cashnote found at {0:?}.")]
    NoCashNoteFound(SpendAddress),

    #[error("Decrypting a Folder's item failed: {0}")]
    FolderEntryDecryption(EntryHash),

    #[error("SelfEncryption Error {0}.")]
    SelfEncryptionIO(#[from] self_encryption::Error),

    #[error("System IO Error {0}.")]
    SystemIO(#[from] std::io::Error),

    #[error("Events receiver error {0}.")]
    EventsReceiver(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Events sender error {0}.")]
    EventsSender(#[from] tokio::sync::broadcast::error::SendError<ClientEvent>),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Invalid DAG")]
    InvalidDag,
    #[error("Serialization error: {0:?}")]
    Serialization(#[from] rmp_serde::encode::Error),
    #[error("Deserialization error: {0:?}")]
    Deserialization(#[from] rmp_serde::decode::Error),

    #[error(
        "Content branches detected in the Register which need to be merged/resolved by user. \
        Entries hashes of branches are: {0:?}"
    )]
    ContentBranchDetected(BTreeSet<(EntryHash, Entry)>),

    #[error("The provided amount contains zero nanos")]
    AmountIsZero,

    #[error("The payee for the address {0:?} was not found.")]
    PayeeNotFound(NetworkAddress),

    /// CashNote add would overflow
    #[error("Total price exceed possible token amount")]
    TotalPriceTooHigh,

    #[error("Logic error: NonZeroUsize was initialised as zero")]
    NonZeroUsizeWasInitialisedAsZero,

    #[error("Could not connect to the network in {0:?}")]
    ConnectionTimeout(Duration),

    #[error("Could not send files event")]
    CouldNotSendFilesEvent,

    #[error("Incorrect Download Option")]
    IncorrectDownloadOption,

    #[error("The provided data map is empty")]
    EmptyDataMap,

    #[error("Error occurred while assembling the downloaded chunks")]
    FailedToAssembleDownloadedChunks,

    #[error("Task completion notification channel is done")]
    FailedToReadFromNotificationChannel,

    #[error("Could not find register after batch sync: {0:?}")]
    RegisterNotFoundAfterUpload(XorName),

    #[error("Could not connect due to incompatible network protocols. Our protocol: {0} Network protocol: {1}")]
    UnsupportedProtocol(String, String),

    // ------ Upload Errors --------
    #[error("Overflow occurred while adding values")]
    NumericOverflow,

    #[error("Uploadable item not found: {0:?}")]
    UploadableItemNotFound(XorName),

    #[error("Invalid upload item found")]
    InvalidUploadItemFound,

    #[error("The state tracked by the uploader is empty")]
    UploadStateTrackerIsEmpty,

    #[error("Internal task channel dropped")]
    InternalTaskChannelDropped,

    #[error("Multiple consecutive network errors reported during upload")]
    SequentialNetworkErrors,

    #[error("Too many sequential payment errors reported during upload")]
    SequentialUploadPaymentError,

    #[error("The maximum specified repayments has been reached for a single item: {0:?}")]
    MaximumRepaymentsReached(XorName),

    #[error("The upload failed with maximum repayments reached for multiple items: {items:?} Summary: {summary:?}")]
    UploadFailedWithMaximumRepaymentsReached {
        items: Vec<XorName>,
        summary: UploadSummary,
    },

    #[error("Error occurred when access wallet file")]
    FailedToAccessWallet,

    #[error("Error parsing entropy for mnemonic phrase")]
    FailedToParseEntropy,

    #[error("Error parsing mnemonic phrase")]
    FailedToParseMnemonic,

    #[error("Invalid mnemonic seed phrase")]
    InvalidMnemonicSeedPhrase,

    #[error("SecretKey could not be created from the provided bytes")]
    InvalidKeyBytes,
}
