// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! The uploader module provides an interface to upload data to the network, with the ability to retry failed uploads,
//! make repayments for failed payments, and verify the data after it has been uploaded.
//!
//! # Example
//! let mut uploader = Uploader::new(self.clone(), wallet.clone());
//! uploader.insert_chunks(chunks);
//! uploader.insert_chunks(vec![data_map_chunk]);
//! let summary = uploader.start_upload().await?;
//!
//! # Configuration
//! The `Uploader` can be configured using the `UploadCfg` struct. The most notable options are the `batch_size` and
//! `payment_batch_size` which determine the number of data that are processed in parallel and the number of payments
//! that are made in a single evm transaction, respectively.
//! Also the `max_repayments_for_failed_data` option determines the maximum number of repayments to make if the
//! initial payment fails.

#[cfg(test)]
mod tests;
mod upload;

use crate::client::payments::{PaymentOption, Receipt};
#[cfg(feature = "registers")]
use crate::client::registers::{Register, RegisterError};
use crate::Client;
use itertools::Either;
use sn_evm::{Amount, EvmNetworkTokenError, ProofOfPayment};
use sn_networking::target_arch::{mpsc, mpsc_channel};
use sn_networking::{NetworkError, PayeeQuote};
#[cfg(feature = "data")]
use sn_protocol::storage::{Chunk, ChunkAddress};
use sn_protocol::{storage::RetryStrategy, NetworkAddress};
#[cfg(feature = "registers")]
use sn_registers::RegisterAddress;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    path::PathBuf,
};
use upload::InnerUploader;
use xor_name::XorName;

/// The default batch size that determines the number of data that are processed in parallel.
/// This includes fetching the store cost, uploading and verifying the data.
/// Use PAYMENT_BATCH_SIZE to control the number of payments made in a single transaction.
pub const BATCH_SIZE: usize = 16;

/// The number of payments to make in a single EVM transaction.
pub const PAYMENT_BATCH_SIZE: usize = 512;

/// The number of repayments to attempt for a failed item before returning an error.
/// If value = 1, we do an initial payment & 1 repayment. Thus we make a max 2 payments per data item.
#[cfg(not(test))]
pub(super) const MAX_REPAYMENTS_PER_FAILED_ITEM: usize = 3;
#[cfg(test)]
pub(super) const MAX_REPAYMENTS_PER_FAILED_ITEM: usize = 1;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("Network Token error: {0:?}")]
    EvmNetworkTokenError(#[from] EvmNetworkTokenError),
    #[error("Internal Error")]
    InternalError,
    #[error("Invalid cfg: {0:?}")]
    InvalidCfg(String),
    #[error("I/O error: {0:?}")]
    Io(#[from] std::io::Error),
    #[error("The upload failed with maximum repayments reached for multiple items: {items:?}")]
    MaximumRepaymentsReached { items: Vec<XorName> },
    #[error("The uploader does not have the following items to upload: {missing_items:?}")]
    MissingItemsFromReceipt { missing_items: HashSet<XorName> },
    #[error("Network error: {0:?}")]
    Network(#[from] NetworkError),
    #[cfg(feature = "registers")]
    #[error("Register could not be verified (corrupt)")]
    RegisterFailedVerification,
    #[cfg(feature = "registers")]
    #[error("Failed to write to low-level register")]
    RegisterWrite(#[source] sn_registers::Error),
    #[cfg(feature = "registers")]
    #[error("Failed to sign register")]
    RegisterCouldNotSign(#[source] sn_registers::Error),
    #[error("Multiple consecutive network errors reported during upload")]
    SequentialNetworkErrors,
    #[error("Too many sequential payment errors reported during upload")]
    SequentialUploadPaymentError,
    #[error("Too many sequential upload errors reported during upload")]
    SequentialUploadError,
    #[error("Failed to serialize {0}")]
    Serialization(String),
}

// UploadError is used inside RegisterError, but the uploader emits RegisterError. So this is used to avoid
// recursive enum definition.
#[cfg(feature = "registers")]
impl From<RegisterError> for UploadError {
    fn from(err: RegisterError) -> Self {
        match err {
            RegisterError::Network(err) => Self::Network(err),
            RegisterError::Write(err) => Self::RegisterWrite(err),
            RegisterError::CouldNotSign(err) => Self::RegisterCouldNotSign(err),
            RegisterError::Cost(_) => Self::InternalError,
            RegisterError::Serialization => Self::Serialization("Register".to_string()),
            RegisterError::FailedVerification => Self::RegisterFailedVerification,
            RegisterError::Upload(err) => err,
        }
    }
}

/// The set of options to pass into the `Uploader`
#[derive(Debug, Clone, Copy)]
pub struct UploadCfg {
    pub batch_size: usize,
    pub payment_batch_size: usize,
    pub verify_store: bool,
    pub show_holders: bool,
    pub retry_strategy: RetryStrategy,
    pub max_repayments_for_failed_data: usize,
    #[cfg(feature = "registers")]
    pub collect_registers: bool,
}

impl Default for UploadCfg {
    fn default() -> Self {
        Self {
            batch_size: BATCH_SIZE,
            payment_batch_size: PAYMENT_BATCH_SIZE,
            verify_store: true,
            show_holders: false,
            retry_strategy: RetryStrategy::Balanced,
            max_repayments_for_failed_data: MAX_REPAYMENTS_PER_FAILED_ITEM,
            #[cfg(feature = "registers")]
            collect_registers: false,
        }
    }
}

/// The result of a successful upload.
#[derive(Debug, Clone, Default)]
pub struct UploadSummary {
    pub storage_cost: Amount,
    pub final_balance: Amount,
    pub uploaded_addresses: HashSet<NetworkAddress>,
    #[cfg(feature = "registers")]
    pub uploaded_registers: HashMap<RegisterAddress, Register>,
    /// The number of records that were paid for and uploaded to the network.
    pub uploaded_count: usize,
    /// The number of records that were skipped during because they were already present in the network.
    pub skipped_count: usize,
}

impl UploadSummary {
    /// Merge two UploadSummary together.
    pub fn merge(mut self, other: Self) -> Result<Self, Box<dyn std::error::Error>> {
        self.uploaded_addresses.extend(other.uploaded_addresses);
        #[cfg(feature = "registers")]
        self.uploaded_registers.extend(other.uploaded_registers);

        let summary = Self {
            storage_cost: self
                .storage_cost
                .checked_add(other.storage_cost)
                .ok_or_else(|| {
                    error!("Failed to merge UploadSummary: NumericOverflow");
                    UploadError::InternalError
                })?,
            final_balance: self
                .final_balance
                .checked_add(other.storage_cost)
                .ok_or_else(|| {
                    error!("Failed to merge UploadSummary: NumericOverflow");
                    UploadError::InternalError
                })?,
            uploaded_addresses: self.uploaded_addresses,
            #[cfg(feature = "registers")]
            uploaded_registers: self.uploaded_registers,
            uploaded_count: self.uploaded_count + other.uploaded_count,
            skipped_count: self.skipped_count + other.skipped_count,
        };
        Ok(summary)
    }
}

#[derive(Debug, Clone)]
/// The events emitted from the upload process.
pub enum UploadEvent {
    /// Uploaded a record to the network.
    #[cfg(feature = "data")]
    ChunkUploaded(ChunkAddress),
    /// Uploaded a Register to the network.
    /// The returned register is just the passed in register.
    #[cfg(feature = "registers")]
    RegisterUploaded(Register),
    /// The Chunk already exists in the network. No payments were made.
    #[cfg(feature = "data")]
    ChunkAlreadyExistsInNetwork(ChunkAddress),
    /// The Register already exists in the network. The locally register changes were pushed to the network.
    /// No payments were made.
    /// The returned register contains the remote replica merged with the passed in register.
    #[cfg(feature = "registers")]
    RegisterUpdated(Register),
    /// Payment for a batch of records has been made.
    PaymentMade { tokens_spent: Amount },
    /// The upload process has terminated with an error.
    // Note:  We cannot send the Error enum as it does not implement Clone. So we cannot even do Result<UploadEvent> if
    // we also want to return this error from the function.
    Error,
}

pub struct Uploader {
    // Has to be stored as an Option as we have to take ownership of inner during the upload.
    inner: Option<InnerUploader>,
}

impl Uploader {
    /// Start the upload process.
    pub async fn start_upload(mut self) -> Result<UploadSummary, UploadError> {
        let event_sender = self
            .inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .event_sender
            .clone();
        match upload::start_upload(Box::new(self)).await {
            Err(err) => {
                if let Some(event_sender) = event_sender {
                    if let Err(err) = event_sender.send(UploadEvent::Error).await {
                        error!("Error while emitting event: {err:?}");
                    }
                }
                Err(err)
            }
            Ok(summary) => Ok(summary),
        }
    }

    /// Creates a new instance of `Uploader` with the default configuration.
    /// To modify the configuration, use the provided setter methods (`set_...` functions).
    ///
    /// If `PaymentOption::Receipt` is provided, make sure to add all the items in the receipt to the uploader, using
    /// `insert_chunks` or `insert_register` etc., methods.
    // NOTE: Self has to be constructed only using this method. We expect `Self::inner` is present everywhere.
    pub fn new(client: Client, payment_option: PaymentOption) -> Self {
        Self {
            inner: Some(InnerUploader::new(client, payment_option)),
        }
    }

    /// Update all the configurations by passing the `UploadCfg` struct
    pub fn set_upload_cfg(&mut self, cfg: UploadCfg) {
        // Self can only be constructed with new(), which will set inner to InnerUploader always.
        // So it is okay to call unwrap here.
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_cfg(cfg);
    }

    /// Sets the default batch size that determines the number of data that are processed in parallel.
    ///
    /// By default, this option is set to the constant `BATCH_SIZE: usize = 16`.
    pub fn set_batch_size(&mut self, batch_size: usize) {
        // Self can only be constructed with new(), which will set inner to InnerUploader always.
        // So it is okay to call unwrap here.
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_batch_size(batch_size);
    }

    /// Sets the default  payment batch size that determines the number of payments that are made in a single
    /// transaction. The maximum number of payments that can be made in a single transaction is 512.
    ///
    /// By default, this option is set to the constant `PAYMENT_BATCH_SIZE: usize = 512`.
    pub fn set_payment_batch_size(&mut self, payment_batch_size: usize) {
        // Self can only be constructed with new(), which will set inner to InnerUploader always.
        // So it is okay to call unwrap here.
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_payment_batch_size(payment_batch_size);
    }

    /// Sets the option to verify the data after they have been uploaded.
    ///
    /// By default, this option is set to `true`.
    pub fn set_verify_store(&mut self, verify_store: bool) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_verify_store(verify_store);
    }

    /// Sets the option to display the holders that are expected to be holding the data during verification.
    ///
    /// By default, this option is set to false.
    pub fn set_show_holders(&mut self, show_holders: bool) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_show_holders(show_holders);
    }

    /// Sets the RetryStrategy to increase the re-try during the GetStoreCost & Upload tasks.
    /// This does not affect the retries during the Payment task. Use `set_max_repayments_for_failed_data` to
    /// configure the re-payment attempts.
    ///
    /// By default, this option is set to `RetryStrategy::Quick`
    pub fn set_retry_strategy(&mut self, retry_strategy: RetryStrategy) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_retry_strategy(retry_strategy);
    }

    /// Sets the maximum number of repayments to perform if the initial payment failed.
    /// NOTE: This creates an extra Spend and uses the wallet funds.
    ///
    /// By default, this option is set to `3` repayments.
    pub fn set_max_repayments_for_failed_data(&mut self, retries: usize) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_max_repayments_for_failed_data(retries);
    }

    /// Enables the uploader to return all the registers that were Uploaded or Updated.
    /// The registers are emitted through the event channel whenever they're completed, but this returns them
    /// through the UploadSummary when the whole upload process completes.
    ///
    /// By default, this option is set to `False`
    #[cfg(feature = "registers")]
    pub fn collect_registers(&mut self, collect_registers: bool) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_collect_registers(collect_registers);
    }

    /// Returns a receiver for UploadEvent.
    /// This method is optional and the upload process can be performed without it.
    pub fn get_event_receiver(&mut self) -> mpsc::Receiver<UploadEvent> {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .get_event_receiver()
    }

    /// Insert a list of chunk paths into the uploader.
    ///
    /// Use `start_upload` to start the upload process.
    #[cfg(feature = "fs")]
    pub fn insert_chunk_paths(&mut self, chunks: impl IntoIterator<Item = (XorName, PathBuf)>) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .insert_chunk_paths(chunks);
    }

    /// Insert a list of chunks into the uploader.
    ///
    /// Use `start_upload` to start the upload process.
    #[cfg(feature = "data")]
    pub fn insert_chunks(&mut self, chunks: impl IntoIterator<Item = Chunk>) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .insert_chunks(chunks);
    }

    /// Insert a list of registers into the uploader. To get the updated registers, set `collect_registers` to true.
    ///
    /// Use `start_upload` to start the upload process.
    #[cfg(feature = "registers")]
    pub fn insert_register(&mut self, registers: impl IntoIterator<Item = Register>) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .insert_register(registers);
    }
}

// ======= Private ========

/// An interface to make the testing easier by not interacting with the network.
trait UploaderInterface: Send + Sync {
    fn take_inner_uploader(&mut self) -> InnerUploader;

    // Mutable reference is used in tests.
    fn submit_get_register_task(
        &mut self,
        client: Client,
        reg_addr: RegisterAddress,
        task_result_sender: mpsc::Sender<TaskResult>,
    );

    fn submit_push_register_task(
        &mut self,
        client: Client,
        upload_item: UploadItem,
        verify_store: bool,
        task_result_sender: mpsc::Sender<TaskResult>,
    );

    #[expect(clippy::too_many_arguments)]
    fn submit_get_store_cost_task(
        &mut self,
        client: Client,
        xorname: XorName,
        address: NetworkAddress,
        previous_payments: Option<&Vec<ProofOfPayment>>,
        get_store_cost_strategy: GetStoreCostStrategy,
        max_repayments_for_failed_data: usize,
        task_result_sender: mpsc::Sender<TaskResult>,
    );

    fn submit_make_payment_task(
        &mut self,
        to_send: Option<(UploadItem, Box<PayeeQuote>)>,
        make_payment_sender: mpsc::Sender<Option<(UploadItem, Box<PayeeQuote>)>>,
    );

    fn submit_upload_item_task(
        &mut self,
        upload_item: UploadItem,
        client: Client,
        previous_payments: Option<&Vec<ProofOfPayment>>,
        verify_store: bool,
        retry_strategy: RetryStrategy,
        task_result_sender: mpsc::Sender<TaskResult>,
    );
}

// Configuration functions are used in tests. So these are defined here and re-used inside `Uploader`
impl InnerUploader {
    pub(super) fn set_cfg(&mut self, cfg: UploadCfg) {
        self.cfg = cfg;
    }

    pub(super) fn set_batch_size(&mut self, batch_size: usize) {
        self.cfg.batch_size = batch_size;
    }

    pub(super) fn set_payment_batch_size(&mut self, payment_batch_size: usize) {
        self.cfg.payment_batch_size = payment_batch_size;
    }

    pub(super) fn set_verify_store(&mut self, verify_store: bool) {
        self.cfg.verify_store = verify_store;
    }

    pub(super) fn set_show_holders(&mut self, show_holders: bool) {
        self.cfg.show_holders = show_holders;
    }

    pub(super) fn set_retry_strategy(&mut self, retry_strategy: RetryStrategy) {
        self.cfg.retry_strategy = retry_strategy;
    }

    pub(super) fn set_max_repayments_for_failed_data(&mut self, retries: usize) {
        self.cfg.max_repayments_for_failed_data = retries;
    }

    #[cfg(feature = "registers")]
    pub(super) fn set_collect_registers(&mut self, collect_registers: bool) {
        self.cfg.collect_registers = collect_registers;
    }

    pub(super) fn get_event_receiver(&mut self) -> mpsc::Receiver<UploadEvent> {
        let (tx, rx) = mpsc_channel(100);
        self.event_sender = Some(tx);
        rx
    }

    #[cfg(feature = "fs")]
    pub(super) fn insert_chunk_paths(
        &mut self,
        chunks: impl IntoIterator<Item = (XorName, PathBuf)>,
    ) {
        self.all_upload_items
            .extend(chunks.into_iter().map(|(xorname, path)| {
                let item = UploadItem::Chunk {
                    address: ChunkAddress::new(xorname),
                    chunk: Either::Right(path),
                };
                (xorname, item)
            }));
    }

    #[cfg(feature = "data")]
    pub(super) fn insert_chunks(&mut self, chunks: impl IntoIterator<Item = Chunk>) {
        self.all_upload_items
            .extend(chunks.into_iter().map(|chunk| {
                let xorname = *chunk.name();
                let item = UploadItem::Chunk {
                    address: *chunk.address(),
                    chunk: Either::Left(chunk),
                };
                (xorname, item)
            }));
    }

    #[cfg(feature = "registers")]
    pub(super) fn insert_register(&mut self, registers: impl IntoIterator<Item = Register>) {
        self.all_upload_items
            .extend(registers.into_iter().map(|reg| {
                let address = *reg.address();
                let item = UploadItem::Register { address, reg };
                (address.xorname(), item)
            }));
    }
}

#[derive(Debug, Clone)]
enum UploadItem {
    #[cfg(feature = "data")]
    Chunk {
        address: ChunkAddress,
        // Either the actual chunk or the path to the chunk.
        chunk: Either<Chunk, PathBuf>,
    },
    #[cfg(feature = "registers")]
    Register {
        address: RegisterAddress,
        reg: Register,
    },
}

impl UploadItem {
    fn address(&self) -> NetworkAddress {
        match self {
            #[cfg(feature = "data")]
            Self::Chunk { address, .. } => NetworkAddress::from_chunk_address(*address),
            #[cfg(feature = "registers")]
            Self::Register { address, .. } => NetworkAddress::from_register_address(*address),
        }
    }

    fn xorname(&self) -> XorName {
        match self {
            #[cfg(feature = "data")]
            UploadItem::Chunk { address, .. } => *address.xorname(),
            #[cfg(feature = "registers")]
            UploadItem::Register { address, .. } => address.xorname(),
        }
    }
}

#[derive(Debug)]
enum TaskResult {
    #[cfg(feature = "registers")]
    GetRegisterFromNetworkOk {
        remote_register: Register,
    },
    #[cfg(feature = "registers")]
    GetRegisterFromNetworkErr(XorName),
    #[cfg(feature = "registers")]
    PushRegisterOk {
        updated_register: Register,
    },
    PushRegisterErr(XorName),
    GetStoreCostOk {
        xorname: XorName,
        quote: Box<PayeeQuote>,
    },
    GetStoreCostErr {
        xorname: XorName,
        get_store_cost_strategy: GetStoreCostStrategy,
        max_repayments_reached: bool,
    },
    MakePaymentsOk {
        payment_proofs: Receipt,
    },
    MakePaymentsErr {
        failed_xornames: Vec<(XorName, Box<PayeeQuote>)>,
    },
    UploadOk(XorName),
    UploadErr {
        xorname: XorName,
        io_error: Option<Box<std::io::Error>>,
    },
}

#[derive(Debug, Clone)]
enum GetStoreCostStrategy {
    /// Selects the PeerId with the lowest quote
    Cheapest,
    /// Selects the cheapest PeerId that we have not made payment to.
    SelectDifferentPayee,
}
