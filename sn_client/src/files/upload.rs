// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::PathBuf,
};

use bytes::Bytes;
use libp2p::PeerId;
use tokio::sync::mpsc::{self};
use xor_name::XorName;

use sn_networking::PayeeQuote;
use sn_protocol::{
    NetworkAddress,
    storage::{Chunk, ChunkAddress, RetryStrategy},
};
use sn_transfers::NanoTokens;

use crate::{
    BATCH_SIZE,
    error::{Error as ClientError, Result}, FilesApi,
};

/// The maximum number of sequential payment failures before aborting the upload process.
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 3;

/// The events emitted from the upload process.
pub enum FileUploadEvent {
    /// Uploaded a Chunk to the network
    Uploaded(ChunkAddress),
    /// The Chunk already exists in the network, skipping upload.
    AlreadyExistsInNetwork(ChunkAddress),
    /// Failed to upload a chunk to the network. This event can be emitted multiple times for a single ChunkAddress
    /// if retries are enabled.
    FailedToUpload(ChunkAddress),
    /// Payment for a batch of chunk has been made.
    PayedForChunks {
        storage_cost: NanoTokens,
        royalty_fees: NanoTokens,
        new_balance: NanoTokens,
    },
    /// The upload process has terminated with an error.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ChunkInfo {
    name: XorName,
    path: PathBuf,
}

#[allow(clippy::large_enum_variant)]
enum TaskResult {
    GetStoreCostOK((ChunkInfo, PayeeQuote)),
    // (store_cost, royalty_fee, new_wallet_balance)
    MakePaymentsOK(
        (
            (NanoTokens, NanoTokens, NanoTokens),
            Vec<(ChunkInfo, PeerId)>,
        ),
    ),
    UploadChunksOK(XorName),
    ErrorEntries(Vec<ChunkInfo>),
}

/// `FilesUpload` provides functionality for uploading chunks with support for retries and queuing.
/// This struct is not cloneable. To create a new instance with default configuration, use the `new` function.
/// To modify the configuration, use the provided setter methods (`set_...` functions).
pub struct FilesUpload {
    // Configurations
    batch_size: usize,
    verify_store: bool,
    show_holders: bool,
    retry_strategy: RetryStrategy,
    // API
    api: FilesApi,
    // Uploads
    failed_chunks: HashSet<ChunkInfo>,
    // Upload stats
    upload_storage_cost: NanoTokens,
    upload_royalty_fees: NanoTokens,
    upload_final_balance: NanoTokens,
    // Events
    event_sender: Option<mpsc::Sender<FileUploadEvent>>,
    logged_event_sender_absence: bool,
}

impl FilesUpload {
    /// Creates a new instance of `FilesUpload` with the default configuration.
    /// To modify the configuration, use the provided setter methods (`set_...` functions).
    pub fn new(files_api: FilesApi) -> Self {
        Self {
            batch_size: BATCH_SIZE,
            verify_store: true,
            show_holders: false,
            retry_strategy: RetryStrategy::Balanced,
            api: files_api,
            failed_chunks: Default::default(),
            upload_storage_cost: NanoTokens::zero(),
            upload_royalty_fees: NanoTokens::zero(),
            upload_final_balance: NanoTokens::zero(),
            event_sender: None,
            logged_event_sender_absence: false,
        }
    }

    /// Sets the default batch size that determines the number of chunks that are processed in parallel during the
    /// payment and upload process.
    ///
    /// By default, this option is set to the constant `BATCH_SIZE: usize = 64`.
    pub fn set_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Sets the option to verify the chunks after they have been uploaded.
    ///
    /// By default, this option is set to true.
    pub fn set_verify_store(mut self, verify_store: bool) -> Self {
        self.verify_store = verify_store;
        self
    }

    /// Sets the option to display the holders that are expected to be holding a chunk during verification.
    ///
    /// By default, this option is set to false.
    pub fn set_show_holders(mut self, show_holders: bool) -> Self {
        self.show_holders = show_holders;
        self
    }

    /// Sets the RetryStrategy to increase the re-try on failure attempts.
    ///
    /// By default, this option is set to RetryStrategy::Balanced
    pub fn set_retry_strategy(mut self, retry_strategy: RetryStrategy) -> Self {
        self.retry_strategy = retry_strategy;
        self
    }

    /// Returns a receiver for file upload events.
    /// This method is optional and the upload process can be performed without it.
    pub fn get_upload_events(&mut self) -> mpsc::Receiver<FileUploadEvent> {
        let (event_sender, event_receiver) = mpsc::channel(10);
        // should we return error if an sender is already set?
        self.event_sender = Some(event_sender);

        event_receiver
    }

    /// Returns the total amount of fees paid for storage after the upload completes.
    pub fn get_upload_storage_cost(&self) -> NanoTokens {
        self.upload_storage_cost
    }
    /// Returns the total amount of royalties paid after the upload completes.
    pub fn get_upload_royalty_fees(&self) -> NanoTokens {
        self.upload_royalty_fees
    }

    /// Returns the final wallet balance after the upload completes.
    pub fn get_upload_final_balance(&self) -> NanoTokens {
        self.upload_final_balance
    }

    /// Uploads the provided chunks to the network.
    /// If you want to track the upload progress, use the `get_upload_events` method.
    pub async fn upload_chunks(&mut self, mut chunks: Vec<(XorName, PathBuf)>) -> Result<()> {
        trace!("Uploading chunks {:?}", chunks.len());

        // make sure we log that the event sender is absent atleast once
        self.logged_event_sender_absence = false;

        // clean up the trackers/stats
        self.failed_chunks = Default::default();
        self.upload_storage_cost = NanoTokens::zero();
        self.upload_royalty_fees = NanoTokens::zero();
        self.upload_final_balance = NanoTokens::zero();

        let mut result;
        let mut last_failed = 0;
        let mut for_failed_chunks = false;
        loop {
            result = self.upload(chunks, for_failed_chunks).await;

            // Any error raised from `upload` function is un-recoverable
            // and shall terminate the overall upload immediately.
            if result.is_err() {
                error!("Upload terminated due to un-recoverable error {result:?}");
                println!("Upload terminated due to un-recoverable error {result:?}");
                // send an event indicating that the upload process completed with an error
                self.send_event(FileUploadEvent::Error);
                break;
            }

            chunks = self
                .take_failed_chunks()
                .iter()
                .map(|chunk_info| (chunk_info.name, chunk_info.path.clone()))
                .collect();
            if chunks.len() == last_failed {
                // Terminate the flow whenever there is no progress with failed_chunks
                // It could be there is no failure at all (last_failed being 0)
                // Or payment/network issue that rejected any uploads
                break;
            }

            for_failed_chunks = true;
            last_failed = chunks.len();
            warn!("Retrying failed chunks {last_failed} ...");
            println!("Retrying failed chunks {last_failed} ...");
        }

        // drop the sender to close the channel.
        let sender = self.event_sender.take();
        drop(sender);

        result
    }

    /// There are three main task groups to upload chunks:
    ///   1, Fetch the store_cost of a chunk
    ///   2, Pay for the chunk based on the fetched store_cost
    ///   3, Upload the chunk
    /// Task groups can be run in parallel to each other, however sometimes requires input from previous group.
    /// Within each group, for group 1 and 3, the mini tasks inside can be parallel to each other.
    /// However for group 2, mini tasks inside has to be undertaken sequentially.
    async fn upload(
        &mut self,
        mut chunks: Vec<(XorName, PathBuf)>,
        for_failed_chunks: bool,
    ) -> Result<()> {
        let mut pending_to_pay: Vec<(ChunkInfo, PayeeQuote)> = vec![];
        let mut pending_to_upload: Vec<(ChunkInfo, PeerId)> = vec![];
        let mut uploaded = 0;
        let mut skipped = 0;
        let mut on_going_get_cost = BTreeSet::new();
        let mut on_going_pay_for_chunk = BTreeSet::new();
        let mut on_going_uploadings = BTreeSet::new();
        let mut sequential_payment_fails = 0;

        let batch_size = self.batch_size;
        let total_chunks = chunks.len();

        let (get_store_cost_sender, mut get_store_cost_receiver) = mpsc::channel(batch_size);
        let (paid_chunk_sender, mut paid_chunk_receiver) = mpsc::channel::<TaskResult>(batch_size);
        let (paying_work_sender, paying_work_receiver) = mpsc::channel(batch_size);
        let (upload_chunk_sender, mut upload_chunk_receiver) = mpsc::channel(batch_size);

        self.spawn_paying_thread(paying_work_receiver, paid_chunk_sender, batch_size);

        loop {
            if uploaded + skipped + self.failed_chunks.len() == total_chunks {
                // To avoid empty final_balance when all chunks are skipped.
                self.upload_final_balance = self.api.wallet()?.balance();
                return Ok(());
            }

            while !chunks.is_empty()
                && on_going_get_cost.len() < batch_size
                && pending_to_pay.len() < batch_size
            {
                if let Some((name, path)) = chunks.pop() {
                    let _ = on_going_get_cost.insert(name);
                    self.spawn_get_store_cost_task(
                        ChunkInfo { name, path },
                        get_store_cost_sender.clone(),
                    );
                }
            }

            while !pending_to_pay.is_empty()
                && on_going_pay_for_chunk.len() < batch_size
                && pending_to_upload.len() < batch_size
            {
                if let Some(to_pay) = pending_to_pay.pop() {
                    let _ = on_going_pay_for_chunk.insert(to_pay.0.name);
                    let paying_work_sender_clone = paying_work_sender.clone();
                    let _handle = tokio::spawn(async move {
                        let _ = paying_work_sender_clone.send(Some(to_pay)).await;
                    });
                }
            }

            while !pending_to_upload.is_empty() && on_going_uploadings.len() < batch_size {
                if let Some((chunk_info, payee)) = pending_to_upload.pop() {
                    let _ = on_going_uploadings.insert(chunk_info.name);
                    self.spawn_upload_chunk_task(chunk_info, payee, upload_chunk_sender.clone());
                }
            }

            if chunks.is_empty() && !on_going_pay_for_chunk.is_empty() {
                // Fire None to trigger a forced round of making leftover payments.
                let paying_work_sender_clone = paying_work_sender.clone();
                let _handle = tokio::spawn(async move {
                    let _ = paying_work_sender_clone.send(None).await;
                });
            }

            let task_result = if let Some(result) = progress_tasks(
                &mut get_store_cost_receiver,
                &mut paid_chunk_receiver,
                &mut upload_chunk_receiver,
            )
            .await
            {
                result
            } else {
                return Err(ClientError::FailedToReadFromNotificationChannel);
            };

            match task_result {
                TaskResult::GetStoreCostOK((chunk_info, cost)) => {
                    let _ = on_going_get_cost.remove(&chunk_info.name);
                    trace!(
                        "Upload task got chunk {:?}'s store_cost {:?}",
                        chunk_info.name,
                        cost.quote.cost
                    );
                    if cost.quote.cost != NanoTokens::zero() {
                        pending_to_pay.push((chunk_info, cost));
                    } else if for_failed_chunks {
                        // the chunk was already paid for but might have not been verified on the first try.
                        self.send_event(FileUploadEvent::Uploaded(ChunkAddress::new(
                            chunk_info.name,
                        )));
                        uploaded += 1;
                    } else {
                        // if during the first try we skip the chunk, then it was already uploaded.
                        self.send_event(FileUploadEvent::AlreadyExistsInNetwork(
                            ChunkAddress::new(chunk_info.name),
                        ));
                        skipped += 1;
                    }
                }
                TaskResult::MakePaymentsOK((
                    (storage_cost, royalty_fees, new_balance),
                    reply_list,
                )) => {
                    trace!("Paid {} chunks, with {storage_cost:?} store_cost and {royalty_fees:?} royalty_fees, and new_balance is {new_balance:?}",
                        reply_list.len());
                    sequential_payment_fails = 0;
                    for (chunk_info, _) in reply_list.iter() {
                        let _ = on_going_pay_for_chunk.remove(&chunk_info.name);
                    }
                    pending_to_upload.extend(reply_list);

                    self.upload_storage_cost = self
                        .upload_storage_cost
                        .checked_add(storage_cost)
                        .ok_or(ClientError::TotalPriceTooHigh)?;
                    self.upload_royalty_fees = self
                        .upload_royalty_fees
                        .checked_add(royalty_fees)
                        .ok_or(ClientError::TotalPriceTooHigh)?;
                    self.upload_final_balance = new_balance;
                    self.send_event(FileUploadEvent::PayedForChunks {
                        storage_cost,
                        royalty_fees,
                        new_balance,
                    });
                }
                TaskResult::UploadChunksOK(xor_name) => {
                    trace!("Upload task uploaded chunk {xor_name:?}");
                    let _ = on_going_uploadings.remove(&xor_name);
                    uploaded += 1;
                    self.send_event(FileUploadEvent::Uploaded(ChunkAddress::new(xor_name)));
                }
                TaskResult::ErrorEntries(error_list) => {
                    if error_list.is_empty() {
                        // Empty error_list indicating an unrecoverable failure to access wallet.
                        // The entire upload process shall be terminated.
                        return Err(ClientError::FailedToAccessWallet);
                    }
                    if error_list.len() > 1 {
                        sequential_payment_fails += 1;
                        if sequential_payment_fails >= MAX_SEQUENTIAL_PAYMENT_FAILS {
                            // Too many sequential overall payment failure indicating
                            // unrecoverable failure of spend tx continously rejected by network.
                            // The entire upload process shall be terminated.
                            return Err(ClientError::SequentialUploadPaymentError);
                        }
                    }

                    // In case of error, remove the entries from correspondent on_going_tasks list.
                    // So that the overall upload flow can progress on to other work.
                    for chunk_info in error_list.iter() {
                        let _ = on_going_get_cost.remove(&chunk_info.name);
                        let _ = on_going_pay_for_chunk.remove(&chunk_info.name);
                        let _ = on_going_uploadings.remove(&chunk_info.name);
                    }

                    self.failed_chunks.extend(error_list);
                }
            }
        }
    }

    /// Store chunks from chunk_paths (assuming payments have already been made and are in our local wallet).
    /// If verify_store is true, we will attempt to fetch the chunks from the network to verify it is stored.
    async fn upload_chunk(
        files_api: FilesApi,
        chunk_info: ChunkInfo,
        payee: PeerId,
        verify_store: bool,
        retry_strategy: RetryStrategy,
    ) -> (ChunkInfo, Result<()>) {
        let chunk_address = ChunkAddress::new(chunk_info.name);
        let bytes = match std::fs::read(chunk_info.path.clone()) {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                warn!("Chunk {chunk_address:?} could not be read from the system from {:?}. 
            Normally this happens if it has been uploaded, but the cleanup process was interrupted. Ignoring error: {error}", chunk_info.path);

                return (chunk_info, Ok(()));
            }
        };
        let chunk = Chunk::new(bytes);
        match files_api
            .get_local_payment_and_upload_chunk(chunk, payee, verify_store, Some(retry_strategy))
            .await
        {
            Ok(()) => (chunk_info, Ok(())),
            Err(err) => (chunk_info, Err(err)),
        }
    }

    fn take_failed_chunks(&mut self) -> Vec<ChunkInfo> {
        std::mem::take(&mut self.failed_chunks)
            .into_iter()
            .collect()
    }

    fn send_event(&mut self, event: FileUploadEvent) {
        if let Some(sender) = self.event_sender.as_ref() {
            let sender_clone = sender.clone();
            let _handle = tokio::spawn(async move {
                let _ = sender_clone.send(event).await;
            });
        } else if !self.logged_event_sender_absence {
            info!("FilesUpload upload event sender is not set. Use get_upload_events() if you need to keep track of the progress");
            self.logged_event_sender_absence = true;
        }
    }

    fn spawn_get_store_cost_task(
        &self,
        chunk_info: ChunkInfo,
        get_store_cost_sender: mpsc::Sender<TaskResult>,
    ) {
        trace!("spawning a get_store_cost task");
        let client = self.api.client.clone();
        let _handle = tokio::spawn(async move {
            let cost = match client
                .network
                .get_store_costs_from_network(NetworkAddress::from_chunk_address(
                    ChunkAddress::new(chunk_info.name),
                ))
                .await
            {
                Ok(cost) => {
                    let t = &cost.quote;
                    debug!("Storecosts retrieved for {:?} {t:?}", chunk_info.name);
                    TaskResult::GetStoreCostOK((chunk_info, cost))
                }
                Err(err) => {
                    error!(
                        "Encountered error {err:?} when getting store_cost of {:?}",
                        chunk_info.name
                    );
                    TaskResult::ErrorEntries(vec![chunk_info])
                }
            };

            let _ = get_store_cost_sender.send(cost).await;
        });
    }

    fn spawn_paying_thread(
        &self,
        mut paying_work_receiver: mpsc::Receiver<Option<(ChunkInfo, PayeeQuote)>>,
        pay_for_chunk_sender: mpsc::Sender<TaskResult>,
        batch_size: usize,
    ) {
        let files_api = self.api.clone();
        let verify_store = self.verify_store;
        let _handle = tokio::spawn(async move {
            trace!("spawning paying thread");
            let mut wallet_client = match files_api.wallet() {
                Ok(wallet) => wallet,
                Err(err) => {
                    error!("Failed to open wallet when handling {err:?}");
                    let _ = pay_for_chunk_sender
                        .send(TaskResult::ErrorEntries(vec![]))
                        .await;
                    return;
                }
            };
            let mut cost_map = BTreeMap::new();
            let mut chunk_info_map = vec![];

            while let Some(payment) = paying_work_receiver.recv().await {
                let make_payments = if let Some((chunk_info, quote)) = payment {
                    let _ = cost_map.insert(chunk_info.name, (quote.key, quote.quote));
                    chunk_info_map.push((chunk_info, quote.peer));
                    cost_map.len() >= batch_size
                } else {
                    // using None to indicate as all paid.
                    !cost_map.is_empty()
                };

                if make_payments {
                    let result = match wallet_client.pay_for_records(&cost_map, verify_store).await
                    {
                        Ok((storage_cost, royalty_fees)) => {
                            trace!("Made payments for {} chunks", cost_map.len());
                            let reply_list = std::mem::take(&mut chunk_info_map);
                            TaskResult::MakePaymentsOK((
                                (storage_cost, royalty_fees, wallet_client.balance()),
                                reply_list,
                            ))
                        }
                        Err(err) => {
                            let reply_list: Vec<ChunkInfo> = std::mem::take(&mut chunk_info_map)
                                .into_iter()
                                .map(|(chunk_info, _)| chunk_info)
                                .collect();
                            error!("When paying {} chunks, got error {err:?}", reply_list.len());
                            TaskResult::ErrorEntries(reply_list)
                        }
                    };
                    let pay_for_chunk_sender_clone = pay_for_chunk_sender.clone();
                    let _handle = tokio::spawn(async move {
                        let _ = pay_for_chunk_sender_clone.send(result).await;
                    });

                    cost_map = BTreeMap::new();
                }
            }
            trace!("Paying thread terminated");
        });
    }

    fn spawn_upload_chunk_task(
        &self,
        chunk_info: ChunkInfo,
        payee: PeerId,
        upload_chunk_sender: mpsc::Sender<TaskResult>,
    ) {
        trace!("spawning upload chunk task");
        let files_api = self.api.clone();
        let verify_store = self.verify_store;
        let retry_strategy = self.retry_strategy;

        let _handle = tokio::spawn(async move {
            let (chunk_info, result) =
                Self::upload_chunk(files_api, chunk_info, payee, verify_store, retry_strategy)
                    .await;

            debug!(
                "Chunk {:?} uploaded with result {:?}",
                chunk_info.name, result
            );
            if result.is_ok() {
                let _ = upload_chunk_sender
                    .send(TaskResult::UploadChunksOK(chunk_info.name))
                    .await;
            } else {
                let _ = upload_chunk_sender
                    .send(TaskResult::ErrorEntries(vec![chunk_info]))
                    .await;
            }
        });
    }
}

async fn progress_tasks(
    get_store_cost_receiver: &mut mpsc::Receiver<TaskResult>,
    paid_chunk_receiver: &mut mpsc::Receiver<TaskResult>,
    upload_chunk_receiver: &mut mpsc::Receiver<TaskResult>,
) -> Option<TaskResult> {
    tokio::select! {
        get_store_cost_event = get_store_cost_receiver.recv() => {
            get_store_cost_event
        }
        paid_chunk_event = paid_chunk_receiver.recv() => {
            paid_chunk_event
        }
        upload_chunk_event = upload_chunk_receiver.recv() => {
            upload_chunk_event
        }
    }
}
