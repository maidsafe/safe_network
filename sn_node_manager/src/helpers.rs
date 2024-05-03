// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use indicatif::{ProgressBar, ProgressStyle};
use semver::Version;
use sn_releases::{get_running_platform, ArchiveType, ReleaseType, SafeReleaseRepoActions};
use std::{
    fs::create_dir_all,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};

use crate::{config, VerbosityLevel};

const MAX_DOWNLOAD_RETRIES: u8 = 3;

/// Downloads and extracts a release binary to a temporary location.
///
/// If the URL is supplied, that will be downloaded and extracted, and the binary inside the
/// archive will be used; if the version is supplied, a specific version will be downloaded and
/// used; otherwise the latest version will be downloaded and used.
pub async fn download_and_extract_release(
    release_type: ReleaseType,
    url: Option<String>,
    version: Option<String>,
    release_repo: &dyn SafeReleaseRepoActions,
    verbosity: VerbosityLevel,
) -> Result<(PathBuf, String)> {
    let mut pb = None;
    let callback = if verbosity != VerbosityLevel::Minimal {
        let progress_bar = Arc::new(ProgressBar::new(0));
        progress_bar.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"));
        pb = Some(progress_bar.clone());
        let pb_clone = progress_bar.clone();
        let callback: Box<dyn Fn(u64, u64) + Send + Sync> = Box::new(move |downloaded, total| {
            pb_clone.set_length(total);
            pb_clone.set_position(downloaded);
        });
        callback
    } else {
        let callback: Box<dyn Fn(u64, u64) + Send + Sync> = Box::new(move |_, _| {});
        callback
    };

    let mut download_dir_path = create_temp_dir()?;

    let mut download_attempts = 1;
    let archive_path = loop {
        if download_attempts > MAX_DOWNLOAD_RETRIES {
            bail!("Failed to download release after {MAX_DOWNLOAD_RETRIES} tries.");
        }

        if let Some(url) = &url {
            if verbosity != VerbosityLevel::Minimal {
                println!("Retrieving {release_type} from {url}");
            }
            match release_repo
                .download_release(url, &download_dir_path, &callback)
                .await
            {
                Ok(archive_path) => break archive_path,
                Err(err) => {
                    if verbosity != VerbosityLevel::Minimal {
                        println!("Error while downloading release. Trying again {download_attempts}/{MAX_DOWNLOAD_RETRIES}: {err:?}");
                    }
                    download_attempts += 1;
                    if let Some(pb) = &pb {
                        pb.finish_and_clear();
                    }
                }
            }
        } else {
            download_dir_path = config::get_node_manager_path()?.join("downloads");
            create_dir_all(&download_dir_path)?;
            let version = if let Some(version) = version.clone() {
                Version::parse(&version)?
            } else {
                if verbosity != VerbosityLevel::Minimal {
                    println!("Retrieving latest version for {release_type}...");
                }
                release_repo.get_latest_version(&release_type).await?
            };

            let archive_name = format!(
                "{}-{}-{}.{}",
                release_type.to_string().to_lowercase(),
                version,
                &get_running_platform()?,
                &ArchiveType::TarGz
            );
            let archive_path = download_dir_path.join(&archive_name);

            // return if the file has been downloaded already
            if archive_path.exists() {
                break archive_path;
            }

            if verbosity != VerbosityLevel::Minimal {
                println!("Downloading {release_type} version {version}...");
            }
            match release_repo
                .download_release_from_s3(
                    &release_type,
                    &version,
                    &get_running_platform()?,
                    &ArchiveType::TarGz,
                    &download_dir_path,
                    &callback,
                )
                .await
            {
                Ok(archive_path) => break archive_path,
                Err(err) => {
                    if verbosity != VerbosityLevel::Minimal {
                        println!("Error while downloading release. Trying again {download_attempts}/{MAX_DOWNLOAD_RETRIES}: {err:?}");
                    }
                    download_attempts += 1;
                    if let Some(pb) = &pb {
                        pb.finish_and_clear();
                    }
                }
            }
        };
    };
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    let safenode_download_path =
        release_repo.extract_release_archive(&archive_path, &download_dir_path)?;

    if verbosity != VerbosityLevel::Minimal {
        println!("Download completed: {}", &safenode_download_path.display());
    }

    // Finally, obtain the version number from the binary by running `--version`. This is useful
    // when the `--url` argument is used, and in any case, ultimately the binary we obtained is the
    // source of truth.
    let bin_version = get_bin_version(&safenode_download_path)?;

    Ok((safenode_download_path, bin_version))
}

pub fn get_bin_version(bin_path: &PathBuf) -> Result<String> {
    let mut cmd = Command::new(bin_path)
        .arg("--version")
        .stdout(Stdio::piped())
        .spawn()?;

    let mut output = String::new();
    cmd.stdout
        .as_mut()
        .ok_or_else(|| eyre!("Failed to capture stdout"))?
        .read_to_string(&mut output)?;

    let version = output
        .split_whitespace()
        .last()
        .ok_or_else(|| eyre!("Failed to parse version"))?
        .to_string();

    Ok(version)
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
