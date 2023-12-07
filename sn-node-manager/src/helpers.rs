// Copyright (C) 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
use indicatif::{ProgressBar, ProgressStyle};
use sn_releases::{get_running_platform, ArchiveType, ReleaseType, SafeReleaseRepositoryInterface};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn download_and_extract_safenode(
    safenode_version: Option<String>,
    release_repo: Box<dyn SafeReleaseRepositoryInterface>,
) -> Result<(PathBuf, String)> {
    let pb = Arc::new(ProgressBar::new(0));
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("#>-"));
    let pb_clone = pb.clone();
    let callback: Box<dyn Fn(u64, u64) + Send + Sync> = Box::new(move |downloaded, total| {
        pb_clone.set_length(total);
        pb_clone.set_position(downloaded);
    });

    let version = if let Some(version) = safenode_version {
        version
    } else {
        println!("Retrieving latest version for safenode...");
        release_repo
            .get_latest_version(&ReleaseType::Safenode)
            .await?
    };

    println!("Downloading safenode version {version}...");
    let temp_dir_path = create_temp_dir()?;
    let archive_path = release_repo
        .download_release_from_s3(
            &ReleaseType::Safenode,
            &version,
            &get_running_platform()?,
            &ArchiveType::TarGz,
            &temp_dir_path,
            &callback,
        )
        .await?;
    pb.finish_with_message("Download complete");
    let safenode_download_path =
        release_repo.extract_release_archive(&archive_path, &temp_dir_path)?;

    Ok((safenode_download_path, version))
}

/// There is a `tempdir` crate that provides the same kind of functionality, but it was flagged for
/// a security vulnerability.
fn create_temp_dir() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    let unique_dir_name = uuid::Uuid::new_v4().to_string();
    let new_temp_dir = temp_dir.join(unique_dir_name);
    std::fs::create_dir_all(&new_temp_dir)?;
    Ok(new_temp_dir)
}
