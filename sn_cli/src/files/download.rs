// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::get_progress_bar;

use std::ffi::OsString;
use std::path::Path;

use indicatif::ProgressBar;
use xor_name::XorName;

use sn_client::{
    protocol::storage::{Chunk, ChunkAddress, RetryStrategy},
    FilesApi, FilesDownload, FilesDownloadEvent,
};
use tracing::{debug, error};

pub async fn download_file(
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
