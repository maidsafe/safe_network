// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::get_progress_bar;
use crate::ChunkManager;
use bytes::Bytes;
use color_eyre::{eyre::eyre, Report, Result};
use futures::StreamExt;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use sn_client::{
    transfers::{TransferError, WalletError},
    Client, Error as ClientError, UploadCfg, UploadEvent, UploadSummary, Uploader,
};
use sn_protocol::storage::{Chunk, ChunkAddress};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use tracing::{debug, error, info, warn};
use walkdir::{DirEntry, WalkDir};
use xor_name::XorName;

/// The result of a successful files upload.
pub struct FilesUploadSummary {
    /// The cost and count summary of the upload.
    pub upload_summary: UploadSummary,
    /// The list of completed files (FilePath, FileName, HeadChunkAddress)
    pub completed_files: Vec<(PathBuf, OsString, ChunkAddress)>,
    /// The list of incomplete files (FilePath, FileName, HeadChunkAddress)
    pub incomplete_files: Vec<(PathBuf, OsString, ChunkAddress)>,
}

/// A trait designed to customize the standard output behavior for file upload processes.
pub trait FilesUploadStatusNotifier: Send {
    fn collect_entries(&mut self, entries_iter: Vec<DirEntry>);
    fn collect_paths(&mut self, path: &Path);
    fn on_verifying_uploaded_chunks_init(&self, chunks_len: usize);
    fn on_verifying_uploaded_chunks_success(
        &self,
        completed_files: &[(PathBuf, OsString, ChunkAddress)],
        make_data_public: bool,
    );
    fn on_verifying_uploaded_chunks_failure(&self, failed_chunks_len: usize);
    fn on_failed_to_upload_all_files(
        &self,
        incomplete_files: Vec<(&PathBuf, &OsString, &ChunkAddress)>,
        completed_files: &[(PathBuf, OsString, ChunkAddress)],
        make_data_public: bool,
    );
    fn on_chunking_complete(
        &self,
        upload_cfg: &UploadCfg,
        make_data_public: bool,
        chunks_to_upload_len: usize,
    );
    fn on_upload_complete(
        &self,
        upload_sum: &UploadSummary,
        elapsed_time: Duration,
        chunks_to_upload_len: usize,
    );
}

/// Combines the `Uploader` along with the `ChunkManager`
pub struct FilesUploader {
    client: Client,
    root_dir: PathBuf,
    /// entries to upload
    entries_to_upload: Vec<DirEntry>,
    /// The status notifier that can be overridden to perform custom actions instead of printing things to stdout.
    status_notifier: Option<Box<dyn FilesUploadStatusNotifier>>,
    /// config
    make_data_public: bool,
    upload_cfg: UploadCfg,
}

impl FilesUploader {
    pub fn new(client: Client, root_dir: PathBuf) -> Self {
        let status_notifier = Box::new(StdOutPrinter {
            file_paths_to_print: Default::default(),
        });
        Self {
            client,
            root_dir,
            entries_to_upload: Default::default(),
            status_notifier: Some(status_notifier),
            make_data_public: false,
            upload_cfg: Default::default(),
        }
    }

    pub fn set_upload_cfg(mut self, cfg: UploadCfg) -> Self {
        self.upload_cfg = cfg;
        self
    }

    pub fn set_make_data_public(mut self, make_data_public: bool) -> Self {
        self.make_data_public = make_data_public;
        self
    }

    /// Override the default status notifier. By default we print things to stdout.
    pub fn set_status_notifier(
        mut self,
        status_notifier: Box<dyn FilesUploadStatusNotifier>,
    ) -> Self {
        self.status_notifier = Some(status_notifier);
        self
    }

    pub fn insert_entries(mut self, entries_iter: impl IntoIterator<Item = DirEntry>) -> Self {
        self.entries_to_upload.extend(entries_iter);
        self
    }

    pub fn insert_path(mut self, path: &Path) -> Self {
        if let Some(notifier) = &mut self.status_notifier {
            notifier.collect_paths(path);
        }
        let entries = WalkDir::new(path).into_iter().flatten();
        self.entries_to_upload.extend(entries);
        self
    }

    pub async fn start_upload(mut self) -> Result<FilesUploadSummary> {
        let mut chunk_manager = ChunkManager::new(&self.root_dir);
        let chunks_to_upload = self.get_chunks_to_upload(&mut chunk_manager).await?;
        let chunks_to_upload_len = chunks_to_upload.len();

        // Notify on chunking complete
        if let Some(notifier) = &self.status_notifier {
            notifier.on_chunking_complete(
                &self.upload_cfg,
                self.make_data_public,
                chunks_to_upload_len,
            );
        }

        let now = Instant::now();
        let mut uploader = Uploader::new(self.client, self.root_dir);
        uploader.set_upload_cfg(self.upload_cfg);
        uploader.insert_chunk_paths(chunks_to_upload);

        let events_handle = Self::spawn_upload_events_handler(
            chunk_manager,
            self.make_data_public,
            chunks_to_upload_len,
            uploader.get_event_receiver(),
            self.status_notifier.take(),
        )?;

        let upload_sum = match uploader.start_upload().await {
            Ok(summary) => summary,
            Err(ClientError::Wallet(WalletError::Transfer(TransferError::NotEnoughBalance(
                available,
                required,
            )))) => {
                return Err(eyre!(
                    "Not enough balance in wallet to pay for chunk. \
            We have {available:?} but need {required:?} to pay for the chunk"
                ))
            }
            Err(err) => return Err(eyre!("Failed to upload chunk batch: {err}")),
        };
        let (chunk_manager, status_notifier) = events_handle.await??;
        self.status_notifier = status_notifier;

        // Notify on upload complete
        if let Some(notifier) = &self.status_notifier {
            notifier.on_upload_complete(&upload_sum, now.elapsed(), chunks_to_upload_len);
        }

        let summary = FilesUploadSummary {
            upload_summary: upload_sum,
            completed_files: chunk_manager.completed_files().clone(),
            incomplete_files: chunk_manager
                .incomplete_files()
                .into_iter()
                .map(|(path, file_name, head_address)| {
                    (path.clone(), file_name.clone(), *head_address)
                })
                .collect(),
        };
        Ok(summary)
    }

    // This will read from the cache if possible. We only re-verify with the network if the file has been cached but
    // there are no pending chunks to upload.
    async fn get_chunks_to_upload(
        &self,
        chunk_manager: &mut ChunkManager,
    ) -> Result<Vec<(XorName, PathBuf)>> {
        // Initially try reading from the cache
        chunk_manager.chunk_with_iter(
            self.entries_to_upload.iter().cloned(),
            true,
            self.make_data_public,
        )?;
        // We verify if there are no chunks left to upload.
        let mut chunks_to_upload = if !chunk_manager.is_chunks_empty() {
            chunk_manager.get_chunks()
        } else {
            // re chunk it again to get back all the chunks
            let chunks = chunk_manager.already_put_chunks(
                self.entries_to_upload.iter().cloned(),
                self.make_data_public,
            )?;

            // Notify on verification init
            if let Some(notifier) = &self.status_notifier {
                notifier.on_verifying_uploaded_chunks_init(chunks.len());
            }

            let failed_chunks = self.verify_uploaded_chunks(&chunks).await?;

            chunk_manager.mark_completed(
                chunks
                    .into_iter()
                    .filter(|c| !failed_chunks.contains(c))
                    .map(|(xor, _)| xor),
            )?;

            if failed_chunks.is_empty() {
                // Notify on verification success
                if let Some(notifier) = &self.status_notifier {
                    notifier.on_verifying_uploaded_chunks_success(
                        chunk_manager.completed_files(),
                        self.make_data_public,
                    );
                }

                return Ok(vec![]);
            }
            // Notify on verification failure
            if let Some(notifier) = &self.status_notifier {
                notifier.on_verifying_uploaded_chunks_failure(failed_chunks.len());
            }
            failed_chunks
        };
        // shuffle the chunks
        let mut rng = thread_rng();
        chunks_to_upload.shuffle(&mut rng);

        Ok(chunks_to_upload)
    }

    async fn verify_uploaded_chunks(
        &self,
        chunks_paths: &[(XorName, PathBuf)],
    ) -> Result<Vec<(XorName, PathBuf)>> {
        let mut stream = futures::stream::iter(chunks_paths)
            .map(|(xorname, path)| async move {
                let chunk = Chunk::new(Bytes::from(std::fs::read(path)?));
                let res = self.client.verify_chunk_stored(&chunk).await;
                Ok::<_, Report>((xorname, path.clone(), res.is_err()))
            })
            .buffer_unordered(self.upload_cfg.batch_size);
        let mut failed_chunks = Vec::new();

        while let Some(result) = stream.next().await {
            let (xorname, path, is_error) = result?;
            if is_error {
                warn!("Failed to fetch a chunk {xorname:?}");
                failed_chunks.push((*xorname, path));
            }
        }

        Ok(failed_chunks)
    }

    #[allow(clippy::type_complexity)]
    fn spawn_upload_events_handler(
        mut chunk_manager: ChunkManager,
        make_data_public: bool,
        chunks_to_upload_len: usize,
        mut upload_event_rx: Receiver<UploadEvent>,
        status_notifier: Option<Box<dyn FilesUploadStatusNotifier>>,
    ) -> Result<JoinHandle<Result<(ChunkManager, Option<Box<dyn FilesUploadStatusNotifier>>)>>>
    {
        let progress_bar = get_progress_bar(chunks_to_upload_len as u64)?;
        let handle = tokio::spawn(async move {
            let mut upload_terminated_with_error = false;
            // The loop is guaranteed to end, as the channel will be
            // closed when the upload completes or errors out.
            while let Some(event) = upload_event_rx.recv().await {
                match event {
                    UploadEvent::ChunkUploaded(addr)
                    | UploadEvent::ChunkAlreadyExistsInNetwork(addr) => {
                        progress_bar.clone().inc(1);
                        if let Err(err) =
                            chunk_manager.mark_completed(std::iter::once(*addr.xorname()))
                        {
                            error!("Failed to mark chunk {addr:?} as completed: {err:?}");
                        }
                    }
                    UploadEvent::Error => {
                        upload_terminated_with_error = true;
                    }
                    UploadEvent::RegisterUploaded { .. }
                    | UploadEvent::RegisterUpdated { .. }
                    | UploadEvent::PaymentMade { .. } => {}
                }
            }
            progress_bar.finish_and_clear();

            // this check is to make sure that we don't partially write to the uploaded_files file if the upload process
            // terminates with an error. This race condition can happen as we bail on `upload_result` before we await the
            // handler.
            if upload_terminated_with_error {
                error!("Got UploadEvent::Error inside upload event loop");
            } else {
                // Notify on upload failure
                if let Some(notifier) = &status_notifier {
                    notifier.on_failed_to_upload_all_files(
                        chunk_manager.incomplete_files(),
                        chunk_manager.completed_files(),
                        make_data_public,
                    );
                }
            }

            Ok::<_, Report>((chunk_manager, status_notifier))
        });

        Ok(handle)
    }
}

/// The default
struct StdOutPrinter {
    file_paths_to_print: Vec<PathBuf>,
}

impl FilesUploadStatusNotifier for StdOutPrinter {
    fn collect_entries(&mut self, _entries_iter: Vec<DirEntry>) {}

    fn collect_paths(&mut self, path: &Path) {
        self.file_paths_to_print.push(path.to_path_buf());
    }

    fn on_verifying_uploaded_chunks_init(&self, chunks_len: usize) {
        println!("Files upload attempted previously, verifying {chunks_len} chunks",);
    }

    fn on_verifying_uploaded_chunks_success(
        &self,
        completed_files: &[(PathBuf, OsString, ChunkAddress)],
        make_data_public: bool,
    ) {
        println!("All files were already uploaded and verified");
        Self::print_uploaded_msg(make_data_public);

        if completed_files.is_empty() {
            println!("chunk_manager doesn't have any verified_files, nor any failed_chunks to re-upload.");
        }
        Self::print_completed_file_list(completed_files);
    }

    fn on_verifying_uploaded_chunks_failure(&self, failed_chunks_len: usize) {
        println!("{failed_chunks_len} chunks were uploaded in the past but failed to verify. Will attempt to upload them again...");
    }

    fn on_failed_to_upload_all_files(
        &self,
        incomplete_files: Vec<(&PathBuf, &OsString, &ChunkAddress)>,
        completed_files: &[(PathBuf, OsString, ChunkAddress)],
        make_data_public: bool,
    ) {
        for (_, file_name, _) in incomplete_files {
            if let Some(file_name) = file_name.to_str() {
                println!("Unverified file \"{file_name}\", suggest to re-upload again.");
                info!("Unverified {file_name}");
            } else {
                println!("Unverified file \"{file_name:?}\", suggest to re-upload again.");
                info!("Unverified file {file_name:?}");
            }
        }

        // log uploaded file information
        Self::print_uploaded_msg(make_data_public);
        Self::print_completed_file_list(completed_files);
    }

    fn on_chunking_complete(
        &self,
        upload_cfg: &UploadCfg,
        make_data_public: bool,
        chunks_to_upload_len: usize,
    ) {
        for path in self.file_paths_to_print.iter() {
            debug!(
                "Uploading file(s) from {path:?} batch size {:?} will verify?: {}",
                upload_cfg.batch_size, upload_cfg.verify_store
            );
            if make_data_public {
                info!("{path:?} will be made public and linkable");
                println!("{path:?} will be made public and linkable");
            }
        }
        if self.file_paths_to_print.len() == 1 {
            println!(
                "Splitting and uploading {:?} into {chunks_to_upload_len} chunks",
                self.file_paths_to_print[0]
            );
        } else {
            println!(
                "Splitting and uploading {:?} into {chunks_to_upload_len} chunks",
                self.file_paths_to_print
            );
        }
    }

    fn on_upload_complete(
        &self,
        upload_sum: &UploadSummary,
        elapsed_time: Duration,
        chunks_to_upload_len: usize,
    ) {
        let elapsed_minutes = elapsed_time.as_secs() / 60;
        let elapsed_seconds = elapsed_time.as_secs() % 60;
        let elapsed = if elapsed_minutes > 0 {
            format!("{elapsed_minutes} minutes {elapsed_seconds} seconds")
        } else {
            format!("{elapsed_seconds} seconds")
        };

        println!(
            "Among {chunks_to_upload_len} chunks, found {} already existed in network, uploaded \
            the leftover {} chunks in {elapsed}",
            upload_sum.skipped_count, upload_sum.uploaded_count,
        );
        info!(
            "Among {chunks_to_upload_len} chunks, found {} already existed in network, uploaded \
            the leftover {} chunks in {elapsed}",
            upload_sum.skipped_count, upload_sum.uploaded_count,
        );
        println!("**************************************");
        println!("*          Payment Details           *");
        println!("**************************************");
        println!(
            "Made payment of {:?} for {} chunks",
            upload_sum.storage_cost, upload_sum.uploaded_count
        );
        println!(
            "Made payment of {:?} for royalties fees",
            upload_sum.royalty_fees
        );
        println!("New wallet balance: {}", upload_sum.final_balance);
    }
}

impl StdOutPrinter {
    fn print_completed_file_list(completed_files: &[(PathBuf, OsString, ChunkAddress)]) {
        for (_, file_name, addr) in completed_files {
            let hex_addr = addr.to_hex();
            if let Some(file_name) = file_name.to_str() {
                println!("\"{file_name}\" {hex_addr}");
                info!("Uploaded {file_name} to {hex_addr}");
            } else {
                println!("\"{file_name:?}\" {hex_addr}");
                info!("Uploaded {file_name:?} to {hex_addr}");
            }
        }
    }

    fn print_uploaded_msg(make_data_public: bool) {
        println!("**************************************");
        println!("*          Uploaded Files            *");
        if !make_data_public {
            println!("*                                    *");
            println!("*  These are not public by default.  *");
            println!("*     Reupload with `-p` option      *");
            println!("*      to publish the datamaps.      *");
        }
        println!("**************************************");
    }
}
