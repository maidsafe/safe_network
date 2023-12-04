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

use sn_client::{Client, Error as ClientError, Files, BATCH_SIZE};
use sn_protocol::storage::{Chunk, ChunkAddress};
use sn_transfers::{Error as TransfersError, NanoTokens, WalletError};
use std::{
    collections::BTreeSet,
    io::prelude::*,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;
use xor_name::XorName;

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
        } => {
            upload_files(
                path,
                client,
                root_dir,
                verify_store,
                batch_size,
                show_holders,
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
            let file_api: Files = Files::new(client.clone(), download_dir.clone());

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
) -> Result<()> {
    debug!("Uploading file(s) from {files_path:?}, batch size {batch_size:?} will verify?: {verify_store}");

    let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());
    if file_api.wallet()?.balance().is_zero() {
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
    println!("Input was split into {chunks_to_upload_len} chunks",);
    println!("Will now attempt to upload them...");

    let mut total_cost = NanoTokens::zero();
    let mut total_royalties = NanoTokens::zero();
    let mut final_balance = file_api.wallet()?.balance();
    let chunks_batches = chunks_to_upload.chunks(batch_size);
    let now = Instant::now();

    // Task set to add and remove chunks from the chunk manager
    let mut uploading_chunks = FuturesUnordered::new();
    let mut total_exist_chunks = 0;

    for chunks_batch in chunks_batches {
        // while we dont have a full batch_size of ongoing uploading_chunks
        // we can pay for the next batch and carry on
        while uploading_chunks.len() >= batch_size {
            if let Some(result) = uploading_chunks.next().await {
                // bail if we've had any errors so far
                match result? {
                    // or cleanup via chunk_manager
                    Ok(xorname) => {
                        // mark the chunk as completed
                        chunk_manager.mark_completed(std::iter::once(xorname));
                    }
                    Err(report) => {
                        error!("Failed to upload a chunk: {report}");
                        return Err(report);
                    }
                }
            }
        }

        // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
        let skipped_chunks = match file_api
            .pay_for_chunks(chunks_batch.iter().map(|(name, _)| *name).collect())
            .await
        {
            Ok(((storage_cost, royalties_fees, new_balance), skipped_chunks)) => {
                final_balance = new_balance;
                total_cost = total_cost
                    .checked_add(storage_cost)
                    .ok_or_else(|| eyre!("Unable to add cost to total cost"))?;
                total_royalties = total_royalties
                    .checked_add(royalties_fees)
                    .ok_or_else(|| eyre!("Unable to add cost to total royalties fees"))?;
                skipped_chunks
            }
            Err(ClientError::Transfers(WalletError::Transfer(
                TransfersError::NotEnoughBalance(available, required),
            ))) => {
                bail!("Not enough balance in wallet to pay for chunk. We have {available:?} but need {required:?} to pay for the chunk");
            }
            Err(error) => {
                bail!("Failed to pay for chunks: {error}");
            }
        };

        let mut chunks_to_upload = chunks_batch.to_vec();
        chunks_to_upload.retain(|(name, _)| !skipped_chunks.contains(name));

        total_exist_chunks += skipped_chunks.len();
        progress_bar.inc(skipped_chunks.len() as u64);
        chunk_manager.mark_completed(skipped_chunks.into_iter());

        // upload paid chunks
        let upload_tasks = upload_chunks_in_parallel(
            &file_api,
            chunks_to_upload,
            verify_store,
            &progress_bar,
            show_holders,
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
            while uploading_chunks.len() >= batch_size {
                if let Some(result) = uploading_chunks.next().await {
                    match result? {
                        Ok(xorname) => {
                            // mark the chunk as completed
                            chunk_manager.mark_completed(std::iter::once(xorname));
                        }
                        Err(report) => {
                            // recorded_upload_errors.push(report);
                            error!("Failed to upload a chunk: {report}");
                            return Err(report);
                        }
                    }
                }
            }

            uploading_chunks.push(task);
        }
    }

    // ensure we wait on any remaining uploading_chunks
    while let Some(result) = uploading_chunks.next().await {
        // bail if we've errored so far
        match result? {
            // or cleanup via chunk_manager
            Ok(xorname) => {
                // mark the chunk as completed
                chunk_manager.mark_completed(std::iter::once(xorname));
            }
            Err(report) => {
                // recorded_upload_errors.push(report);
                error!("Failed to upload a chunk: {report}");
                bail!(report);
            }
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
    println!("Uploaded {chunks_to_upload_len} chunks (with {total_exist_chunks} exist chunks) in {elapsed}");
    info!("Uploaded {chunks_to_upload_len} chunks (with {total_exist_chunks} exist chunks) in {elapsed}");

    println!("**************************************");
    println!("*          Payment Details           *");
    println!("**************************************");
    println!("Made payment of {total_cost} for {chunks_to_upload_len} chunks");
    println!("Made payment of {total_royalties} for royalties fees");
    println!("New wallet balance: {final_balance}");
    info!("Made payment of {total_cost} for {chunks_to_upload_len} chunks");
    info!("New wallet balance: {final_balance}");

    Ok(())
}

/// Store all chunks from chunk_paths (assuming payments have already been made and are in our local wallet).
/// If verify_store is true, we will attempt to fetch all chunks from the network and check they are stored.
///
/// This spawns a task for each chunk to be uploaded, returns those handles.
///
fn upload_chunks_in_parallel(
    file_api: &Files,
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
    file_api: Files,
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
    file_api: &Files,
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
    file_api: &Files,
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
