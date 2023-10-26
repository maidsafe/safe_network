// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk_manager;

pub(crate) use chunk_manager::ChunkManager;

use super::wallet::BATCH_SIZE;
use bytes::Bytes;
use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre, Error},
    Help, Result,
};
use indicatif::{ProgressBar, ProgressStyle};
use libp2p::futures::future::join_all;
use sn_client::{Client, Error as ClientError, Files};
use sn_protocol::storage::{Chunk, ChunkAddress};
use sn_transfers::{Error as TransfersError, NanoTokens, WalletError};
use std::{
    fs,
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
        /// The batch_size to split chunks into parallely handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE)]
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
    },
}

pub(crate) async fn files_cmds(
    cmds: FilesCmds,
    client: Client,
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

            let file_api: Files = Files::new(client, root_dir.to_path_buf());

            match (file_name, file_addr) {
                (Some(name), Some(address)) => {
                    let bytes = hex::decode(address).expect("Input address is not a hex string");
                    let xor_name = XorName(
                        bytes
                            .try_into()
                            .expect("Failed to parse XorName from hex string"),
                    );
                    download_file(&file_api, &xor_name, &name, root_dir, show_holders).await
                }
                _ => {
                    println!("Attempting to download all files uploaded by the current user...");
                    download_files(&file_api, root_dir, show_holders).await?
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
    client: Client,
    root_dir: &Path,
    verify_store: bool,
    batch_size: usize,
    show_holders: bool,
) -> Result<()> {
    debug!(
        "Uploading file(s) from {:?}, will verify?: {verify_store}",
        files_path
    );

    let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());
    if file_api.wallet()?.balance().is_zero() {
        bail!("The wallet is empty. Cannot upload any files! Please transfer some funds into the wallet");
    }
    let mut manager = ChunkManager::new(root_dir, file_api.clone());
    manager.chunk_path(&files_path)?;

    // let uploaded_file_info = chunked_files
    //     .iter()
    //     .map(|(file_addr, chunked_file)| (*file_addr, chunked_file.file_name.clone()))
    //     .collect::<Vec<_>>();

    let chunks_to_upload = manager.get_chunks();
    let progress_bar = get_progress_bar(chunks_to_upload.len() as u64)?;
    println!("Input was split into {} chunks", chunks_to_upload.len());
    println!("Will now attempt to upload them...");

    let mut total_cost = NanoTokens::zero();
    let mut final_balance = file_api.wallet()?.balance();
    let now = Instant::now();

    let chunks_batches = chunks_to_upload.chunks(batch_size);
    let verify_as_we_upload = verify_store && chunks_batches.len() == 1;

    for chunks_batch in chunks_batches {
        // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
        let (cost, new_balance) = match file_api
            .pay_for_chunks(chunks_batch.iter().map(|(name, _)| *name).collect())
            .await
        {
            Ok((cost, new_balance)) => (cost, new_balance),
            Err(ClientError::Transfers(WalletError::Transfer(
                TransfersError::NotEnoughBalance(available, required),
            ))) => {
                bail!("Not enough balance in wallet to pay for chunk. We have {available:?} but need {required:?} to pay for the chunk");
            }
            Err(error) => {
                warn!(
                    "Failed to pay for chunks. Validation steps should retry this batch: {error}"
                );
                continue;
            }
        };
        final_balance = new_balance;
        total_cost = total_cost
            .checked_add(cost)
            .ok_or_else(|| eyre!("Unable to add cost to total cost"))?;

        // Verification will be carried out later on, if being asked to.
        // Hence no need to carry out verification within the first attempt.
        // Just to check there were no odd errors during upload.
        for result in join_all(upload_chunks_in_parallel(
            file_api.clone(),
            chunks_batch.to_vec(),
            verify_as_we_upload,
            progress_bar.clone(),
            show_holders,
        ))
        .await
        {
            let _ = result?;
        }
    }

    progress_bar.finish_and_clear();
    let elapsed = now.elapsed();
    println!(
        "Uploaded {} chunks in {}",
        chunks_to_upload.len(),
        format_elapsed_time(elapsed)
    );
    info!(
        "Uploaded {} chunks in {}",
        chunks_to_upload.len(),
        format_elapsed_time(elapsed)
    );
    println!("**************************************");
    println!("*          Payment Details           *");
    println!("**************************************");
    println!(
        "Made payment of {total_cost} for {} chunks",
        chunks_to_upload.len()
    );
    println!("New wallet balance: {final_balance}");
    info!(
        "Made payment of {total_cost} for {} chunks",
        chunks_to_upload.len()
    );
    info!("New wallet balance: {final_balance}");

    // If we are not verifying, we can skip this
    if verify_store && !verify_as_we_upload {
        println!("**************************************");
        println!("*            Verification            *");
        println!("**************************************");

        let mut data_to_verify_or_repay = chunks_to_upload;
        while !data_to_verify_or_repay.is_empty() {
            trace!(
                "Verifying and potentially topping up payment of {:?} chunks",
                data_to_verify_or_repay.len()
            );
            data_to_verify_or_repay = verify_and_repay_if_needed(
                file_api.clone(),
                data_to_verify_or_repay,
                batch_size,
                show_holders,
            )
            .await?;
        }
    }

    progress_bar.finish_and_clear();

    println!("**************************************");
    println!("*          Uploaded Files            *");
    println!("**************************************");
    let file_names_path = root_dir.join("uploaded_files");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(file_names_path)?;
    // for (addr, file_name) in uploaded_file_info.iter() {
    //     if let Some(file_name) = file_name.to_str() {
    //         println!("\"{file_name}\" {addr:x}");
    //         info!("Uploaded {file_name} to {addr:x}");
    //         writeln!(file, "{addr:x}: {file_name}")?;
    //     } else {
    //         println!("\"{file_name}\" {addr:x}");
    //         info!("Uploaded {file_name:?} to {addr:x}");
    //         writeln!(file, "{addr:x}: {file_name:?}")?;
    //     }
    // }
    file.flush()?;

    Ok(())
}

/// Store all chunks from chunk_paths (assuming payments have already been made and are in our local wallet).
/// If verify_store is true, we will attempt to fetch all chunks from the network and check they are stored.
///
/// This spawns a task for each chunk to be uploaded, returns those handles.
///
fn upload_chunks_in_parallel(
    file_api: Files,
    chunks_paths: Vec<(XorName, PathBuf)>,
    verify_store: bool,
    progress_bar: ProgressBar,
    show_holders: bool,
) -> Vec<JoinHandle<Result<()>>> {
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
) -> Result<()> {
    let (_, path) = chunk;
    let chunk = Chunk::new(Bytes::from(fs::read(path)?));
    file_api
        .get_local_payment_and_upload_chunk(chunk, verify_store, show_holders)
        .await?;
    progress_bar.inc(1);
    Ok(())
}

/// Verify if chunks exist on the network.
/// Repay if they don't.
/// Return a list of files which had to be repaid, but are not yet reverified.
async fn verify_and_repay_if_needed(
    file_api: Files,
    chunks_paths: Vec<(XorName, PathBuf)>,
    batch_size: usize,
    show_holders: bool,
) -> Result<Vec<(XorName, PathBuf)>> {
    let total_chunks = chunks_paths.len();

    println!("{total_chunks} chunks to be checked and repaid if required");

    let progress_bar = get_progress_bar(total_chunks as u64)?;
    let now = Instant::now();
    let mut failed_chunks = Vec::new();
    for chunks_batch in chunks_paths.chunks(batch_size) {
        // now we try and get batched chunks, keep track of any that fail
        // Iterate over each uploaded chunk
        let mut verify_handles = Vec::new();
        for (name, path) in chunks_batch.iter().cloned() {
            let file_api = file_api.clone();
            let pb = progress_bar.clone();

            // Spawn a new task to fetch each chunk concurrently
            let handle = tokio::spawn(async move {
                let chunk_address: ChunkAddress = ChunkAddress::new(name);
                // make sure the chunk is stored
                let res = file_api.client().verify_chunk_stored(chunk_address).await;

                pb.inc(1);
                Ok::<_, Error>(((chunk_address, path), res.is_err()))
            });
            verify_handles.push(handle);
        }

        // Await all fetch tasks and collect the results
        let verify_results = join_all(verify_handles).await;

        // Check for any errors during fetch
        for result in verify_results {
            if let ((chunk_addr, path), true) = result?? {
                warn!("Failed to fetch a chunk {chunk_addr:?}");
                // This needs to be NetAddr to allow for repayment
                failed_chunks.push((chunk_addr, path));
            }
        }
    }

    progress_bar.finish_and_clear();
    let elapsed = now.elapsed();
    println!("Verified {total_chunks:?} chunks in {elapsed:?}");
    let now = Instant::now();

    let total_failed_chunks = failed_chunks
        .iter()
        .map(|(addr, path)| (*addr.xorname(), path.clone()))
        .collect::<Vec<_>>();

    if total_failed_chunks.is_empty() {
        println!("Verification complete: all chunks paid and stored");
        return Ok(total_failed_chunks);
    }

    let num_of_failed_chunks = failed_chunks.len();
    println!("{num_of_failed_chunks} chunks were not stored. Repaying them in batches.");
    let progress_bar = get_progress_bar(total_chunks as u64)?;

    // If there were any failed chunks, we need to repay them
    for failed_chunks_batch in failed_chunks.chunks(batch_size) {
        let mut wallet = file_api.wallet()?;

        // Now we pay again or top up, depending on the new current store cost is
        match wallet
            .pay_for_storage(
                failed_chunks_batch
                    .iter()
                    .map(|(addr, _path)| sn_protocol::NetworkAddress::ChunkAddress(*addr)),
            )
            .await
        {
            Ok(_new_cost) => {}
            Err(error) => {
                error!("Failed to repay for record storage: {failed_chunks_batch:?}: {error:?}");
            }
        };

        // outcome here is not important as we'll verify this later
        let upload_file_api = file_api.clone();
        let ongoing_uploads = upload_chunks_in_parallel(
            upload_file_api,
            failed_chunks_batch
                .iter()
                .cloned()
                .map(|(addr, path)| (*addr.xorname(), path))
                .collect(),
            false,
            progress_bar.clone(),
            show_holders,
        );

        // Now we've batched all payments, we can await all uploads to happen in parallel
        let upload_results = join_all(ongoing_uploads).await;

        // lets check there were no odd errors during upload
        for result in upload_results {
            if let Err(error) = result? {
                error!("Error uploading chunk during repayment: {error}");
            };
        }
    }

    let elapsed = now.elapsed();
    println!(
        "Repaid and re-uploaded {:?} chunks in {elapsed:?}",
        chunks_paths.len()
    );
    println!("{total_failed_chunks:?} were not yet verified...");

    Ok(total_failed_chunks)
}

async fn download_files(file_api: &Files, root_dir: &Path, show_holders: bool) -> Result<()> {
    let uploaded_files_path = root_dir.join("uploaded_files");
    let download_path = root_dir.join("downloaded_files");
    std::fs::create_dir_all(download_path.as_path())?;

    let file = std::fs::File::open(&uploaded_files_path)?;
    let reader = BufReader::new(file);
    let mut uploaded_files = Vec::new();
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

            uploaded_files.push((xor_name, file_name.to_string()));
        } else {
            println!("Skipping malformed line: {}", line);
        }
    }

    for (xorname, file_name) in uploaded_files.iter() {
        download_file(file_api, xorname, file_name, &download_path, show_holders).await;
    }

    Ok(())
}

/// Function to format elapsed time into a string
fn format_elapsed_time(elapsed_time: std::time::Duration) -> String {
    let elapsed_minutes = elapsed_time.as_secs() / 60;
    let elapsed_seconds = elapsed_time.as_secs() % 60;
    if elapsed_minutes > 0 {
        format!("{} minutes {} seconds", elapsed_minutes, elapsed_seconds)
    } else {
        format!("{} seconds", elapsed_seconds)
    }
}

async fn download_file(
    file_api: &Files,
    xorname: &XorName,
    file_name: &String,
    download_path: &Path,
    show_holders: bool,
) {
    println!("Downloading {file_name} from {:64x}", xorname);
    debug!("Downloading {file_name} from {:64x}", xorname);
    let downloaded_file_path = download_path.join(file_name);
    match file_api
        .read_bytes(
            ChunkAddress::new(*xorname),
            Some(downloaded_file_path.clone()),
            show_holders,
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
