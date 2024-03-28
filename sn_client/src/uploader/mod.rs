// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg(test)]
mod tests;
mod upload;

use self::upload::{start_upload, InnerUploader};
use crate::{Client, ClientRegister, Result};
use itertools::Either;
use sn_networking::PayeeQuote;
use sn_protocol::{
    storage::{Chunk, ChunkAddress, RetryStrategy},
    NetworkAddress,
};
use sn_registers::{Register, RegisterAddress};
use sn_transfers::{NanoTokens, WalletApi};
use std::{fmt::Debug, path::PathBuf};
use tokio::sync::mpsc;
use xor_name::XorName;

/// The result of a successful upload.
#[derive(Debug, Clone)]
pub struct UploadSummary {
    pub storage_cost: NanoTokens,
    pub royalty_fees: NanoTokens,
    pub final_balance: NanoTokens,
    pub uploaded_count: usize,
    pub skipped_count: usize,
    pub uploaded_registers: Vec<ClientRegister>,
}

#[derive(Debug, Clone)]
/// The events emitted from the upload process.
pub enum UploadEvent {
    /// Uploaded a record to the network.
    ChunkUploaded(ChunkAddress),
    /// Uploaded a Register to the network.
    /// The returned register is just the passed in register.
    RegisterUploaded(ClientRegister),
    ///
    /// The Chunk already exists in the network. No payments were made.
    ChunkAlreadyExistsInNetwork(ChunkAddress),
    /// The Register already exists in the network. The locally register changes were pushed to the network.
    /// No payments were made.
    /// The returned register contains the remote replica merged with the passed in register.
    RegisterUpdated(ClientRegister),
    /// Payment for a batch of records has been made.
    PaymentMade {
        storage_cost: NanoTokens,
        royalty_fees: NanoTokens,
        new_balance: NanoTokens,
    },
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
    pub async fn start_upload(mut self) -> Result<UploadSummary> {
        let event_sender = self
            .inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .event_sender
            .clone();
        match start_upload(Box::new(self)).await {
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
    // NOTE: Self has to be constructed only using this method. We expect `Self::inner` is present everywhere.
    pub fn new(client: Client, root_dir: PathBuf) -> Self {
        Self {
            inner: Some(InnerUploader::new(client, root_dir)),
        }
    }

    /// Sets the default batch size that determines the number of data that are processed in parallel.
    ///
    /// By default, this option is set to the constant `BATCH_SIZE: usize = 16`.
    pub fn set_batch_size(mut self, batch_size: usize) -> Self {
        // Self can only be constructed with new(), which will set inner to InnerUploader always.
        // So it is okay to call unwrap here.
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_batch_size(batch_size);
        self
    }

    /// Sets the option to verify the data after they have been uploaded.
    ///
    /// By default, this option is set to true.
    pub fn set_verify_store(mut self, verify_store: bool) -> Self {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_verify_store(verify_store);
        self
    }

    /// Sets the option to display the holders that are expected to be holding the data during verification.
    ///
    /// By default, this option is set to false.
    pub fn set_show_holders(mut self, show_holders: bool) -> Self {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_show_holders(show_holders);
        self
    }

    /// Sets the RetryStrategy to increase the re-try during the GetStoreCost & Upload tasks.
    /// This does not affect the retries during the Payment task. Use `set_max_repayments_for_failed_data` to
    /// configure the re-payment attempts.
    ///
    /// By default, this option is set to RetryStrategy::Balanced
    pub fn set_retry_strategy(mut self, retry_strategy: RetryStrategy) -> Self {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_retry_strategy(retry_strategy);
        self
    }

    /// Sets the maximum number of repayments to perform if the initial payment failed.
    /// NOTE: This creates an extra Spend and uses the wallet funds.
    ///
    /// By default, this option is set to 1 retry.
    pub fn set_max_repayments_for_failed_data(mut self, retries: usize) -> Self {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_max_repayments_for_failed_data(retries);
        self
    }

    /// Enables the uploader to return all the registers that were Uploaded or Updated.
    /// The registers are emitted through the event channel whenever they're completed, but this returns them
    /// through the UploadSummary when the whole upload process completes.
    ///
    /// By default, this option is set to False
    pub fn set_collect_registers(mut self, collect_registers: bool) -> Self {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .set_collect_registers(collect_registers);
        self
    }

    /// Returns a receiver for UploadEvent.
    /// This method is optional and the upload process can be performed without it.
    pub fn get_event_receiver(&mut self) -> mpsc::Receiver<UploadEvent> {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .get_event_receiver()
    }

    /// Insert a list of chunk paths to upload to upload.
    pub fn insert_chunk_paths(&mut self, chunks: impl IntoIterator<Item = (XorName, PathBuf)>) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .insert_chunk_paths(chunks);
    }

    /// Insert a list of chunks to upload to upload.
    pub fn insert_chunks(&mut self, chunks: impl IntoIterator<Item = Chunk>) {
        self.inner
            .as_mut()
            .expect("Uploader::new makes sure inner is present")
            .insert_chunks(chunks);
    }

    /// Insert a list of registers to upload.
    pub fn insert_register(&mut self, registers: impl IntoIterator<Item = ClientRegister>) {
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
        upload_item: UploadItem,
        verify_store: bool,
        task_result_sender: mpsc::Sender<TaskResult>,
    );

    #[allow(clippy::too_many_arguments)]
    fn submit_get_store_cost_task(
        &mut self,
        client: Client,
        wallet_api: WalletApi,
        xorname: XorName,
        address: NetworkAddress,
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
        wallet_api: WalletApi,
        verify_store: bool,
        retry_strategy: RetryStrategy,
        task_result_sender: mpsc::Sender<TaskResult>,
    );
}

// Configuration functions are used in tests. So these are defined here and re-used inside `Uploader`
impl InnerUploader {
    pub(super) fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size;
    }

    pub(super) fn set_verify_store(&mut self, verify_store: bool) {
        self.verify_store = verify_store;
    }

    pub(super) fn set_show_holders(&mut self, show_holders: bool) {
        self.show_holders = show_holders;
    }

    pub(super) fn set_retry_strategy(&mut self, retry_strategy: RetryStrategy) {
        self.retry_strategy = retry_strategy;
    }

    pub(super) fn set_max_repayments_for_failed_data(&mut self, retries: usize) {
        self.max_repayments_for_failed_data = retries;
    }

    pub(super) fn get_event_receiver(&mut self) -> mpsc::Receiver<UploadEvent> {
        let (tx, rx) = mpsc::channel(100);
        self.event_sender = Some(tx);
        rx
    }

    pub(super) fn set_collect_registers(&mut self, collect_registers: bool) {
        self.collect_registers = collect_registers;
    }

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

    pub(super) fn insert_register(&mut self, registers: impl IntoIterator<Item = ClientRegister>) {
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
    Chunk {
        address: ChunkAddress,
        // Either the actual chunk or the path to the chunk.
        chunk: Either<Chunk, PathBuf>,
    },
    Register {
        address: RegisterAddress,
        reg: ClientRegister,
    },
}

impl UploadItem {
    fn address(&self) -> NetworkAddress {
        match self {
            Self::Chunk { address, .. } => NetworkAddress::from_chunk_address(*address),
            Self::Register { address, .. } => NetworkAddress::from_register_address(*address),
        }
    }

    fn xorname(&self) -> XorName {
        match self {
            UploadItem::Chunk { address, .. } => *address.xorname(),
            UploadItem::Register { address, .. } => address.xorname(),
        }
    }
}

#[derive(Debug)]
enum TaskResult {
    GetRegisterFromNetworkOk {
        remote_register: Register,
    },
    GetRegisterFromNetworkErr(XorName),
    PushRegisterOk {
        updated_register: ClientRegister,
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
        expired: bool,
    },
    MakePaymentsOk {
        paid_xornames: Vec<XorName>,
        storage_cost: NanoTokens,
        royalty_fees: NanoTokens,
        new_balance: NanoTokens,
    },
    MakePaymentsErr {
        failed_xornames: Vec<(XorName, Box<PayeeQuote>)>,
        insufficient_balance: Option<(NanoTokens, NanoTokens)>,
    },
    UploadOk(XorName),
    UploadErr {
        xorname: XorName,
        quote_expired: bool,
    },
}

#[derive(Debug, Clone)]
enum GetStoreCostStrategy {
    /// Selects the PeerId with the lowest quote
    Cheapest,
    /// Selects the cheapest PeerId that we have not made payment to.
    SelectDifferentPayee,
}
