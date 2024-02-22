// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk_manager;
mod estimate;
pub(crate) mod upload;

pub(crate) use chunk_manager::ChunkManager;

use clap::Parser;
use color_eyre::{eyre::eyre, Help, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sn_client::{Client, FilesApi, FilesDownload, FilesDownloadEvent, BATCH_SIZE};
use sn_protocol::storage::{Chunk, ChunkAddress, RetryStrategy};
use std::time::Duration;
use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
};
use upload::{FilesUploadOptions, UploadedFile, UPLOADED_FILES};
use walkdir::WalkDir;
use xor_name::XorName;

/// The default folder to download files to.
const DOWNLOAD_FOLDER: &str = "safe_files";

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Estimate {
        /// The location of the file(s) to upload. Can be a file or a directory.
        #[clap(name = "path", value_name = "PATH")]
        path: PathBuf,
        /// Should the file be made accessible to all. (This is irreversible)
        #[clap(long, name = "make_public", default_value = "false", short = 'p')]
        make_data_public: bool,
    },
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
        /// Should the file be made accessible to all. (This is irreversible)
        #[clap(long, name = "make_public", default_value = "false", short = 'p')]
        make_data_public: bool,
        /// Set the strategy to use on chunk upload failure. Does not modify the spend failure retry attempts yet.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Balanced, short = 'r', help = "Sets the retry strategy on upload failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
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
        #[clap(long, default_value_t = BATCH_SIZE , short='b')]
        batch_size: usize,
        /// Set the strategy to use on downloads failure.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Quick, short = 'r', help = "Sets the retry strategy on download failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
}

pub(crate) async fn files_cmds(
    cmds: FilesCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        FilesCmds::Estimate {
            path,
            make_data_public,
        } => estimate::estimate_cost(path, make_data_public, client, root_dir).await?,
        FilesCmds::Upload {
            path,
            batch_size,
            retry_strategy,
            make_data_public,
        } => {
            upload::upload_files(
                path,
                client,
                root_dir.to_path_buf(),
                FilesUploadOptions {
                    make_data_public,
                    verify_store,
                    batch_size,
                    retry_strategy,
                },
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
                    progress_bar = get_progress_bar(count as u64).map_err(|err|{
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
                    progress_bar = get_progress_bar(count as u64).map_err(|err|{
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

pub fn get_progress_bar(length: u64) -> Result<ProgressBar> {
    let progress_bar = ProgressBar::new(length);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")?
            .progress_chars("#>-"),
    );
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    Ok(progress_bar)
}
