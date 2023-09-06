// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::{chunk_and_pay_for_storage, ChunkedFile};
use bytes::Bytes;
use clap::Parser;
use color_eyre::{eyre::Error, Result};
use libp2p::futures::future::join_all;
use sn_client::{Client, Files};
use sn_protocol::storage::{Chunk, ChunkAddress};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use xor_name::XorName;

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Upload {
        /// The location of the files to upload.
        #[clap(name = "path", value_name = "DIRECTORY")]
        path: PathBuf,
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
        FilesCmds::Upload { path } => upload_files(path, client, root_dir, verify_store).await?,
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
) -> Result<()> {
    let start_time = std::time::Instant::now();
    debug!(
        "Uploading files from {:?}, will verify?: {verify_store}",
        files_path
    );
    // The input files_path has to be a dir
    let file_names_path = root_dir.join("uploaded_files");

    // Payment shall always be verified.
    let chunks_to_upload = chunk_and_pay_for_storage(&client, root_dir, &files_path, true).await?;

    let mut uploads = Vec::new();

    // Iterate over each file to be uploaded
    for (file_addr, ChunkedFile { file_name, chunks }) in chunks_to_upload {
        // Clone necessary variables for each file upload
        let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());

        // dont verify this first batch
        let upload = async move {
            let res = upload_chunks_in_parallel(file_api, chunks.clone(), verify_store).await;

            (file_addr, file_name, res)
        };

        // Add the upload task to the list of uploads
        uploads.push(upload);
    }

    let upload_results = join_all(uploads).await;

    println!("First chunk upload done, verifying and repaying if required...");
    let mut uploaded_data = vec![];
    for (file_addr, filename, result) in upload_results {
        uploaded_data.push((file_addr, filename, result?));
    }

    // If we are not verifying, we can skip this
    let file_api: Files = Files::new(client.clone(), root_dir.to_path_buf());
    let mut data_to_verify = Vec::new();
    let mut data_stored = Vec::new();

    for (addr, filename, chunks) in uploaded_data {
        data_to_verify.extend(chunks);
        data_stored.push((addr, filename));
    }

    if verify_store {
        while !data_to_verify.is_empty() {
            trace!(
                "verifying and repaying data of len: {:?}",
                data_to_verify.len()
            );
            data_to_verify = verify_and_repay_if_needed(file_api.clone(), data_to_verify).await?;
        }
    }

    println!(
        "Uploaded all chunks in {}",
        format_elapsed_time(start_time.elapsed())
    );

    // Write the chunks locally to be able to verify them later
    let content = bincode::serialize(&data_stored)?;
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
async fn upload_chunks_in_parallel(
    file_api: Files,
    chunks_paths: Vec<(XorName, PathBuf)>,
    verify_store: bool,
) -> Result<Vec<(XorName, PathBuf)>> {
    let mut upload_handles = Vec::new();
    let mut uploaded_chunks = Vec::new();
    for (name, path) in chunks_paths.into_iter() {
        uploaded_chunks.push((name, path.clone()));
        let file_api = file_api.clone();

        // first we upload all chunks in parallel
        let handle = tokio::spawn(async move {
            let permit = Some(file_api.client().get_network_concurrency_permit().await?);
            // as holding chunks in mem is a serious bottleneck, we only hold one chunk in mem at a time
            // and claim a second permit for the duration here to prevent too many happening at once.
            let upload_start_time = std::time::Instant::now();
            let chunk = Chunk::new(Bytes::from(fs::read(path)?));

            file_api
                .get_local_payment_and_upload_chunk(chunk, verify_store, permit)
                .await?;

            println!(
                "Uploaded chunk #{name} in {})",
                format_elapsed_time(upload_start_time.elapsed())
            );
            Ok::<(), Error>(())
        });
        upload_handles.push(handle);
    }

    let upload_results = join_all(upload_handles).await;

    // lets check there were no wild errors during upload
    for result in upload_results {
        result??;
    }

    Ok(uploaded_chunks)
}

/// Verify if chunks exist on the network.
/// Repay if they don't.
/// Return a list of files which had to be repaid, but not yet reverified.
async fn verify_and_repay_if_needed(
    file_api: Files,
    chunks_paths: Vec<(XorName, PathBuf)>,
) -> Result<Vec<(XorName, PathBuf)>> {
    let _start_time = std::time::Instant::now();

    let mut verify_handles = Vec::new();

    // now we try and get all chunks, keep track of any that fail
    // Iterate over each uploaded chunk
    for (name, path) in chunks_paths.into_iter() {
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
            println!("Failed to fetch a chunk {chunk_addr:?}");
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

        println!("=======re uploading failed chunks =============");

        upload_chunks_in_parallel(
            file_api,
            failed_chunks
                .iter()
                .cloned()
                .map(|(addr, path)| (*addr.xorname(), path))
                .collect(),
            false,
        )
        .await
    } else {
        // No more failed chunks, we are done
        Ok(vec![])
    }
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
    match file_api.read_bytes(ChunkAddress::new(*xorname)).await {
        Ok(bytes) => {
            debug!("Successfully got file {file_name}!");
            println!("Successfully got file {file_name}!");
            let file_name_path = download_path.join(file_name);
            println!("Writing {} bytes to {file_name_path:?}", bytes.len());
            if let Err(err) = fs::write(file_name_path, bytes) {
                println!("Failed to create file {file_name:?} with error {err:?}");
            }
        }
        Err(error) => {
            error!("Did not get file {file_name:?} from the network! {error}");
            println!("Did not get file {file_name:?} from the network! {error}")
        }
    }
}
