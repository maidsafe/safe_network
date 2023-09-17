// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::{ChunkedFile, BATCH_SIZE};
use bytes::Bytes;
use clap::Parser;
use color_eyre::{
    eyre::{bail, Error},
    Result,
};
use libp2p::futures::future::join_all;
use sn_client::{Client, Files};
use sn_protocol::storage::{Chunk, ChunkAddress};
use std::{
    collections::BTreeMap,
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

pub(super) async fn chunk_path(
    client: &Client,
    root_dir: &Path,
    files_path: &Path,
) -> Result<BTreeMap<XorName, ChunkedFile>> {
    trace!("Starting to chunk {files_path:?}");

    let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());

    // Get the list of Chunks addresses from the files found at 'files_path'
    let chunks_dir = std::env::temp_dir();
    let mut num_of_chunks = 0;
    let mut chunked_files = BTreeMap::new();
    for entry in WalkDir::new(files_path).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file_name = if let Some(file_name) = entry.file_name().to_str() {
                file_name.to_string()
            } else {
                println!(
                    "Skipping file {:?} as it is not valid UTF-8.",
                    entry.file_name()
                );
                continue;
            };

            let (file_addr, _size, chunks) =
                file_api.chunk_file(entry.path(), chunks_dir.as_path())?;
            num_of_chunks += chunks.len();

            chunked_files.insert(file_addr, ChunkedFile { file_name, chunks });
        }
    }

    if chunked_files.is_empty() {
        bail!("The provided path does not contain any file. Please check your path!\nExiting...");
    }

    println!("Total number of chunks to be stored: {}", num_of_chunks);

    Ok(chunked_files)
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

    // the file's xorname and name to be recorded locally
    let content_to_be_written_locally = {
        let content = chunks_to_upload
            .iter()
            .map(|(file_addr, chunked_file)| (file_addr, &chunked_file.file_name))
            .collect::<Vec<_>>();
        bincode::serialize(&content)?
    };

    let chunks_to_upload = chunks_to_upload
        .into_iter()
        .flat_map(|(_, chunk)| chunk.chunks)
        .collect::<Vec<_>>();

    let total_chunks_uploading = chunks_to_upload.len();

    let mut progress = 0;
    // Clone necessary variables for each file upload
    let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());

    let mut ongoing_uploads = Vec::new();
    for chunks_batch in chunks_to_upload.chunks(batch_size) {
        let now = Instant::now();
        progress += chunks_batch.len();

        // pay for and verify payment... if we don't verify here, chunks uploads will surely fail
        file_api
            .pay_for_chunks(chunks_batch.iter().map(|(name, _)| *name).collect(), true)
            .await?;

        // Verification will be carried out later on, if being asked to.
        // Hence no need to carry out verification within the first attempt.
        ongoing_uploads.extend(upload_chunks_in_parallel(
            file_api.clone(),
            chunks_batch.to_vec(),
            false,
        ));

        let elapsed = now.elapsed();
        println!(
            "After {elapsed:?}, uploaded {:?} chunks, current progress is {progress}/{total_chunks_uploading}. ",
            chunks_batch.len(),
        );
        info!(
            "After {elapsed:?}, uploaded {:?} chunks, current progress is {progress}/{total_chunks_uploading}. ",
            chunks_batch.len(),
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

        let mut data_to_verify_or_repay = chunks_to_upload;
        while !data_to_verify_or_repay.is_empty() {
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
    fs::create_dir_all(file_names_path.as_path())?;
    let date_time = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let file_names_path = file_names_path.join(format!("file_names_{date_time}"));
    println!(
        "Writing {} bytes to {file_names_path:?}",
        content_to_be_written_locally.len()
    );
    fs::write(file_names_path, content_to_be_written_locally)?;

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

    let upload_start_time = std::time::Instant::now();
    let chunk = Chunk::new(Bytes::from(fs::read(path)?));

    file_api
        .get_local_payment_and_upload_chunk(chunk, verify_store, None)
        .await?;

    println!(
        "Uploaded chunk #{name} in {}",
        format_elapsed_time(upload_start_time.elapsed())
    );

    Ok(())
}

/// Verify if chunks exist on the network.
/// Repay if they don't.
/// Return a list of files which had to be repaid, but are not yet reverified.
async fn verify_and_repay_if_needed(
    file_api: Files,
    chunks_paths: Vec<(XorName, PathBuf)>,
    batch_size: usize,
) -> Result<Vec<(XorName, PathBuf)>> {
    let total_chunks = chunks_paths.len();

    println!(
        "======= Verification: {total_chunks} chunks to be checked and repayed if required ============="
    );

    let now = Instant::now();
    let mut failed_chunks = Vec::new();
    for chunks_batch in chunks_paths.chunks(batch_size) {
        // now we try and get batched chunks, keep track of any that fail
        // Iterate over each uploaded chunk
        let mut verify_handles = Vec::new();
        for (name, path) in chunks_batch.iter().cloned() {
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

        // Check for any errors during fetch
        for result in verify_results {
            if let ((chunk_addr, path), true) = result?? {
                println!("Failed to fetch a chunk {chunk_addr:?}");
                // This needs to be NetAddr to allow for repayment
                failed_chunks.push((chunk_addr, path));
            }
        }
    }

    let total_failed_chunks = failed_chunks
        .iter()
        .map(|(addr, path)| (*addr.xorname(), path.clone()))
        .collect::<Vec<_>>();

    if total_failed_chunks.is_empty() {
        println!(
            "======= Verification Completed! All chunks have been paid and stored! ============="
        );
        return Ok(total_failed_chunks);
    }

    println!("======= Verification: {} chunks were not stored in the network, repaying them in batches =============", failed_chunks.len());

    // If there were any failed chunks, we need to repay them
    for failed_chunks_batch in failed_chunks.chunks(batch_size) {
        println!(
            "Failed to fetch {} chunks, attempting to repay them",
            failed_chunks_batch.len()
        );

        let mut wallet = file_api.wallet()?;

        // Now we pay again or top up, depending on the new current store cost is
        wallet
            .pay_for_storage(
                failed_chunks_batch
                    .iter()
                    .map(|(addr, _path)| sn_protocol::NetworkAddress::ChunkAddress(*addr)),
                true,
            )
            .await?;

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
        );

        // Now we've batched all payments, we can await all uploads to happen in parallel
        let upload_results = join_all(ongoing_uploads).await;

        // lets check there were no odd errors during upload
        for result in upload_results {
            result??;
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
