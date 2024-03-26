// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk_manager;
mod download;
mod estimate;
mod iterative_uploader;
mod upload;

pub use chunk_manager::ChunkManager;
pub use download::{download_file, download_files};
pub use estimate::Estimator;
pub use iterative_uploader::IterativeUploader;
pub use upload::{FilesUploadOptions, UploadedFile, UPLOADED_FILES};

use color_eyre::Result;
use indicatif::{ProgressBar, ProgressStyle};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use sn_client::FilesApi;
use std::path::PathBuf;
use std::time::Duration;
use walkdir::DirEntry;
use xor_name::XorName;

pub async fn chunks_to_upload_with_iter(
    files_api: &FilesApi,
    chunk_manager: &mut ChunkManager,
    entries_iter: impl Iterator<Item = DirEntry>,
    read_cache: bool,
    batch_size: usize,
    make_data_public: bool,
) -> Result<Vec<(XorName, PathBuf)>> {
    chunk_manager.chunk_with_iter(entries_iter, read_cache, make_data_public)?;
    let chunks_to_upload = if chunk_manager.is_chunks_empty() {
        let chunks = chunk_manager.get_chunks();

        let failed_chunks = files_api
            .client()
            .verify_uploaded_chunks(&chunks, batch_size)
            .await?;

        chunk_manager.mark_completed(
            chunks
                .into_iter()
                .filter(|c| !failed_chunks.contains(c))
                .map(|(xor, _)| xor),
        )?;

        if failed_chunks.is_empty() {
            msg_files_already_uploaded_verified();
            if !make_data_public {
                msg_not_public_by_default();
            }
            msg_star_line();
            if chunk_manager.completed_files().is_empty() {
                msg_chk_mgr_no_verified_file_nor_re_upload();
            }
            iterative_uploader::msg_chunk_manager_upload_complete(chunk_manager.clone());

            return Ok(vec![]);
        }
        msg_unverified_chunks_reattempted(&failed_chunks.len());
        failed_chunks
    } else {
        let mut chunks = chunk_manager.get_chunks();
        let mut rng = thread_rng();
        chunks.shuffle(&mut rng);
        chunks
    };
    Ok(chunks_to_upload)
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

fn msg_files_already_uploaded_verified() {
    println!("All files were already uploaded and verified");
    println!("**************************************");
    println!("*          Uploaded Files            *");
}

fn msg_chk_mgr_no_verified_file_nor_re_upload() {
    println!("chunk_manager doesn't have any verified_files, nor any failed_chunks to re-upload.");
}

fn msg_not_public_by_default() {
    println!("*                                    *");
    println!("*  These are not public by default.  *");
    println!("*     Reupload with `-p` option      *");
    println!("*      to publish the datamaps.      *");
}

fn msg_star_line() {
    println!("**************************************");
}

fn msg_unverified_chunks_reattempted(failed_amount: &usize) {
    println!(
        "{failed_amount} chunks were uploaded in the past but failed to verify. Will attempt to upload them again..."
    );
}
