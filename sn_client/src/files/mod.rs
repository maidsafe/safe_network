// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(crate) mod api;

use crate::{
    error::{Error as ClientError, Result},
    FilesApi,
};
use bytes::Bytes;
use futures::{stream::FuturesUnordered, StreamExt};
use sn_protocol::storage::{Chunk, ChunkAddress};
use sn_transfers::NanoTokens;
use std::{collections::HashSet, path::PathBuf};
use tokio::{
    sync::mpsc::{self},
    task::JoinHandle,
};
use xor_name::XorName;

/// `BATCH_SIZE` determines the number of chunks that are processed in parallel during the payment and upload process.
pub const BATCH_SIZE: usize = 16;

/// The maximum number of retries to perform on a failed chunk.
pub const MAX_UPLOAD_RETRIES: usize = 3;

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

/// `Files` provides functionality for uploading and downloading chunks with support for retries and queuing.
/// This struct is not cloneable. To create a new instance with default configuration, use the `new` function.
/// To modify the configuration, use the provided setter methods (`set_...` functions).
pub struct Files {
    // Configurations
    batch_size: usize,
    verify_store: bool,
    show_holders: bool,
    max_retries: usize,
    // API
    api: FilesApi,
    // Uploads
    failed_chunks: HashSet<ChunkInfo>,
    uploading_chunks: FuturesUnordered<JoinHandle<(ChunkInfo, Result<()>)>>,
    // Upload stats
    upload_storage_cost: NanoTokens,
    upload_royalty_fees: NanoTokens,
    upload_final_balance: NanoTokens,
    // Events
    event_sender: Option<mpsc::Sender<FileUploadEvent>>,
    logged_event_sender_absence: bool,
}

impl Files {
    /// Creates a new instance of `Files` with the default configuration.
    /// To modify the configuration, use the provided setter methods (`set_...` functions).
    pub fn new(files_api: FilesApi) -> Self {
        Self {
            batch_size: BATCH_SIZE,
            verify_store: true,
            show_holders: false,
            max_retries: MAX_UPLOAD_RETRIES,
            api: files_api,
            failed_chunks: Default::default(),
            uploading_chunks: Default::default(),
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

    /// Sets the maximum number of retries to perform if a chunk fails to upload.
    ///
    /// By default, this option is set to the constant `MAX_UPLOAD_RETRIES: usize = 3`.
    pub fn set_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
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

    /// get the set of failed chunks that could not be uploaded
    pub fn get_failed_chunks(&self) -> HashSet<XorName> {
        self.failed_chunks
            .clone()
            .into_iter()
            .map(|chunk_info| chunk_info.name)
            .collect()
    }

    /// Uploads the provided chunks to the network.
    /// If you want to track the upload progress, use the `get_upload_events` method.
    pub async fn upload_chunks(&mut self, chunks: Vec<(XorName, PathBuf)>) -> Result<()> {
        // make sure we log that the event sender is absent atleast once
        self.logged_event_sender_absence = false;

        // clean up the trackers/stats
        self.failed_chunks = Default::default();
        self.uploading_chunks = Default::default();
        self.upload_storage_cost = NanoTokens::zero();
        self.upload_royalty_fees = NanoTokens::zero();
        self.upload_final_balance = NanoTokens::zero();

        let result = self.upload(chunks).await;

        // send an event indicating that the upload process completed with an error
        if result.is_err() {
            self.send_event(FileUploadEvent::Error).await?;
        }

        // drop the sender to close the channel.
        let sender = self.event_sender.take();
        drop(sender);

        result
    }

    async fn upload(&mut self, chunks: Vec<(XorName, PathBuf)>) -> Result<()> {
        let mut sequential_payment_fails = 0;

        let mut chunk_batches = Vec::with_capacity(chunks.len());
        chunk_batches.extend(
            chunks
                .into_iter()
                .map(|(name, path)| ChunkInfo { name, path }),
        );
        let n_batches = {
            let total_elements = chunk_batches.len();
            // to get +1 if there is a remainder
            (total_elements + self.batch_size - 1) / self.batch_size
        };
        let mut batch = 1;
        let chunk_batches = chunk_batches.chunks(self.batch_size);

        for chunks_batch in chunk_batches {
            trace!("Uploading batch {batch}/{n_batches}");
            if sequential_payment_fails >= MAX_SEQUENTIAL_PAYMENT_FAILS {
                return Err(ClientError::SequentialUploadPaymentError);
            }
            // if the payment fails, we can continue to the next batch
            let res = self.handle_chunk_batch(chunks_batch, false).await;
            batch += 1;
            match res {
                Ok(()) => {
                    trace!("Uploaded batch {batch}/{n_batches}");
                }
                Err(err) => match err {
                    ClientError::CouldNotVerifyTransfer(err) => {
                        warn!(
                            "Failed to verify transfer validity in the network. Chunk batch will be retried... {err:?}"
                        );
                        println!(
                            "Failed to verify transfer validity in the network. Chunk batch will be retried..."
                        );
                        sequential_payment_fails += 1;
                        continue;
                    }
                    error => {
                        return Err(error);
                    }
                },
            }
        }

        // ensure we wait on any remaining uploading_chunks
        self.progress_uploading_chunks(true).await?;

        let mut retry_count = 0;
        let max_retries = self.max_retries;
        let mut failed_chunks_to_upload = self.take_failed_chunks();
        while !failed_chunks_to_upload.is_empty() && retry_count < max_retries {
            warn!(
                "Retrying failed chunks {:?}, attempt {retry_count}/{max_retries}...",
                failed_chunks_to_upload.len()
            );
            println!(
                "Retrying failed chunks {:?}, attempt {retry_count}/{max_retries}...",
                failed_chunks_to_upload.len()
            );
            retry_count += 1;
            let batches = failed_chunks_to_upload.chunks(self.batch_size);
            for chunks_batch in batches {
                self.handle_chunk_batch(chunks_batch, true).await?;
            }
            // ensure we wait on any remaining uploading_chunks w/ drain_all
            self.progress_uploading_chunks(true).await?;

            // take the new failed chunks
            failed_chunks_to_upload = self.take_failed_chunks();
        }

        Ok(())
    }

    /// Handles a batch of chunks for upload. This includes paying for the chunks, uploading them,
    /// and handling any errors that occur during the process.
    ///
    /// If `failed_batch` is true, we emit FilesUploadEvent::Uploaded for the skipped_chunks. This is because,
    /// the failed_batch was already paid for, but could not be verified on the first try.
    async fn handle_chunk_batch(
        &mut self,
        chunks_batch: &[ChunkInfo],
        failed_batch: bool,
    ) -> Result<()> {
        // while we don't have a full batch_size of ongoing uploading_chunks
        // we can pay for the next batch and carry on
        self.progress_uploading_chunks(false).await?;

        // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
        let skipped_chunks = match self
            .api
            .pay_for_chunks(chunks_batch.iter().map(|info| info.name).collect())
            .await
        {
            Ok(((storage_cost, royalty_fees, new_balance), skipped_chunks)) => {
                // store the stats and emit event too
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
                })
                .await?;
                skipped_chunks
            }
            Err(err) => return Err(err),
        };

        let mut chunks_to_upload = chunks_batch.to_vec();
        // don't reupload skipped chunks
        chunks_to_upload.retain(|info| !skipped_chunks.contains(&info.name));

        // send update about the existing chunks
        for chunk in skipped_chunks {
            if failed_batch {
                // the chunk was already paid for but might have not been verified on the first try.
                self.send_event(FileUploadEvent::Uploaded(ChunkAddress::new(chunk)))
                    .await?;
            } else {
                // if during the first try we skip the chunk, then it was already uploaded.
                self.send_event(FileUploadEvent::AlreadyExistsInNetwork(ChunkAddress::new(
                    chunk,
                )))
                .await?;
            }
        }

        // upload paid chunks
        for chunk_info in chunks_to_upload.into_iter() {
            let files_api = self.api.clone();
            let verify_store = self.verify_store;

            // Spawn a task for each chunk to be uploaded
            let handle = tokio::spawn(Self::upload_chunk(files_api, chunk_info, verify_store));
            self.progress_uploading_chunks(false).await?;

            self.uploading_chunks.push(handle);
        }

        Ok(())
    }

    /// Progresses the uploading of chunks. If the number of ongoing uploading chunks is less than the batch size,
    /// it pays for the next batch and continues. If an error occurs during the upload, it will be returned.
    ///
    /// If `drain_all` is true, will wait for all ongoing uploads to complete before returning.
    async fn progress_uploading_chunks(&mut self, drain_all: bool) -> Result<()> {
        while drain_all || self.uploading_chunks.len() >= self.batch_size {
            if let Some(result) = self.uploading_chunks.next().await {
                // bail if we've had any errors so far
                match result? {
                    (chunk_info, Ok(())) => {
                        self.send_event(FileUploadEvent::Uploaded(ChunkAddress::new(
                            chunk_info.name,
                        )))
                        .await?;
                    }
                    (chunk_info, Err(err)) => {
                        warn!("Failed to upload a chunk: {err}");
                        self.send_event(FileUploadEvent::FailedToUpload(ChunkAddress::new(
                            chunk_info.name,
                        )))
                        .await?;
                        // store the failed chunk to be retried later
                        self.failed_chunks.insert(chunk_info);
                    }
                }
            } else {
                // we're finished
                break;
            }
        }
        Ok(())
    }

    /// Store chunks from chunk_paths (assuming payments have already been made and are in our local wallet).
    /// If verify_store is true, we will attempt to fetch the chunks from the network to verify it is stored.
    async fn upload_chunk(
        files_api: FilesApi,
        chunk_info: ChunkInfo,
        verify_store: bool,
    ) -> (ChunkInfo, Result<()>) {
        let chunk_address = ChunkAddress::new(chunk_info.name);
        let bytes = match tokio::fs::read(chunk_info.path.clone()).await {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                warn!("Chunk {chunk_address:?} could not be read from the system from {:?}. 
            Normally this happens if it has been uploaded, but the cleanup process was interrupted. Ignoring error: {error}", chunk_info.path);

                return (chunk_info, Ok(()));
            }
        };
        let chunk = Chunk::new(bytes);
        match files_api
            .get_local_payment_and_upload_chunk(chunk, verify_store)
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

    async fn send_event(&mut self, event: FileUploadEvent) -> Result<()> {
        if let Some(sender) = self.event_sender.as_ref() {
            sender.send(event).await.map_err(|err| {
                error!("Could not send files event due to {err:?}");
                ClientError::CouldNotSendFilesEvent
            })?;
        } else if !self.logged_event_sender_absence {
            info!("Files upload event sender is not set. Use get_upload_events() if you need to keep track of the progress");
            self.logged_event_sender_absence = true;
        }
        Ok(())
    }
}
