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
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use crate::{config, VerbosityLevel};

const MAX_DOWNLOAD_RETRIES: u8 = 3;

#[cfg(windows)]
pub async fn configure_winsw(dest_path: &Path, verbosity: VerbosityLevel) -> Result<()> {
    if let Ok(_) = which::which("winsw.exe") {
        return Ok(());
    }

    if !dest_path.exists() {
        if verbosity != VerbosityLevel::Minimal {
            println!("Downloading winsw.exe...");
        }

        let release_repo = <dyn SafeReleaseRepoActions>::default_config();

        let mut pb = None;
        let callback = if verbosity != VerbosityLevel::Minimal {
            let progress_bar = Arc::new(ProgressBar::new(0));
            progress_bar.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("#>-"));
            pb = Some(progress_bar.clone());
            let pb_clone = progress_bar.clone();
            let callback: Box<dyn Fn(u64, u64) + Send + Sync> =
                Box::new(move |downloaded, total| {
                    pb_clone.set_length(total);
                    pb_clone.set_position(downloaded);
                });
            callback
        } else {
            let callback: Box<dyn Fn(u64, u64) + Send + Sync> = Box::new(move |_, _| {});
            callback
        };

        let mut download_attempts = 1;
        loop {
            if download_attempts > MAX_DOWNLOAD_RETRIES {
                bail!("Failed to download WinSW after {MAX_DOWNLOAD_RETRIES} tries.");
            }
            match release_repo.download_winsw(dest_path, &callback).await {
                Ok(_) => break,
                Err(e) => {
                    if verbosity != VerbosityLevel::Minimal {
                        println!("Error downloading WinSW: {e:?}");
                        println!("Trying again {download_attempts}/{MAX_DOWNLOAD_RETRIES}");
                    }
                    download_attempts += 1;
                    if let Some(pb) = &pb {
                        pb.finish_and_clear();
                    }
                }
            }
        }

        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
    }

    std::env::set_var("WINSW_PATH", dest_path.to_string_lossy().to_string());

    Ok(())
}

#[cfg(not(windows))]
pub async fn configure_winsw(_dest_path: &Path, _verbosity: VerbosityLevel) -> Result<()> {
    Ok(())
}

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
    download_dir_path: Option<PathBuf>,
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

    let download_dir_path = if let Some(path) = download_dir_path {
        std::fs::create_dir_all(&path)?;
        path
    } else if url.is_some() {
        create_temp_dir()?
    } else {
        // The node manager path can require root access, or can only be accessed by the service
        // user, which is why we have an optional path for the whole function.
        let path = config::get_node_manager_path()?.join("downloads");
        std::fs::create_dir_all(&path)?;
        path
    };

    let mut download_attempts = 1;
    let binary_download_path = loop {
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
                Ok(archive_path) => {
                    let binary_download_path =
                        release_repo.extract_release_archive(&archive_path, &download_dir_path)?;
                    break binary_download_path;
                }
                Err(err) => {
                    if verbosity != VerbosityLevel::Minimal {
                        println!("Error downloading release: {err:?}");
                        println!("Trying again {download_attempts}/{MAX_DOWNLOAD_RETRIES}");
                    }
                    download_attempts += 1;
                    if let Some(pb) = &pb {
                        pb.finish_and_clear();
                    }
                }
            }
        } else {
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
            if archive_path.exists() {
                // try extracting it, else download it.
                match release_repo.extract_release_archive(&archive_path, &download_dir_path) {
                    Ok(binary_download_path) => {
                        if verbosity != VerbosityLevel::Minimal {
                            println!("Using cached {release_type} version {version}...");
                        }
                        break binary_download_path;
                    }
                    Err(_) => {
                        if verbosity != VerbosityLevel::Minimal {
                            println!("Cached {release_type} version {version} is corrupted. Downloading again...");
                        }
                    }
                }
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
                Ok(archive_path) => {
                    let binary_download_path =
                        release_repo.extract_release_archive(&archive_path, &download_dir_path)?;
                    break binary_download_path;
                }
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

    if verbosity != VerbosityLevel::Minimal {
        println!("Download completed: {}", &binary_download_path.display());
    }

    // Finally, obtain the version number from the binary by running `--version`. This is useful
    // when the `--url` argument is used, and in any case, ultimately the binary we obtained is the
    // source of truth.
    let bin_version = get_bin_version(&binary_download_path)?;

    Ok((binary_download_path, bin_version))
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

#[cfg(target_os = "windows")]
pub fn get_username() -> Result<String> {
    Ok(std::env::var("USERNAME")?)
}

#[cfg(not(target_os = "windows"))]
pub fn get_username() -> Result<String> {
    Ok(std::env::var("USER")?)
}

/// There is a `tempdir` crate that provides the same kind of functionality, but it was flagged for
/// a security vulnerability.
pub fn create_temp_dir() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    let unique_dir_name = uuid::Uuid::new_v4().to_string();
    let new_temp_dir = temp_dir.join(unique_dir_name);
    std::fs::create_dir_all(&new_temp_dir)?;
    Ok(new_temp_dir)
}
