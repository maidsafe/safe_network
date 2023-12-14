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

/// The maximum number of sequential payment failures before aborting the upload process.
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 3;

pub enum FileUploadEvent {
    Uploaded(ChunkAddress),
    AlreadyExistsInNetwork(ChunkAddress),
    PayedForChunks {
        storage_cost: NanoTokens,
        royalty_fees: NanoTokens,
        new_balance: NanoTokens,
    },
    FailedToUpload(ChunkAddress),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ChunkInfo {
    name: XorName,
    path: PathBuf,
}

pub struct Files {
    batch_size: usize,
    verify_store: bool,
    show_holders: bool,
    max_retries: usize,

    api: FilesApi,

    failed_chunks: HashSet<ChunkInfo>,
    uploading_chunks: FuturesUnordered<JoinHandle<(ChunkInfo, Result<()>)>>,

    event_sender: Option<mpsc::Sender<FileUploadEvent>>,
    logged_event_sender_absence: bool,
}

impl Files {
    pub fn new(
        files_api: FilesApi,
        batch_size: usize,
        verify_store: bool,
        show_holders: bool,
        max_retries: usize,
    ) -> Self {
        Self {
            batch_size,
            verify_store,
            show_holders,
            max_retries,
            api: files_api,
            failed_chunks: Default::default(),
            uploading_chunks: Default::default(),
            event_sender: None,
            logged_event_sender_absence: false,
        }
    }

    // new rx per upload session.
    pub fn get_upload_events(&mut self) -> mpsc::Receiver<FileUploadEvent> {
        let (event_sender, event_receiver) = mpsc::channel(10);
        // should we return error if an sender is already set?
        self.event_sender = Some(event_sender);

        event_receiver
    }
    pub async fn upload_chunks(&mut self, chunks: Vec<(XorName, PathBuf)>) -> Result<()> {
        // make sure we log that the event sender is absent atleast once
        self.logged_event_sender_absence = false;
        // new fn to upload chunks

        let result = self.upload(chunks).await;

        // drop the sender
        let sender = self.event_sender.take();
        drop(sender);

        result
    }

    pub fn failed_chunks(&self) -> HashSet<XorName> {
        self.failed_chunks
            .clone()
            .into_iter()
            .map(|chunk_info| chunk_info.name)
            .collect()
    }

    async fn upload(&mut self, chunks: Vec<(XorName, PathBuf)>) -> Result<()> {
        let mut sequential_payment_fails = 0;

        let mut chunk_batches = Vec::with_capacity(chunks.len());
        chunk_batches.extend(
            chunks
                .into_iter()
                .map(|(name, path)| ChunkInfo { name, path }),
        );
        let chunk_batches = chunk_batches.chunks(self.batch_size);

        for chunks_batch in chunk_batches {
            if sequential_payment_fails >= MAX_SEQUENTIAL_PAYMENT_FAILS {
                return Err(ClientError::SequentialUploadPaymentError);
            }
            // if the payment fails, we can continue to the next batch
            let res = self.handle_chunk_batch(chunks_batch).await;
            match res {
                Ok(()) => {
                    trace!("Uploaded a batch");
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
                self.handle_chunk_batch(chunks_batch).await?;
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
    async fn handle_chunk_batch(&mut self, chunks_batch: &[ChunkInfo]) -> Result<()> {
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
            self.send_event(FileUploadEvent::AlreadyExistsInNetwork(ChunkAddress::new(
                chunk,
            )))
            .await?;
        }

        // upload paid chunks
        for chunk_info in chunks_to_upload.into_iter() {
            let file_api = self.api.clone();
            let verify_store = self.verify_store;
            let show_holders = self.show_holders;

            // Spawn a task for each chunk to be uploaded
            let handle = tokio::spawn(Self::upload_chunk(
                file_api,
                chunk_info,
                verify_store,
                show_holders,
            ));
            self.progress_uploading_chunks(false).await?;

            self.uploading_chunks.push(handle);
        }

        Ok(())
    }

    /// Progresses the uploading of chunks. If the number of ongoing uploading chunks is less than the batch size,
    /// it pays for the next batch and continues. If an error occurs during the upload, it will be returned.
    ///
    /// # Arguments
    ///
    /// * `params` - The parameters for the upload, including the chunk manager and the batch size.
    /// * `drain_all` - If true, will wait for all ongoing uploads to complete before returning.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - The result of the upload. If successful, it will return `Ok(())`. If an error occurs, it will return `Err(report)`.
    async fn progress_uploading_chunks(&mut self, drain_all: bool) -> Result<()> {
        while drain_all || self.uploading_chunks.len() >= self.batch_size {
            if let Some(result) = self.uploading_chunks.next().await {
                // bail if we've had any errors so far
                match result? {
                    // or cleanup via chunk_manager
                    (chunk_info, Ok(())) => {
                        // mark the chunk as completed
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
        file_api: FilesApi,
        chunk_info: ChunkInfo,
        verify_store: bool,
        show_holders: bool,
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
        match file_api
            .get_local_payment_and_upload_chunk(chunk, verify_store, show_holders)
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
