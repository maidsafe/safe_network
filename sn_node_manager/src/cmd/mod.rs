// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod daemon;
pub mod faucet;
pub mod local;
pub mod node;

use crate::helpers::download_and_extract_release;
use color_eyre::{eyre::eyre, Result};
use colored::Colorize;
use semver::Version;
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::UpgradeResult;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

#[cfg(unix)]
pub fn is_running_as_root() -> bool {
    users::get_effective_uid() == 0
}

#[cfg(windows)]
pub fn is_running_as_root() -> bool {
    true
}

pub async fn download_and_get_upgrade_bin_path(
    release_type: ReleaseType,
    url: Option<String>,
    version: Option<String>,
) -> Result<(PathBuf, Version)> {
    let release_repo = <dyn SafeReleaseRepoActions>::default_config();
    if let Some(version) = version {
        let (upgrade_bin_path, version) =
            download_and_extract_release(release_type, None, Some(version), &*release_repo).await?;
        Ok((upgrade_bin_path, Version::parse(&version)?))
    } else if let Some(url) = url {
        let (upgrade_bin_path, version) =
            download_and_extract_release(release_type, Some(url), None, &*release_repo).await?;
        Ok((upgrade_bin_path, Version::parse(&version)?))
    } else {
        println!("Retrieving latest version of {}...", release_type);
        let latest_version = release_repo.get_latest_version(&release_type).await?;
        println!("Latest version is {latest_version}");
        let (upgrade_bin_path, _) = download_and_extract_release(
            release_type,
            None,
            Some(latest_version.to_string()),
            &*release_repo,
        )
        .await?;
        Ok((upgrade_bin_path, latest_version))
    }
}

pub fn print_upgrade_summary(upgrade_summary: Vec<(String, UpgradeResult)>) {
    println!("Upgrade summary:");
    for (service_name, upgrade_result) in upgrade_summary {
        match upgrade_result {
            UpgradeResult::NotRequired => {
                println!("- {} did not require an upgrade", service_name);
            }
            UpgradeResult::Upgraded(previous_version, new_version) => {
                println!(
                    "{} {} upgraded from {previous_version} to {new_version}",
                    "✓".green(),
                    service_name
                );
            }
            UpgradeResult::Forced(previous_version, target_version) => {
                println!(
                    "{} Forced {} version change from {previous_version} to {target_version}.",
                    "✓".green(),
                    service_name
                );
            }
            UpgradeResult::Error(msg) => {
                println!("{} {} was not upgraded: {}", "✕".red(), service_name, msg);
            }
        }
    }
}

pub async fn get_bin_path(
    build: bool,
    path: Option<PathBuf>,
    release_type: ReleaseType,
    version: Option<String>,
    release_repo: &dyn SafeReleaseRepoActions,
) -> Result<PathBuf> {
    if build {
        build_binary(&release_type)?;
        Ok(PathBuf::from("target")
            .join("release")
            .join(release_type.to_string()))
    } else if let Some(path) = path {
        Ok(path)
    } else {
        let (download_path, _) =
            download_and_extract_release(release_type, None, version, release_repo).await?;
        Ok(download_path)
    }
}

fn build_binary(bin_type: &ReleaseType) -> Result<()> {
    let mut args = vec!["build", "--release"];
    let bin_name = bin_type.to_string();
    args.push("--bin");
    args.push(&bin_name);

    // Keep features consistent to avoid recompiling.
    if cfg!(feature = "chaos") {
        println!("*** Building testnet with CHAOS enabled. Watch out. ***");
        args.push("--features");
        args.push("chaos");
    }
    if cfg!(feature = "statemap") {
        args.extend(["--features", "statemap"]);
    }
    if cfg!(feature = "otlp") {
        args.extend(["--features", "otlp"]);
    }
    if cfg!(feature = "local-discovery") {
        args.extend(["--features", "local-discovery"]);
    }
    if cfg!(feature = "network-contacts") {
        args.extend(["--features", "network-contacts"]);
    }
    if cfg!(feature = "websockets") {
        args.extend(["--features", "websockets"]);
    }
    if cfg!(feature = "open-metrics") {
        args.extend(["--features", "open-metrics"]);
    }

    let build_binary_msg = format!("Building {} binary", bin_name);
    let banner = "=".repeat(build_binary_msg.len());
    println!("{}\n{}\n{}", banner, build_binary_msg, banner);

    let mut build_result = Command::new("cargo");
    let _ = build_result.args(args.clone());

    if let Ok(val) = std::env::var("CARGO_TARGET_DIR") {
        let _ = build_result.env("CARGO_TARGET_DIR", val);
    }

    let build_result = build_result
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !build_result.status.success() {
        return Err(eyre!("Failed to build binaries"));
    }

    Ok(())
}
