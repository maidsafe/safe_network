// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{helpers::download_and_extract_release, VerbosityLevel};
use color_eyre::eyre::{bail, OptionExt, Result};
use libp2p::Multiaddr;
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::NatDetectionStatus;
use std::{path::PathBuf, process::Stdio};

#[derive(Debug)]
pub struct NatDetectionOptions {
    pub force_nat_detection: bool,
    pub path: Option<PathBuf>,
    pub servers: Vec<Multiaddr>,
    pub terminate_on_private_nat: bool,
    pub url: Option<String>,
    pub version: Option<String>,
}

pub async fn run_nat_detection(
    options: &NatDetectionOptions,
    release_repo: &dyn SafeReleaseRepoActions,
    verbosity: VerbosityLevel,
) -> Result<NatDetectionStatus> {
    let nat_detection_path = if let Some(path) = options.path.clone() {
        path
    } else {
        let (nat_detection_path, _) = download_and_extract_release(
            ReleaseType::NatDetection,
            options.url.clone(),
            options.version.clone(),
            release_repo,
            verbosity,
            None,
        )
        .await?;
        nat_detection_path
    };

    if options.servers.is_empty() {
        bail!("No servers provided for NAT detection");
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("Running NAT detection. This can take a while..");
    }

    let stdout = match verbosity {
        VerbosityLevel::Minimal => Stdio::null(),
        VerbosityLevel::Normal => Stdio::inherit(),
        VerbosityLevel::Full => Stdio::inherit(),
    };

    let mut command = std::process::Command::new(nat_detection_path);
    command.arg(
        options
            .servers
            .iter()
            .map(|addr| addr.to_string())
            .collect::<Vec<String>>()
            .join(","),
    );
    // todo: clarify the different verbosity levels. Minimal actually means none. Full/Normal are not used yet.
    if verbosity == VerbosityLevel::Full {
        command.arg("-vv");
    }
    let status = command.stdout(stdout).status()?;
    match status.code().ok_or_eyre("Failed to get the exit code")? {
        10 => Ok(NatDetectionStatus::Public),
        11 => Ok(NatDetectionStatus::UPnP),
        12 => Ok(NatDetectionStatus::Private),
        code => bail!("Failed to detect NAT status, exit code: {code}"),
    }
}
