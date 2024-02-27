use crate::subcommands::files;
use crate::subcommands::files::{ChunkManager, FilesUploadOptions};
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use sn_client::{Client, Error as ClientError, FileUploadEvent, FilesApi, FilesUpload};
use sn_transfers::{Error as TransfersError, NanoTokens, WalletError};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use walkdir::DirEntry;
use xor_name::XorName;

/// Given an iterator over files, upload them. Optionally verify if the data was stored successfully.
pub(crate) async fn upload_files_with_iter(
    entries_iter: impl Iterator<Item = DirEntry>,
    files_path: PathBuf,
    client: &Client,
    wallet_dir: PathBuf,
    root_dir: PathBuf,
    options: FilesUploadOptions,
) -> Result<()> {
    let FilesUploadOptions {
        make_data_public,
        verify_store,
        batch_size,
        retry_strategy,
    } = options;
    debug!("Uploading file(s) from {files_path:?}, batch size {batch_size:?} will verify?: {verify_store}");
    if make_data_public {
        info!("{files_path:?} will be made public and linkable");
        println!("{files_path:?} will be made public and linkable");
    }

    let files_api: FilesApi = FilesApi::new(client.clone(), wallet_dir);
    if files_api.wallet()?.balance().is_zero() {
        bail!("The wallet is empty. Cannot upload any files! Please transfer some funds into the wallet");
    }

    let mut chunk_manager = ChunkManager::new(&root_dir);
    println!("Starting to chunk {files_path:?} now.");
    chunk_manager.chunk_with_iter(entries_iter, true, make_data_public)?;

    // Return early if we already uploaded them
    let mut chunks_to_upload = if chunk_manager.is_chunks_empty() {
        // make sure we don't have any failed chunks in those
        let chunks = chunk_manager.already_put_chunks(&files_path, make_data_public)?;
        println!(
            "Files upload attempted previously, verifying {} chunks",
            chunks.len()
        );
        let failed_chunks = client.verify_uploaded_chunks(&chunks, batch_size).await?;

        // mark the non-failed ones as completed
        chunk_manager.mark_completed(
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
            if chunk_manager.completed_files().is_empty() {
                msg_chk_mgr_no_verified_file_nor_re_upload();
            }
            file_and_addr_in_chunk_mgr_completed_files_to_msg(chunk_manager);
            return Ok(());
        }
        msg_unverified_chunks_reattempted(&failed_chunks);
        failed_chunks
    } else {
        chunk_manager.get_chunks()
    };

    // Random shuffle the chunks_to_upload, so that uploading of a large file can be speed up by
    // having multiple client instances uploading the same target.
    let mut rng = thread_rng();
    chunks_to_upload.shuffle(&mut rng);

    let chunks_to_upload_len = chunks_to_upload.len();
    let progress_bar = files::get_progress_bar(chunks_to_upload.len() as u64)?;
    let total_existing_chunks = Arc::new(AtomicU64::new(0));
    let mut files_upload = FilesUpload::new(files_api)
        .set_batch_size(batch_size)
        .set_verify_store(verify_store)
        .set_retry_strategy(retry_strategy);
    let mut upload_event_rx = files_upload.get_upload_events();
    // keep track of the progress in a separate task
    let progress_bar_clone = progress_bar.clone();
    let total_existing_chunks_clone = total_existing_chunks.clone();

    let progress_handler = tokio::spawn(async move {
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
        if !upload_terminated_with_error {
            check_incomplete_files(&mut chunk_manager);

            // log uploaded file information
            msg_uploaded_files_banner();
            if !make_data_public {
                msg_not_public_by_default_banner();
            }
            msg_star_line();

            file_and_addr_in_chunk_mgr_completed_files_to_msg(chunk_manager);
        } else {
            error!("Got FileUploadEvent::Error inside upload event loop");
        }

        Ok::<_, ClientError>(())
    });

    // upload the files
    msg_uploading_chunks(chunks_to_upload_len);

    let now = Instant::now();
    let upload_result = match files_upload.upload_chunks(chunks_to_upload).await {
        Ok(()) => Ok(()),
        Err(ClientError::Transfers(WalletError::Transfer(TransfersError::NotEnoughBalance(
            available,
            required,
        )))) => Err(eyre!(
            "Not enough balance in wallet to pay for chunk. \
            We have {available:?} but need {required:?} to pay for the chunk"
        )),
        Err(err) => Err(eyre!("Failed to upload chunk batch: {err}")),
    };

    // bail on errors
    upload_result?;

    progress_handler
        .await?
        .map_err(|err| eyre!("Failed to write uploaded files with err: {err:?}"))?;

    let elapsed = format_elapsed_time(now.elapsed());
    let total_existing_chunks = total_existing_chunks.load(Ordering::Relaxed);
    let total_storage_cost = files_upload.get_upload_storage_cost();
    let total_royalty_fees = files_upload.get_upload_royalty_fees();
    let final_balance = files_upload.get_upload_final_balance();
    let uploaded_chunks = chunks_to_upload_len - total_existing_chunks as usize;

    msg_chunks_found_existed(
        chunks_to_upload_len,
        &elapsed,
        total_existing_chunks,
        uploaded_chunks,
    );
    msg_chunks_found_existed_info(
        chunks_to_upload_len,
        elapsed,
        total_existing_chunks,
        uploaded_chunks,
    );
    msg_payment_details(
        total_storage_cost,
        total_royalty_fees,
        final_balance,
        uploaded_chunks,
    );
    msg_made_payment_info(total_storage_cost, uploaded_chunks);

    Ok(())
}

/// Function to format elapsed time into a string
fn format_elapsed_time(elapsed_time: std::time::Duration) -> String {
    let elapsed_minutes = elapsed_time.as_secs() / 60;
    let elapsed_seconds = elapsed_time.as_secs() % 60;
    if elapsed_minutes > 0 {
        format!("{elapsed_minutes} minutes {elapsed_seconds} seconds")
    } else {
        format!("{elapsed_seconds} seconds")
    }
}

fn file_and_addr_in_chunk_mgr_completed_files_to_msg(chunk_manager: ChunkManager) {
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
fn check_incomplete_files(chunk_manager: &mut ChunkManager) {
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

/////////////////  Messages  /////////////////

fn msg_made_payment_info(total_storage_cost: NanoTokens, uploaded_chunks: usize) {
    info!("Made payment of {total_storage_cost} for {uploaded_chunks} chunks");
}

fn msg_chunks_found_existed_info(
    chunks_to_upload_len: usize,
    elapsed: String,
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
fn msg_unverified_chunks_reattempted(failed_chunks: &Vec<(XorName, PathBuf)>) {
    println!(
        "{:?} chunks were uploaded in the past but failed to verify. \
    Will attempt to upload them again...",
        failed_chunks.len()
    );
}
