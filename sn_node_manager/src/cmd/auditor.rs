// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{download_and_get_upgrade_bin_path, print_upgrade_summary};
use crate::{
    add_services::{add_auditor, config::AddAuditorServiceOptions},
    config::{self, is_running_as_root},
    helpers::{download_and_extract_release, get_bin_version},
    print_banner, ServiceManager, VerbosityLevel,
};
use color_eyre::{eyre::eyre, Result};
use colored::Colorize;
use semver::Version;
use sn_peers_acquisition::PeersArgs;
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::{
    auditor::AuditorService,
    control::{ServiceControl, ServiceController},
    NodeRegistry, UpgradeOptions,
};
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub async fn add(
    beta_encryption_key: Option<String>,
    env_variables: Option<Vec<(String, String)>>,
    log_dir_path: Option<PathBuf>,
    peers_args: PeersArgs,
    src_path: Option<PathBuf>,
    url: Option<String>,
    version: Option<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        error!("The auditor add command must run as the root user");
        return Err(eyre!("The add command must run as the root user"));
    }

    if verbosity != VerbosityLevel::Minimal {
        print_banner("Add Auditor Service");
    }

    let service_user = "safe";
    let service_manager = ServiceController {};
    service_manager.create_service_user(service_user)?;

    let service_log_dir_path = config::get_service_log_dir_path(
        ReleaseType::SnAuditor,
        log_dir_path,
        Some(service_user.to_string()),
    )?;

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    let release_repo = <dyn SafeReleaseRepoActions>::default_config();

    let (auditor_src_bin_path, version) = if let Some(path) = src_path {
        let version = get_bin_version(&path)?;
        (path, version)
    } else {
        download_and_extract_release(
            ReleaseType::SnAuditor,
            url.clone(),
            version,
            &*release_repo,
            verbosity,
            None,
        )
        .await?
    };

    info!("Adding auditor service");
    add_auditor(
        AddAuditorServiceOptions {
            auditor_src_bin_path,
            auditor_install_bin_path: PathBuf::from("/usr/local/bin/auditor"),
            beta_encryption_key,
            bootstrap_peers: peers_args.get_peers().await?,
            env_variables,
            service_log_dir_path,
            user: service_user.to_string(),
            version,
        },
        &mut node_registry,
        &service_manager,
        verbosity,
    )?;

    Ok(())
}

pub async fn start(verbosity: VerbosityLevel) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The start command must run as the root user"));
    }
    info!("Starting the auditor service");

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if let Some(auditor) = &mut node_registry.auditor {
        if verbosity != VerbosityLevel::Minimal {
            print_banner("Start Auditor Service");
        }
        info!("Starting the auditor service");

        let service = AuditorService::new(auditor, Box::new(ServiceController {}));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(ServiceController {}),
            VerbosityLevel::Normal,
        );
        service_manager.start().await?;

        node_registry.save()?;
        return Ok(());
    }
    error!("The auditor service has not been added yet");
    Err(eyre!("The auditor service has not been added yet"))
}

pub async fn stop(verbosity: VerbosityLevel) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The stop command must run as the root user"));
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if let Some(auditor) = &mut node_registry.auditor {
        if verbosity != VerbosityLevel::Minimal {
            print_banner("Stop Auditor Service");
        }
        info!("Stopping the auditor service");

        let service = AuditorService::new(auditor, Box::new(ServiceController {}));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity);
        service_manager.stop().await?;

        node_registry.save()?;

        return Ok(());
    }

    error!("The auditor service has not been added yet");
    Err(eyre!("The auditor service has not been added yet"))
}

pub async fn upgrade(
    do_not_start: bool,
    force: bool,
    provided_env_variables: Option<Vec<(String, String)>>,
    url: Option<String>,
    version: Option<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The upgrade command must run as the root user"));
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if node_registry.auditor.is_none() {
        println!("No auditor service has been created yet. No upgrade required.");
        return Ok(());
    }

    if verbosity != VerbosityLevel::Minimal {
        print_banner("Upgrade Auditor Service");
    }
    info!("Upgrading the auditor service");

    let (upgrade_bin_path, target_version) =
        download_and_get_upgrade_bin_path(None, ReleaseType::SnAuditor, url, version, verbosity)
            .await?;
    let auditor = node_registry.auditor.as_mut().unwrap();
    debug!(
        "Current version {:?}, target version {target_version:?}",
        auditor.version,
    );

    if !force {
        let current_version = Version::parse(&auditor.version)?;
        if target_version <= current_version {
            info!("The auditor is already at the latest version, do nothing.");
            println!(
                "{} The auditor is already at the latest version",
                "✓".green()
            );
            return Ok(());
        }
    }

    let env_variables = if provided_env_variables.is_some() {
        &provided_env_variables
    } else {
        &node_registry.environment_variables
    };
    let options = UpgradeOptions {
        auto_restart: true,
        bootstrap_peers: node_registry.bootstrap_peers.clone(),
        env_variables: env_variables.clone(),
        force,
        start_service: !do_not_start,
        target_bin_path: upgrade_bin_path.clone(),
        target_version: target_version.clone(),
    };
    let service = AuditorService::new(auditor, Box::new(ServiceController {}));
    let mut service_manager =
        ServiceManager::new(service, Box::new(ServiceController {}), verbosity);

    match service_manager.upgrade(options).await {
        Ok(upgrade_result) => {
            info!("Upgrade the auditor service successfully");
            print_upgrade_summary(vec![("auditor".to_string(), upgrade_result)]);
            node_registry.save()?;
            Ok(())
        }
        Err(e) => {
            error!("Failed to upgrade the auditor service: {e:?}",);
            Err(eyre!("Upgrade failed: {e}"))
        }
    }
}
