// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk_manager;

pub(crate) use chunk_manager::ChunkManager;

use bytes::Bytes;
use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre},
    Help, Result,
};
use futures::{stream::FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use sn_client::{Client, Error as ClientError, FileUploadEvent, Files, FilesApi, BATCH_SIZE};
use sn_protocol::storage::{Chunk, ChunkAddress};
use sn_transfers::{Error as TransfersError, NanoTokens, WalletError};
use std::{
    collections::BTreeSet,
    io::prelude::*,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;
use xor_name::XorName;

/// The maximum number of sequential payment failures before aborting the upload process.
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 3;

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Upload {
        /// The location of the file(s) to upload.
        ///
        /// Can be a file or a directory.
        #[clap(name = "path", value_name = "PATH")]
        path: PathBuf,
        /// The batch_size to split chunks into parallel handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE, short='b')]
        batch_size: usize,
        /// Flagging whether to show the holders of the uploaded chunks.
        /// Default to be not showing.
        #[clap(long, name = "show_holders", default_value = "false")]
        show_holders: bool,
        /// The retry_count for retrying failed chunks
        /// during payment and upload processing.
        /// Defaults to 3 retry passes over unsuccessful chunks.
        #[clap(long, default_value = "3", short = 'r')]
        max_retries: usize,
    },
    Download {
        /// The name to apply to the downloaded file.
        ///
        /// If the name argument is used, the address argument must also be supplied.
        ///
        /// If neither are, all the files uploaded by the current user will be downloaded again.
        #[clap(name = "name")]
        file_name: Option<String>,
        /// The hex address of a file.
        ///
        /// If the address argument is used, the name argument must also be supplied.
        ///
        /// If neither are, all the files uploaded by the current user will be downloaded again.
        #[clap(name = "address")]
        file_addr: Option<String>,
        /// Flagging whether to show the holders of the uploaded chunks.
        /// Default to be not showing.
        #[clap(long, name = "show_holders", default_value = "false")]
        show_holders: bool,
        /// The batch_size for parallel downloading
        #[clap(long, default_value_t = BATCH_SIZE / 4, short='b')]
        batch_size: usize,
    },
}

pub(crate) async fn files_cmds(
    cmds: FilesCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        FilesCmds::Upload {
            path,
            batch_size,
            show_holders,
            max_retries,
        } => {
            upload_files(
                path,
                client,
                root_dir,
                verify_store,
                batch_size,
                show_holders,
                max_retries,
            )
            .await?
        }
        FilesCmds::Download {
            file_name,
            file_addr,
            show_holders,
            batch_size,
        } => {
            if (file_name.is_some() && file_addr.is_none())
                || (file_addr.is_some() && file_name.is_none())
            {
                return Err(
                    eyre!("Both the name and address must be supplied if either are used")
                        .suggestion(
                        "Please run the command again in the form 'files upload <name> <address>'",
                    ),
                );
            }

            let download_dir = dirs_next::download_dir().unwrap_or(root_dir.to_path_buf());
            let file_api: FilesApi = FilesApi::new(client.clone(), download_dir.clone());

            match (file_name, file_addr) {
                (Some(name), Some(address)) => {
                    let bytes = hex::decode(address).expect("Input address is not a hex string");
                    let xor_name = XorName(
                        bytes
                            .try_into()
                            .expect("Failed to parse XorName from hex string"),
                    );
                    download_file(
                        &file_api,
                        &xor_name,
                        &name,
                        &download_dir,
                        show_holders,
                        batch_size,
                    )
                    .await
                }
                _ => {
                    println!("Attempting to download all files uploaded by the current user...");
                    download_files(&file_api, root_dir, show_holders, batch_size).await?
                }
            }
        }
    };
    Ok(())
}

/// Given a file or directory, upload either the file or all the files in the directory. Optionally
/// verify if the data was stored successfully.
async fn upload_files(
    files_path: PathBuf,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
    batch_size: usize,
    show_holders: bool,
    max_retries: usize,
) -> Result<()> {
    debug!("Uploading file(s) from {files_path:?}, batch size {batch_size:?} will verify?: {verify_store}");

    let files_api: FilesApi = FilesApi::new(client.clone(), root_dir.to_path_buf());
    if files_api.wallet()?.balance().is_zero() {
        bail!("The wallet is empty. Cannot upload any files! Please transfer some funds into the wallet");
    }
    let mut chunk_manager = ChunkManager::new(root_dir);
    chunk_manager.chunk_path(&files_path, true)?;

    // Return early if we already uploaded them
    let chunks_to_upload;
    if chunk_manager.is_chunks_empty() {
        // make sure we don't have any failed chunks in those
        let chunks = chunk_manager.already_put_chunks(&files_path)?;
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
        );

        // if none are failed, we can return early
        if failed_chunks.is_empty() {
            println!("All files were already uploaded and verified");
            println!("**************************************");
            println!("*          Uploaded Files            *");
            println!("**************************************");
            for (file_name, addr) in chunk_manager.verified_files() {
                if let Some(file_name) = file_name.to_str() {
                    println!("\"{file_name}\" {addr:x}");
                    info!("Uploaded {file_name} to {addr:x}");
                } else {
                    println!("\"{file_name:?}\" {addr:x}");
                    info!("Uploaded {file_name:?} to {addr:x}");
                }
            }
            return Ok(());
        }
        println!("{:?} chunks were uploaded in the past but failed to verify. Will attempt to upload them again...", failed_chunks.len());
        chunks_to_upload = failed_chunks;
    } else {
        chunks_to_upload = chunk_manager.get_chunks();
    }

    let chunks_to_upload_len = chunks_to_upload.len();

    let progress_bar = get_progress_bar(chunks_to_upload.len() as u64)?;
    let mut total_cost = AtomicU64::new(0);
    let mut total_royalties = AtomicU64::new(0);
    let mut final_balance = AtomicU64::new(files_api.wallet()?.balance().as_nano());
    let mut files = Files::new(
        files_api,
        batch_size,
        verify_store,
        show_holders,
        max_retries,
    );
    let upload_event_rx = files.get_upload_events();
    // keep track of the progress in a separate task
    let progress_bar_clone = progress_bar.clone();
    tokio::spawn(async move {
        while let Some(event) = upload_event_rx.recv().await {
            match event {
                FileUploadEvent::Uploaded(addr) => {
                    progress_bar.inc(1);
                }
                FileUploadEvent::PayedForChunks {
                    storage_cost,
                    royalty_fees,
                    new_balance,
                } => {
                    let _ = total_cost.fetch_add(storage_cost.as_nano(), Ordering::Relaxed);
                    let _ = total_royalties.fetch_add(royalty_fees.as_nano(), Ordering::Relaxed);
                    let _ = final_balance.store(new_balance.as_nano(), Ordering::Relaxed);
                }
                FileUploadEvent::FailedToUpload(addr) => {}
            }
        }
    });
    println!("Uploading {chunks_to_upload_len} chunks",);

    let now = Instant::now();
    match files.upload_chunks(chunks_to_upload).await {
        Ok(()) => {}
        Err(ClientError::Transfers(WalletError::Transfer(TransfersError::NotEnoughBalance(
            available,
            required,
        )))) => {
            bail!("Not enough balance in wallet to pay for chunk. We have {available:?} but need {required:?} to pay for the chunk");
        }
        Err(err) => {
            bail!("Failed to upload chunk batch: {err}");
        }
    }
    progress_bar.finish_and_clear();

    // log uploaded file information
    println!("**************************************");
    println!("*          Uploaded Files            *");
    println!("**************************************");
    let file_names_path = root_dir.join("uploaded_files");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(file_names_path)?;
    for (file_name, addr) in chunk_manager.verified_files() {
        if let Some(file_name) = file_name.to_str() {
            println!("\"{file_name}\" {addr:x}");
            info!("Uploaded {file_name} to {addr:x}");
            writeln!(file, "{addr:x}: {file_name}")?;
        } else {
            println!("\"{file_name:?}\" {addr:x}");
            info!("Uploaded {file_name:?} to {addr:x}");
            writeln!(file, "{addr:x}: {file_name:?}")?;
        }
    }

    file.flush()?;

    let elapsed = format_elapsed_time(now.elapsed());
    let uploaded_chunks = chunks_to_upload_len - total_existing_chunks;
    println!("Among {chunks_to_upload_len} chunks, find {total_existing_chunks} already existed in network, uploaded the leftover {uploaded_chunks} chunks in {elapsed}");
    info!("Among {chunks_to_upload_len} chunks, find {total_existing_chunks} already existed in network, uploaded the leftover {uploaded_chunks} chunks in {elapsed}");

    println!("**************************************");
    println!("*          Payment Details           *");
    println!("**************************************");
    println!("Made payment of {total_cost} for {uploaded_chunks} chunks");
    println!("Made payment of {total_royalties} for royalties fees");
    println!("New wallet balance: {final_balance}");
    info!("Made payment of {total_cost} for {uploaded_chunks} chunks");
    info!("New wallet balance: {final_balance}");

    Ok(())
}

struct UploadParams<'a> {
    total_existing_chunks: &'a mut usize,
    file_api: &'a FilesApi,
    chunk_manager: &'a mut ChunkManager,
    uploading_chunks: &'a mut FuturesUnordered<JoinHandle<Result<XorName>>>,
    verify_store: bool,
    progress_bar: &'a ProgressBar,
    show_holders: bool,
    total_cost: &'a mut NanoTokens,
    total_royalties: &'a mut NanoTokens,
    final_balance: &'a mut NanoTokens,
    batch_size: usize,
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
async fn progress_uploading_chunks(params: &mut UploadParams<'_>, drain_all: bool) -> Result<()> {
    while drain_all || params.uploading_chunks.len() >= params.batch_size {
        if let Some(result) = params.uploading_chunks.next().await {
            // bail if we've had any errors so far
            match result? {
                // or cleanup via chunk_manager
                Ok(xorname) => {
                    // mark the chunk as completed
                    params
                        .chunk_manager
                        .mark_completed(std::iter::once(xorname));
                }
                Err(report) => {
                    warn!("Failed to upload a chunk: {report}");
                }
            }
        } else {
            // we're finished
            break;
        }
    }
    Ok(())
}

/// Handles a batch of chunks for upload. This includes paying for the chunks, uploading them,
/// and handling any errors that occur during the process.
async fn handle_chunk_batch(
    params: &mut UploadParams<'_>,
    chunks_batch: &[(XorName, PathBuf)],
) -> Result<()> {
    // while we dont have a full batch_size of ongoing uploading_chunks
    // we can pay for the next batch and carry on
    progress_uploading_chunks(params, false).await?;

    // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
    let skipped_chunks = match params
        .file_api
        .pay_for_chunks(chunks_batch.iter().map(|(name, _)| *name).collect())
        .await
    {
        Ok(((storage_cost, royalties_fees, new_balance), skipped_chunks)) => {
            *params.final_balance = new_balance;
            *params.total_cost = params
                .total_cost
                .checked_add(storage_cost)
                .ok_or_else(|| eyre!("Unable to add cost to total cost"))?;
            *params.total_royalties = params
                .total_royalties
                .checked_add(royalties_fees)
                .ok_or_else(|| eyre!("Unable to add cost to total royalties fees"))?;
            skipped_chunks
        }
        Err(error) => return Err(eyre!(error)),
    };

    let mut chunks_to_upload = chunks_batch.to_vec();
    // dont reupload skipped chunks
    chunks_to_upload.retain(|(name, _)| !skipped_chunks.contains(name));

    // update totals, progress and chunk management for skipped chunks
    *params.total_existing_chunks += skipped_chunks.len();
    params.progress_bar.inc(skipped_chunks.len() as u64);
    params
        .chunk_manager
        .mark_completed(skipped_chunks.into_iter());

    // upload paid chunks
    let upload_tasks = upload_chunks_in_parallel(
        params.file_api,
        chunks_to_upload,
        params.verify_store,
        params.progress_bar,
        params.show_holders,
    );

    for task in upload_tasks {
        // if we have a full batch_size of ongoing uploading_chunks
        // wait until there is space before we start adding more
        //
        // This should ensure that we're always uploading a full batch_size
        // of chunks, instead of waiting on 1/2 stragglers
        //
        // We bail on _any_ error here as we want to stop the upload process
        // and there are no more retries after this point
        progress_uploading_chunks(params, false).await?;

        params.uploading_chunks.push(task);
    }

    Ok(())
}

/// Store all chunks from chunk_paths (assuming payments have already been made and are in our local wallet).
/// If verify_store is true, we will attempt to fetch all chunks from the network and check they are stored.
///
/// This spawns a task for each chunk to be uploaded, returns those handles.
///
fn upload_chunks_in_parallel(
    file_api: &FilesApi,
    chunks_paths: Vec<(XorName, PathBuf)>,
    verify_store: bool,
    progress_bar: &ProgressBar,
    show_holders: bool,
) -> Vec<JoinHandle<Result<XorName>>> {
    let mut upload_handles = Vec::new();
    for (name, path) in chunks_paths.into_iter() {
        let file_api = file_api.clone();
        let progress_bar = progress_bar.clone();

        // Spawn a task for each chunk to be uploaded
        let handle = tokio::spawn(upload_chunk(
            file_api,
            (name, path),
            verify_store,
            progress_bar,
            show_holders,
        ));
        upload_handles.push(handle);
    }

    // Return the handles immediately without awaiting their completion
    upload_handles
}

/// Store chunks from chunk_paths (assuming payments have already been made and are in our local wallet).
/// If verify_store is true, we will attempt to fetch the chunks from the network to verify it is stored.
async fn upload_chunk(
    file_api: FilesApi,
    chunk: (XorName, PathBuf),
    verify_store: bool,
    progress_bar: ProgressBar,
    show_holders: bool,
) -> Result<XorName> {
    let (xorname, path) = chunk;
    let bytes = match tokio::fs::read(path.clone()).await {
        Ok(bytes) => Bytes::from(bytes),
        Err(error) => {
            warn!("Chunk {xorname:?} could not be read from the system from {path:?}. 
            Normally this happens if it has been uploaded, but the cleanup process was interrupted. Ignoring error: {error}");

            return Ok(xorname);
        }
    };
    let chunk = Chunk::new(bytes);
    file_api
        .get_local_payment_and_upload_chunk(chunk, verify_store, show_holders)
        .await?;
    progress_bar.inc(1);
    Ok(xorname)
}

async fn download_files(
    file_api: &FilesApi,
    root_dir: &Path,
    show_holders: bool,
    batch_size: usize,
) -> Result<()> {
    info!("Downloading with batch size of {}", batch_size);
    let uploaded_files_path = root_dir.join("uploaded_files");
    let download_path = dirs_next::download_dir().unwrap_or(root_dir.join("downloaded_files"));
    std::fs::create_dir_all(download_path.as_path())?;

    let file = std::fs::File::open(&uploaded_files_path)?;
    let reader = BufReader::new(file);
    let mut uploaded_files = BTreeSet::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(": ").collect();

        if parts.len() == 2 {
            let xor_name_hex = parts[0];
            let file_name = parts[1];

            let bytes = hex::decode(xor_name_hex)?;
            let xor_name_bytes: [u8; 32] = bytes
                .try_into()
                .expect("Failed to parse XorName from hex string");
            let xor_name = XorName(xor_name_bytes);

            uploaded_files.insert((xor_name, file_name.to_string()));
        } else {
            println!("Skipping malformed line: {line}");
        }
    }

    for (xorname, file_name) in uploaded_files.iter() {
        download_file(
            file_api,
            xorname,
            file_name,
            &download_path,
            show_holders,
            batch_size,
        )
        .await;
    }

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

async fn download_file(
    file_api: &FilesApi,
    xorname: &XorName,
    file_name: &String,
    download_path: &Path,
    show_holders: bool,
    batch_size: usize,
) {
    println!("Downloading {file_name} from {xorname:64x} with batch-size {batch_size}");
    debug!("Downloading {file_name} from {:64x}", xorname);
    let downloaded_file_path = download_path.join(file_name);
    match file_api
        .read_bytes(
            ChunkAddress::new(*xorname),
            Some(downloaded_file_path.clone()),
            show_holders,
            batch_size,
        )
        .await
    {
        Ok(_) => {
            debug!(
                "Saved {file_name} at {}",
                downloaded_file_path.to_string_lossy()
            );
            println!(
                "Saved {file_name} at {}",
                downloaded_file_path.to_string_lossy()
            );
        }
        Err(error) => {
            error!("Error downloading {file_name:?}: {error}");
            println!("Error downloading {file_name:?}: {error}")
        }
    }
}

fn get_progress_bar(length: u64) -> Result<ProgressBar> {
    let progress_bar = ProgressBar::new(length);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")?
            .progress_chars("#>-"),
    );
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    Ok(progress_bar)
}
