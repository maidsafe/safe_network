// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use bytes::Bytes;
use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre},
    Help, Result,
};
use indicatif::{ProgressBar, ProgressStyle};
use rand::{seq::SliceRandom, thread_rng};
use serde::Deserialize;
use walkdir::WalkDir;
use xor_name::XorName;

pub(crate) use chunk_manager::ChunkManager;
use sn_client::{
    Client, Error as ClientError, FileUploadEvent, FilesApi, FilesDownload, FilesDownloadEvent,
    FilesUpload, BATCH_SIZE,
};
use sn_protocol::storage::{Chunk, ChunkAddress, RetryStrategy};
use sn_protocol::NetworkAddress;
use sn_transfers::{Error as TransfersError, WalletError};

mod chunk_manager;

/// The default folder to download files to.
const DOWNLOAD_FOLDER: &str = "safe_files";

/// Subdir for storing uploaded file into
pub(crate) const UPLOADED_FILES: &str = "uploaded_files";

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Estimate {
        /// The location of the file(s) to upload. Can be a file or a directory.
        #[clap(name = "path", value_name = "PATH")]
        path: PathBuf,
    },
    Upload {
        /// The location of the file(s) to upload. Can be a file or a directory.
        #[clap(name = "path", value_name = "PATH")]
        path: PathBuf,
        /// The batch_size to split chunks into parallel handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE, short = 'b')]
        batch_size: usize,
        /// Should the file be made accessible to all. (This is irreversible)
        #[clap(long, name = "make_public", default_value = "false", short = 'p')]
        make_public: bool,
        /// Set the strategy to use on chunk upload failure. Does not modify the spend failure retry attempts yet.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Balanced, short = 'r', help = "Sets the retry strategy on upload \
        failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
    Download {
        /// The name to apply to the downloaded file.
        ///
        /// If the name argument is used, the address argument must also be supplied.
        ///
        /// If neither are, all the files uploaded by the current user will be downloaded again.
        #[clap(name = "name")]
        file_name: Option<OsString>,
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
        #[clap(long, default_value_t = BATCH_SIZE, short = 'b')]
        batch_size: usize,
        /// Set the strategy to use on downloads failure.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Quick, short = 'r', help = "Sets the retry strategy on download failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
}

/// The metadata related to file that has been uploaded.
/// This is written during upload and read during downloads.
#[derive(Clone, Debug, Deserialize)]
pub struct UploadedFile {
    pub filename: OsString,
    pub data_map: Option<Bytes>,
}

impl UploadedFile {
    /// Write an UploadedFile to a path identified by the hex of the head ChunkAddress.
    /// If you want to update the data_map to None, calling this function will overwrite the previous value.
    pub fn write(&self, root_dir: &Path, head_chunk_address: &ChunkAddress) -> Result<()> {
        let uploaded_files = root_dir.join(UPLOADED_FILES);
        if !uploaded_files.exists() {
            if let Err(error) = std::fs::create_dir_all(&uploaded_files) {
                error!("Failed to create {uploaded_files:?} because {error:?}");
            }
        }
        let uploaded_file_path = uploaded_files.join(head_chunk_address.to_hex());

        if self.data_map.is_none() {
            warn!(
                "No datamap being written for {:?} as it is empty",
                self.filename
            );
        }
        let serialized = rmp_serde::to_vec(&(&self.filename, &self.data_map)).map_err(|err| {
            error!("Failed to serialize UploadedFile");
            err
        })?;

        std::fs::write(&uploaded_file_path, serialized).map_err(|err| {
            error!(
                "Could not write UploadedFile of {:?} to {uploaded_file_path:?}",
                self.filename
            );
            err
        })?;

        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|err| {
            error!("Error while reading the UploadedFile from {path:?}");
            err
        })?;
        let metadata = rmp_serde::from_slice(&bytes).map_err(|err| {
            error!("Error while deserializing UploadedFile for {path:?}");
            err
        })?;
        Ok(metadata)
    }
}

pub(crate) async fn files_cmds(
    cmds: FilesCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        FilesCmds::Estimate { path } => estimate_cost(path, client, root_dir).await?,
        FilesCmds::Upload {
            path,
            batch_size,
            retry_strategy,
            make_public,
        } => {
            upload_files(
                path,
                make_public,
                client,
                root_dir.to_path_buf(),
                verify_store,
                batch_size,
                retry_strategy,
            )
            .await?
        }
        FilesCmds::Download {
            file_name,
            file_addr,
            show_holders,
            batch_size,
            retry_strategy,
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
            let files_api: FilesApi = FilesApi::new(client.clone(), download_dir.clone());

            match (file_name, file_addr) {
                (Some(file_name), Some(address_provided)) => {
                    let bytes =
                        hex::decode(&address_provided).expect("Input address is not a hex string");
                    let xor_name_provided = XorName(
                        bytes
                            .try_into()
                            .expect("Failed to parse XorName from hex string"),
                    );
                    // try to read the data_map if it exists locally.
                    let uploaded_files_path = root_dir.join(UPLOADED_FILES);
                    let expected_data_map_location = uploaded_files_path.join(address_provided);
                    let local_data_map = {
                        if expected_data_map_location.exists() {
                            let uploaded_file_metadata =
                                UploadedFile::read(&expected_data_map_location)?;

                            uploaded_file_metadata.data_map.map(|bytes| Chunk {
                                address: ChunkAddress::new(xor_name_provided),
                                value: bytes,
                            })
                        } else {
                            None
                        }
                    };

                    download_file(
                        files_api,
                        xor_name_provided,
                        (file_name, local_data_map),
                        &download_dir,
                        show_holders,
                        batch_size,
                        retry_strategy,
                    )
                    .await
                }
                _ => {
                    println!("Attempting to download all files uploaded by the current user...");
                    download_files(
                        &files_api,
                        root_dir,
                        show_holders,
                        batch_size,
                        retry_strategy,
                    )
                    .await?
                }
            }
        }
    };
    Ok(())
}

/// Estimate the upload cost of a chosen file
pub(crate) async fn estimate_cost(path: PathBuf, client: &Client, root_dir: &Path) -> Result<()> {
    let mut chunk_manager = ChunkManager::new(root_dir);
    chunk_manager.chunk_path(&path, false, false)?;

    let mut estimate: u64 = 0;

    for chunk in chunk_manager.get_chunks() {
        estimate += FilesApi::new(client.clone(), root_dir.to_path_buf())
            .wallet()?
            .get_store_cost_at_address(NetworkAddress::from_chunk_address(ChunkAddress::new(
                chunk.0,
            )))
            .await?
            .2
            .cost
            .as_nano();
    }
    println!("Estimated cost in NanoTokens: {estimate} "); // TODO: balance after transfer.
    Ok(())
}

/// Given a file or directory, upload either the file or all the files in the directory. Optionally
/// verify if the data was stored successfully.
pub(crate) async fn upload_files(
    files_path: PathBuf,
    make_data_public: bool,
    client: &Client,
    root_dir: PathBuf,
    verify_store: bool,
    batch_size: usize,
    retry_strategy: RetryStrategy,
) -> Result<()> {
    debug!("Uploading file(s) from {files_path:?}, batch size {batch_size:?} will verify?: {verify_store}");
    if make_data_public {
        info!("{files_path:?} will be made public and linkable");
        println!("{files_path:?} will be made public and linkable");
    }

    let files_api: FilesApi = FilesApi::new(client.clone(), root_dir.to_path_buf());

    if files_api.wallet()?.balance().is_zero() {
        bail!("The wallet is empty. Cannot upload any files! Please transfer some funds into the wallet");
    }

    let mut chunk_manager = ChunkManager::new(&root_dir);
    println!("Starting to chunk {files_path:?} now.");
    chunk_manager.chunk_path(&files_path, true, make_data_public)?;

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
            println!("All files were already uploaded and verified");
            println!("**************************************");
            println!("*          Uploaded Files            *");

            if !make_data_public {
                println!("*                                    *");
                println!("*  These are not public by default.  *");
                println!("*     Reupload with `-p` option      *");
                println!("*      to publish the datamaps.      *");
            }
            println!("**************************************");

            if chunk_manager.completed_files().is_empty() {
                println!("chunk_manager doesn't have any verified_files, nor any failed_chunks to re-upload.");
            }
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
            return Ok(());
        }
        println!("{:?} chunks were uploaded in the past but failed to verify. Will attempt to upload them again...", failed_chunks.len());
        failed_chunks
    } else {
        chunk_manager.get_chunks()
    };

    // Random shuffle the chunks_to_upload, so that uploading of a large file can be speed up by
    // having multiple client instances uploading the same target.
    let mut rng = thread_rng();
    chunks_to_upload.shuffle(&mut rng);

    let chunks_to_upload_len = chunks_to_upload.len();
    let progress_bar = get_progress_bar(chunks_to_upload.len() as u64)?;
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
            for file_name in chunk_manager.incomplete_files() {
                if let Some(file_name) = file_name.to_str() {
                    println!("Unverified file \"{file_name}\", suggest to re-upload again.");
                    info!("Unverified {file_name}");
                } else {
                    println!("Unverified file \"{file_name:?}\", suggest to re-upload again.");
                    info!("Unverified file {file_name:?}");
                }
            }

            // log uploaded file information
            println!("**************************************");
            println!("*          Uploaded Files            *");
            if !make_data_public {
                println!("*                                    *");
                println!("*  These are not public by default.  *");
                println!("*     Reupload with `-p` option      *");
                println!("*      to publish the datamaps.      *");
            }
            println!("**************************************");
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
        } else {
            error!("Got FileUploadEvent::Error inside upload event loop");
        }

        Ok::<_, ClientError>(())
    });

    // upload the files
    println!("Uploading {chunks_to_upload_len} chunks",);
    let now = Instant::now();
    let upload_result = match files_upload.upload_chunks(chunks_to_upload).await {
        Ok(()) => {Ok(())}
        Err(ClientError::Transfers(WalletError::Transfer(TransfersError::NotEnoughBalance(
            available,
            required,
        )))) => {
            Err(eyre!("Not enough balance in wallet to pay for chunk. We have {available:?} but need {required:?} to pay for the chunk"))
        }
        Err(err) => {
            Err(eyre!("Failed to upload chunk batch: {err}"))
        }
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
    println!("Among {chunks_to_upload_len} chunks, found {total_existing_chunks} already existed in network, uploaded the leftover {uploaded_chunks} chunks in {elapsed}");
    info!("Among {chunks_to_upload_len} chunks, found {total_existing_chunks} already existed in network, uploaded the leftover {uploaded_chunks} chunks in {elapsed}");

    println!("**************************************");
    println!("*          Payment Details           *");
    println!("**************************************");
    println!("Made payment of {total_storage_cost} for {uploaded_chunks} chunks");
    println!("Made payment of {total_royalty_fees} for royalties fees");
    println!("New wallet balance: {final_balance}");
    info!("Made payment of {total_storage_cost} for {uploaded_chunks} chunks");
    info!("New wallet balance: {final_balance}");

    Ok(())
}

async fn download_files(
    files_api: &FilesApi,
    root_dir: &Path,
    show_holders: bool,
    batch_size: usize,
    retry_strategy: RetryStrategy,
) -> Result<()> {
    info!("Downloading with batch size of {}", batch_size);
    let uploaded_files_path = root_dir.join(UPLOADED_FILES);
    let download_path = dirs_next::download_dir()
        .unwrap_or(root_dir.to_path_buf())
        .join(DOWNLOAD_FOLDER);
    std::fs::create_dir_all(download_path.as_path())?;

    #[allow(clippy::mutable_key_type)]
    let mut uploaded_files = BTreeSet::new();

    for entry in WalkDir::new(uploaded_files_path.clone()) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let hex_xorname = path
                .file_name()
                .expect("Uploaded file to have name")
                .to_str()
                .expect("Failed to convert path to string");
            let bytes = hex::decode(hex_xorname)?;
            let xor_name_bytes: [u8; 32] = bytes
                .try_into()
                .expect("Failed to parse XorName from hex string");
            let xor_name = XorName(xor_name_bytes);
            let address = ChunkAddress::new(xor_name);

            let uploaded_file_metadata = UploadedFile::read(path)?;
            let datamap_chunk = uploaded_file_metadata.data_map.map(|bytes| Chunk {
                address,
                value: bytes,
            });
            uploaded_files.insert((xor_name, (uploaded_file_metadata.filename, datamap_chunk)));
        }
    }

    for (xorname, file_data) in uploaded_files.into_iter() {
        download_file(
            files_api.clone(),
            xorname,
            file_data,
            &download_path,
            show_holders,
            batch_size,
            retry_strategy,
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

pub(crate) async fn download_file(
    files_api: FilesApi,
    xor_name: XorName,
    // original file name and optional datamap chunk
    (file_name, datamap): (OsString, Option<Chunk>),
    download_path: &Path,
    show_holders: bool,
    batch_size: usize,
    retry_strategy: RetryStrategy,
) {
    let mut files_download = FilesDownload::new(files_api.clone())
        .set_batch_size(batch_size)
        .set_show_holders(show_holders)
        .set_retry_strategy(retry_strategy);

    println!("Downloading {file_name:?} from {xor_name:64x} with batch-size {batch_size}");
    debug!("Downloading {file_name:?} from {:64x}", xor_name);
    let downloaded_file_path = download_path.join(&file_name);

    let mut download_events_rx = files_download.get_events();

    let progress_handler = tokio::spawn(async move {
        let mut progress_bar: Option<ProgressBar> = None;
        // The loop is guaranteed to end, as the channel will be closed when the download completes or errors out.
        while let Some(event) = download_events_rx.recv().await {
            match event {
                FilesDownloadEvent::Downloaded(_) => {
                    if let Some(progress_bar) = &progress_bar {
                        progress_bar.inc(1);
                    }
                }
                FilesDownloadEvent::ChunksCount(count) => {
                    // terminate the progress bar from datamap download.
                    if let Some(progress_bar) = progress_bar {
                        progress_bar.finish_and_clear();
                    }
                    progress_bar = get_progress_bar(count as u64).map_err(|err| {
                        println!("Unable to initialize progress bar. The download process will continue without a progress bar.");
                        error!("Failed to obtain progress bar with err: {err:?}");
                        err
                    }).ok();
                }
                FilesDownloadEvent::DatamapCount(count) => {
                    // terminate the progress bar if it was loaded here. This should not happen.
                    if let Some(progress_bar) = progress_bar {
                        progress_bar.finish_and_clear();
                    }
                    progress_bar = get_progress_bar(count as u64).map_err(|err| {
                        println!("Unable to initialize progress bar. The download process will continue without a progress bar.");
                        error!("Failed to obtain progress bar with err: {err:?}");
                        err
                    }).ok();
                }
                FilesDownloadEvent::Error => {
                    error!("Got FilesDownloadEvent::Error");
                }
            }
        }
        if let Some(progress_bar) = progress_bar {
            progress_bar.finish_and_clear();
        }
    });

    let download_result = files_download
        .download_file_to_path(
            ChunkAddress::new(xor_name),
            datamap,
            downloaded_file_path.clone(),
        )
        .await;

    // await on the progress handler first as we want to clear the progress bar before printing things.
    let _ = progress_handler.await;
    match download_result {
        Ok(_) => {
            debug!(
                "Saved {file_name:?} at {}",
                downloaded_file_path.to_string_lossy()
            );
            println!(
                "Saved {file_name:?} at {}",
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
