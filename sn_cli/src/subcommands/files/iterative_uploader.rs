use crate::subcommands::files;
use crate::subcommands::files::{ChunkManager, FilesUploadOptions};
use color_eyre::{eyre::eyre, Result};
use indicatif::ProgressBar;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use sn_client::transfers::{NanoTokens, TransferError, WalletError};
use sn_client::{Error as ClientError, Error, FileUploadEvent, FilesApi, FilesUpload};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use walkdir::DirEntry;
use xor_name::XorName;

pub(crate) struct IterativeUploader {
    chunk_manager: ChunkManager,
    files_api: FilesApi,
}

impl IterativeUploader {
    pub(crate) fn new(chunk_manager: ChunkManager, files_api: FilesApi) -> Self {
        Self {
            chunk_manager,
            files_api,
        }
    }
}

impl IterativeUploader {
    /// Given an iterator over files, upload them. Optionally verify if the data was stored successfully.
    pub(crate) async fn iterate_upload(
        mut self,
        entries_iter: impl Iterator<Item = DirEntry>,
        files_path: PathBuf,
        options: FilesUploadOptions,
    ) -> Result<()> {
        let FilesUploadOptions {
            make_data_public,
            verify_store,
            batch_size,
            retry_strategy,
        } = options;

        let mut rng = thread_rng();

        msg_init(&files_path, &batch_size, &verify_store, make_data_public);

        self.chunk_manager
            .chunk_with_iter(entries_iter, true, make_data_public)?;

        // Return early if we already uploaded them
        let mut chunks_to_upload = if self.chunk_manager.is_chunks_empty() {
            // make sure we don't have any failed chunks in those

            let chunks = self
                .chunk_manager
                .already_put_chunks(&files_path, make_data_public)?;
            println!(
                "Files upload attempted previously, verifying {} chunks",
                chunks.len()
            );

            let failed_chunks = self
                .files_api
                .client()
                .verify_uploaded_chunks(&chunks, batch_size)
                .await?;

            // mark the non-failed ones as completed
            self.chunk_manager.mark_completed(
                chunks
                    .into_iter()
                    .filter(|c| !failed_chunks.contains(c))
                    .map(|(xor, _)| xor),
            )?;

            // if none are failed, we can return early
            if failed_chunks.is_empty() {
                msg_files_already_uploaded_verified();
                if !make_data_public {
                    msg_not_public_by_default();
                }
                msg_star_line();
                if self.chunk_manager.completed_files().is_empty() {
                    msg_chk_mgr_no_verified_file_nor_re_upload();
                }
                msg_chunk_manager_upload_complete(self.chunk_manager);
                return Ok(());
            }
            msg_unverified_chunks_reattempted(&failed_chunks.len());
            failed_chunks
        } else {
            self.chunk_manager.get_chunks()
        };

        // Random shuffle the chunks_to_upload, so that uploading of a large file can be speed up by
        // having multiple client instances uploading the same target.
        chunks_to_upload.shuffle(&mut rng);

        let chunk_amount_to_upload = chunks_to_upload.len();
        let progress_bar = files::get_progress_bar(chunks_to_upload.len() as u64)?;
        let total_existing_chunks = Arc::new(AtomicU64::new(0));
        let mut files_upload = FilesUpload::new(self.files_api)
            .set_batch_size(batch_size)
            .set_verify_store(verify_store)
            .set_retry_strategy(retry_strategy);

        let upload_event_rx = files_upload.get_upload_events();
        // keep track of the progress in a separate task
        let progress_bar_clone = progress_bar.clone();
        let total_existing_chunks_clone = total_existing_chunks.clone();

        let process_join_handle = spawn_progress_handler(
            self.chunk_manager,
            make_data_public,
            progress_bar,
            upload_event_rx,
            progress_bar_clone,
            total_existing_chunks_clone,
        );

        msg_uploading_chunks(chunk_amount_to_upload);

        let current_instant = Instant::now();

        IterativeUploader::upload_result(chunks_to_upload, &mut files_upload).await?;

        process_join_handle
            .await?
            .map_err(|err| eyre!("Failed to write uploaded files with err: {err:?}"))?;

        msg_final(
            chunk_amount_to_upload,
            current_instant,
            total_existing_chunks,
            files_upload,
        );

        Ok(())
    }

    async fn upload_result(
        chunks_to_upload: Vec<(XorName, PathBuf)>,
        files_upload: &mut FilesUpload,
    ) -> Result<()> {
        match files_upload.upload_chunks(chunks_to_upload).await {
            Ok(()) => Ok(()),
            Err(ClientError::Transfers(WalletError::Transfer(
                TransferError::NotEnoughBalance(available, required),
            ))) => Err(eyre!(
                "Not enough balance in wallet to pay for chunk. \
            We have {available:?} but need {required:?} to pay for the chunk"
            )),
            Err(err) => Err(eyre!("Failed to upload chunk batch: {err}")),
        }
    }
}

///////////////// Associative Functions /////////////////

fn spawn_progress_handler(
    mut chunk_manager: ChunkManager,
    make_data_public: bool,
    progress_bar: ProgressBar,
    mut upload_event_rx: Receiver<FileUploadEvent>,
    progress_bar_clone: ProgressBar,
    total_existing_chunks_clone: Arc<AtomicU64>,
) -> JoinHandle<Result<(), Error>> {
    tokio::spawn(async move {
        let mut upload_terminated_with_error = false;
        // The loop is guaranteed to end, as the channel will be closed when the upload completes or errors out.
        while let Some(event) = upload_event_rx.recv().await {
            match event {
                FileUploadEvent::Uploaded(addr) => {
                    progress_bar_clone.inc(1);
                    if let Err(err) = chunk_manager.mark_completed(std::iter::once(*addr.xorname()))
                    {
                        error!("Failed to mark chunk {addr:?} as completed: {err:?}");
                    }
                }
                FileUploadEvent::AlreadyExistsInNetwork(addr) => {
                    let _ = total_existing_chunks_clone.fetch_add(1, Ordering::Relaxed);
                    progress_bar_clone.inc(1);
                    if let Err(err) = chunk_manager.mark_completed(std::iter::once(*addr.xorname()))
                    {
                        error!("Failed to mark chunk {addr:?} as completed: {err:?}");
                    }
                }
                FileUploadEvent::PayedForChunks { .. } => {}
                // Do not increment the progress bar of a chunk upload failure as the event can be emitted multiple
                // times for a single chunk if retries are enabled.
                FileUploadEvent::FailedToUpload(_) => {}
                FileUploadEvent::Error => {
                    upload_terminated_with_error = true;
                }
            }
        }
        progress_bar.finish_and_clear();

        // this check is to make sure that we don't partially write to the uploaded_files file if the upload process
        // terminates with an error. This race condition can happen as we bail on `upload_result` before we await the
        // handler.
        if upload_terminated_with_error {
            error!("Got FileUploadEvent::Error inside upload event loop");
        } else {
            msg_check_incomplete_files(&mut chunk_manager);

            // log uploaded file information
            msg_uploaded_files_banner();
            if !make_data_public {
                msg_not_public_by_default_banner();
            }
            msg_star_line();
            msg_chunk_manager_upload_complete(chunk_manager);
        }

        Ok::<_, ClientError>(())
    })
}

/////////////////  Messages  /////////////////

/// Function to format elapsed time into a string
fn msg_format_elapsed_time(elapsed_time: std::time::Duration) -> String {
    let elapsed_minutes = elapsed_time.as_secs() / 60;
    let elapsed_seconds = elapsed_time.as_secs() % 60;
    if elapsed_minutes > 0 {
        format!("{elapsed_minutes} minutes {elapsed_seconds} seconds")
    } else {
        format!("{elapsed_seconds} seconds")
    }
}

fn msg_check_incomplete_files(chunk_manager: &mut ChunkManager) {
    for file_name in chunk_manager.incomplete_files() {
        if let Some(file_name) = file_name.to_str() {
            println!("Unverified file \"{file_name}\", suggest to re-upload again.");
            info!("Unverified {file_name}");
        } else {
            println!("Unverified file \"{file_name:?}\", suggest to re-upload again.");
            info!("Unverified file {file_name:?}");
        }
    }
}

fn msg_init(files_path: &PathBuf, batch_size: &usize, verify_store: &bool, make_data_public: bool) {
    debug!("Uploading file(s) from {files_path:?}, batch size {batch_size:?} will verify?: {verify_store}");
    if make_data_public {
        info!("{files_path:?} will be made public and linkable");
        println!("{files_path:?} will be made public and linkable");
    }
    println!("Starting to chunk {files_path:?} now."); // check message responsibility
}

fn msg_final(
    chunks_to_upload_amount: usize,
    time_since_mark: Instant,
    total_existing_chunks: Arc<AtomicU64>,
    files_upload: FilesUpload,
) {
    let total_existing_chunks = total_existing_chunks.load(Ordering::Relaxed);
    let uploaded_chunks = chunks_to_upload_amount - total_existing_chunks as usize;
    let time_since_mark_formatted = msg_format_elapsed_time(time_since_mark.elapsed());

    msg_chunks_found_existed(
        chunks_to_upload_amount,
        &time_since_mark_formatted,
        total_existing_chunks,
        uploaded_chunks,
    );
    msg_chunks_found_existed_info(
        chunks_to_upload_amount,
        &time_since_mark_formatted,
        total_existing_chunks,
        uploaded_chunks,
    );
    msg_payment_details(
        files_upload.get_upload_storage_cost(),
        files_upload.get_upload_royalty_fees(),
        files_upload.get_upload_final_balance(),
        uploaded_chunks,
    );

    msg_made_payment_info(files_upload.get_upload_storage_cost(), uploaded_chunks);
}
fn msg_chunk_manager_upload_complete(chunk_manager: ChunkManager) {
    for (file_name, addr) in chunk_manager.completed_files() {
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
fn msg_made_payment_info(total_storage_cost: NanoTokens, uploaded_chunks: usize) {
    info!("Made payment of {total_storage_cost} for {uploaded_chunks} chunks");
}

fn msg_chunks_found_existed_info(
    chunks_to_upload_len: usize,
    elapsed: &String,
    total_existing_chunks: u64,
    uploaded_chunks: usize,
) {
    info!("Among {chunks_to_upload_len} chunks, found {total_existing_chunks} already existed in network, \
    uploaded the leftover {uploaded_chunks} chunks in {elapsed}");
}

fn msg_chunks_found_existed(
    chunks_to_upload_len: usize,
    elapsed: &String,
    total_existing_chunks: u64,
    uploaded_chunks: usize,
) {
    println!("Among {chunks_to_upload_len} chunks, found {total_existing_chunks} already existed in network, \
    uploaded the leftover {uploaded_chunks} chunks in {elapsed}");
}

fn msg_payment_details(
    total_storage_cost: NanoTokens,
    total_royalty_fees: NanoTokens,
    final_balance: NanoTokens,
    uploaded_chunks: usize,
) {
    println!("**************************************");
    println!("*          Payment Details           *");
    println!("**************************************");
    println!("Made payment of {total_storage_cost} for {uploaded_chunks} chunks");
    println!("Made payment of {total_royalty_fees} for royalties fees");
    println!("New wallet balance: {final_balance}");
}

fn msg_chk_mgr_no_verified_file_nor_re_upload() {
    println!("chunk_manager doesn't have any verified_files, nor any failed_chunks to re-upload.");
}

fn msg_star_line() {
    println!("**************************************");
}

fn msg_not_public_by_default() {
    println!("*                                    *");
    println!("*  These are not public by default.  *");
    println!("*     Reupload with `-p` option      *");
    println!("*      to publish the datamaps.      *");
}

fn msg_files_already_uploaded_verified() {
    println!("All files were already uploaded and verified");
    println!("**************************************");
    println!("*          Uploaded Files            *");
}

fn msg_uploading_chunks(chunks_to_upload_len: usize) {
    println!("Uploading {chunks_to_upload_len} chunks",);
}

fn msg_not_public_by_default_banner() {
    println!("*                                    *");
    println!("*  These are not public by default.  *");
    println!("*     Reupload with `-p` option      *");
    println!("*      to publish the datamaps.      *");
}

fn msg_uploaded_files_banner() {
    println!("**************************************");
    println!("*          Uploaded Files            *");
}
fn msg_unverified_chunks_reattempted(failed_amount: &usize) {
    println!(
        "{failed_amount} chunks were uploaded in the past but failed to verify. Will attempt to upload them again..."
    );
}
