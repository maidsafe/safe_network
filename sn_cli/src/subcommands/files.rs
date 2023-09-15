// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::{chunk_path, ChunkedFile, BATCH_SIZE};
use bytes::Bytes;
use clap::Parser;
use color_eyre::{eyre::Error, Result};
use libp2p::futures::future::join_all;
use sn_client::{Client, Files};
use sn_protocol::storage::{Chunk, ChunkAddress};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;
use walkdir::WalkDir;
use xor_name::XorName;

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Upload {
        /// The location of the files to upload.
        #[clap(name = "path", value_name = "DIRECTORY")]
        path: PathBuf,
        /// The batch_size to split chunks into parallely handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE)]
        batch_size: usize,
    },
    Download {
        /// Name of the file to download.
        #[clap(name = "file_name")]
        file_name: Option<String>,
        /// Address of the file to download, in hex string.
        #[clap(name = "file_addr")]
        file_addr: Option<String>,
    },
}

pub(crate) async fn files_cmds(
    cmds: FilesCmds,
    client: Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        FilesCmds::Upload { path, batch_size } => {
            upload_files(path, client, root_dir, verify_store, batch_size).await?
        }
        FilesCmds::Download {
            file_name,
            file_addr,
        } => {
            let file_api: Files = Files::new(client, root_dir.to_path_buf());

            match (file_name, file_addr) {
                (Some(name), Some(address)) => {
                    let bytes = hex::decode(address).expect("Input address is not a hex string");
                    download_file(
                        &file_api,
                        &XorName(
                            bytes
                                .try_into()
                                .expect("Failed to parse XorName from hex string"),
                        ),
                        &name,
                        root_dir,
                    )
                    .await
                }
                _ => {
                    println!("Trying to download files recorded in uploaded_files folder");
                    download_files(&file_api, root_dir).await?
                }
            }
        }
    };
    Ok(())
}

/// Given a directory, upload all files contained
/// Optionally verifies data was stored successfully
async fn upload_files(
    files_path: PathBuf,
    client: Client,
    root_dir: &Path,
    verify_store: bool,
    batch_size: usize,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    debug!(
        "Uploading files from {:?}, will verify?: {verify_store}",
        files_path
    );
    // The input files_path has to be a dir
    let file_names_path = root_dir.join("uploaded_files");

    // Payment shall always be verified.
    let chunks_to_upload = chunk_path(&client, root_dir, &files_path).await?;
    let mut uploaded_files = Vec::new();

    // gather all chunks to upload
    let mut chunks_for_upload = vec![];

    // Iterate over each file to be uploaded so we can track it
    for (file_addr, ChunkedFile { file_name, chunks }) in chunks_to_upload {
        uploaded_files.push((file_addr, file_name));

        // lets put all chunks together for more efficient batching
        chunks_for_upload.extend(chunks);
    }

    let total_chunks_uploading = chunks_for_upload.len();

    // take a clone of all uploads so we can verify them later
    let mut data_to_verify_or_repay = chunks_for_upload.clone();

    let mut progress = 0;
    // Clone necessary variables for each file upload
    let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());

    let mut ongoing_uploads = Vec::new();

    while !chunks_for_upload.is_empty() {
        let now = Instant::now();
        let size = std::cmp::min(batch_size, chunks_for_upload.len());
        progress += size;
        let chunk_batch: Vec<_> = chunks_for_upload.drain(..size).collect();

        // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
        file_api
            .pay_for_chunks(chunk_batch.iter().map(|(name, _)| *name).collect(), true)
            .await?;

        // Verification will be carried out later on, if being asked to.
        // Hence no need to carry out verification within the first attempt.
        ongoing_uploads.extend(upload_chunks_in_parallel(
            file_api.clone(),
            chunk_batch,
            false,
        ));

        let elapsed = now.elapsed();
        println!(
            "After {elapsed:?}, uploaded {size:?} chunks, current progress is {progress}/{}. ",
            total_chunks_uploading
        );
        info!(
            "After {elapsed:?}, uploaded {size:?} chunks, current progress is {progress}/{}. ",
            total_chunks_uploading
        );
    }

    // Now we've batched all payments, we can await all uploads to happen in parallel
    let upload_results = join_all(ongoing_uploads).await;

    // lets check there were no odd errors during upload
    for result in upload_results {
        result??;
    }

    println!("First round of upload completed, verifying and repaying if required...");

    // If we are not verifying, we can skip this
    if verify_store {
        let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());

        while !data_to_verify_or_repay.is_empty() {
            println!(
                "Verifying and potential topping up payment of {:?} chunks",
                data_to_verify_or_repay.len()
            );
            trace!(
                "Verifying and potential topping up payment of {:?} chunks",
                data_to_verify_or_repay.len()
            );
            data_to_verify_or_repay =
                verify_and_repay_if_needed(file_api.clone(), data_to_verify_or_repay, batch_size)
                    .await?;
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    println!(
        "Uploaded all chunks in {}",
        format_elapsed_time(start_time.elapsed())
    );

    // Record the uploaded files locally to be able to fetch them later
    let content = bincode::serialize(&uploaded_files)?;
    fs::create_dir_all(file_names_path.as_path())?;
    let date_time = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let file_names_path = file_names_path.join(format!("file_names_{date_time}"));
    println!("Writing {} bytes to {file_names_path:?}", content.len());
    fs::write(file_names_path, content)?;

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
) -> Vec<JoinHandle<Result<()>>> {
    let mut upload_handles = Vec::new();
    for (name, path) in chunks_paths.into_iter() {
        let file_api = file_api.clone();

        // Spawn a task for each chunk to be uploaded
        let handle = tokio::spawn(upload_chunk(file_api, (name, path), verify_store));
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
) -> Result<()> {
    let (name, path) = chunk;

    let file_api = file_api.clone();

    // as holding chunks in mem is a serious bottleneck, we only hold one chunk in mem at a time
    let permit = Some(file_api.client().get_network_concurrency_permit().await?);
    let upload_start_time = std::time::Instant::now();
    let chunk = Chunk::new(Bytes::from(fs::read(path)?));

    file_api
        .get_local_payment_and_upload_chunk(chunk, verify_store, permit)
        .await?;

    println!(
        "Uploaded chunk #{name} in {})",
        format_elapsed_time(upload_start_time.elapsed())
    );

    Ok(())
}

/// Verify if chunks exist on the network.
/// Repay if they don't.
/// Return a list of files which had to be repaid, but are not yet reverified.
async fn verify_and_repay_if_needed(
    file_api: Files,
    mut chunks_paths: Vec<(XorName, PathBuf)>,
    batch_size: usize,
) -> Result<Vec<(XorName, PathBuf)>> {
    let mut total_failed_chunks: Vec<(XorName, PathBuf)> = vec![];
    let total_chunks = chunks_paths.len();

    println!("======= verify {total_chunks} chunk, and re paying and uploading failed chunks =============");

    let now = Instant::now();
    loop {
        if chunks_paths.is_empty() {
            break;
        } else {
            let size = std::cmp::min(batch_size, chunks_paths.len());
            let batches: Vec<_> = chunks_paths.drain(..size).collect();

            // now we try and get btached chunks, keep track of any that fail
            // Iterate over each uploaded chunk
            let mut verify_handles = Vec::new();
            for (name, path) in batches.into_iter() {
                let file_api = file_api.clone();

                // Spawn a new task to fetch each chunk concurrently
                let handle = tokio::spawn(async move {
                    let chunk_address: ChunkAddress = ChunkAddress::new(name);
                    // Attempt to fetch the chunk
                    let res = file_api.client().get_chunk(chunk_address).await;

                    Ok::<_, Error>(((chunk_address, path), res.is_err()))
                });
                verify_handles.push(handle);
            }

            // Await all fetch tasks and collect the results
            let verify_results = join_all(verify_handles).await;

            let mut failed_chunks = Vec::new();
            // Check for any errors during fetch
            for result in verify_results {
                if let ((chunk_addr, path), true) = result?? {
                    println!("Failed to fetch a chunk {chunk_addr:?}. Repaying...");
                    // This needs to be NetAddr to allow for repayment
                    failed_chunks.push((chunk_addr, path));
                }
            }

            // If there were any failed chunks, we need to repay them
            if !failed_chunks.is_empty() {
                println!(
                    "Failed to fetch {} chunks, attempting to repay them",
                    failed_chunks.len()
                );

                let mut wallet = file_api.wallet()?;

                // Now we pay again or top up, depending on the new current store cost is
                wallet
                    .pay_for_storage(
                        failed_chunks
                            .iter()
                            .map(|(addr, _path)| sn_protocol::NetworkAddress::ChunkAddress(*addr)),
                        true,
                    )
                    .await?;

                // outcome here is not important as we'll verify this later
                let upload_file_api = file_api.clone();
                let ongoing_uploads = upload_chunks_in_parallel(
                    upload_file_api,
                    failed_chunks
                        .iter()
                        .cloned()
                        .map(|(addr, path)| (*addr.xorname(), path))
                        .collect(),
                    false,
                );

                // Now we've batched all payments, we can await all uploads to happen in parallel
                let upload_results = join_all(ongoing_uploads).await;

                // lets check there were no odd errors during upload
                for result in upload_results {
                    result??;
                }
                total_failed_chunks.extend(
                    failed_chunks
                        .into_iter()
                        .map(|(addr, path)| (*addr.xorname(), path))
                        .collect::<Vec<_>>(),
                );
            }
        }
    }

    let elapsed = now.elapsed();
    println!("After {elapsed:?}, verified {total_chunks:?} chunks");

    if !total_failed_chunks.is_empty() {
        println!(
            "{} failed chunks were found, repaid & re-uploaded.",
            total_failed_chunks.len()
        );
    }

    Ok(total_failed_chunks)
}

async fn download_files(file_api: &Files, root_dir: &Path) -> Result<()> {
    let docs_of_uploaded_files_path = root_dir.join("uploaded_files");
    let download_path = root_dir.join("downloaded_files");
    std::fs::create_dir_all(download_path.as_path())?;

    for entry in WalkDir::new(docs_of_uploaded_files_path)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            let index_doc_bytes = Bytes::from(fs::read(entry.path())?);
            let index_doc_name = entry.file_name();

            println!("Loading file names from index doc {index_doc_name:?}");
            let files_to_fetch: Vec<(XorName, String)> = bincode::deserialize(&index_doc_bytes)?;

            if files_to_fetch.is_empty() {
                println!("No files to download!");
            }
            for (xorname, file_name) in files_to_fetch.iter() {
                download_file(file_api, xorname, file_name, &download_path).await;
            }
        }
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
) {
    println!(
        "Downloading file {file_name:?} with address {:64x}",
        xorname
    );
    debug!("Downloading file {file_name:?}");
    let downloaded_file_path = download_path.join(file_name);
    // The downloaded file will be writen to the folder directly.
    match file_api
        .read_bytes(
            ChunkAddress::new(*xorname),
            Some(downloaded_file_path.clone()),
        )
        .await
    {
        Ok(_) => {
            debug!("Successfully got file {file_name}!");
            println!("Successfully got file {file_name}, stored at {downloaded_file_path:?}!");
        }
        Err(error) => {
            error!("Did not get file {file_name:?} from the network! {error}");
            println!("Did not get file {file_name:?} from the network! {error}")
        }
    }
}
