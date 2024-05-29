// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    config::get_node_registry_path, helpers::download_and_extract_release, VerbosityLevel,
};
use color_eyre::eyre::{bail, OptionExt, Result};
use libp2p::Multiaddr;
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::{NatDetectionStatus, NodeRegistry};
use std::{path::PathBuf, process::Stdio};

pub async fn run_nat_detection(
    servers: Vec<Multiaddr>,
    force_run: bool,
    path: Option<PathBuf>,
    url: Option<String>,
    version: Option<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;

    if !force_run {
        if let Some(status) = node_registry.nat_status {
            if verbosity != VerbosityLevel::Minimal {
                println!("NAT status has already been set as: {status:?}");
            }
            return Ok(());
        }
    }

    let nat_detection_path = if let Some(path) = path {
        path
    } else {
        let release_repo = <dyn SafeReleaseRepoActions>::default_config();

        let (nat_detection_path, _) = download_and_extract_release(
            ReleaseType::NatDetection,
            url,
            version,
            &*release_repo,
            verbosity,
            None,
        )
        .await?;
        nat_detection_path
    };

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
        servers
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
    let status = match status.code().ok_or_eyre("Failed to get the exit code")? {
        10 => NatDetectionStatus::Public,
        11 => NatDetectionStatus::UPnP,
        12 => NatDetectionStatus::Private,
        code => bail!("Failed to detect NAT status, exit code: {code}"),
    };

    if verbosity != VerbosityLevel::Minimal {
        println!("NAT status has been found to be: {status:?}");
    }

    node_registry.nat_status = Some(status);
    node_registry.save()?;

    Ok(())
}
