// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{download_and_get_upgrade_bin_path, is_running_as_root, print_upgrade_summary};
use crate::{
    add_services::{add_faucet, config::AddFaucetServiceOptions},
    config,
    helpers::{download_and_extract_release, get_bin_version},
    ServiceManager, VerbosityLevel,
};
use color_eyre::{eyre::eyre, Result};
use colored::Colorize;
use semver::Version;
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::{
    control::{ServiceControl, ServiceController},
    FaucetService, NodeRegistry, UpgradeOptions,
};
use sn_transfers::get_faucet_data_dir;
use std::path::PathBuf;

pub async fn add(
    env_variables: Option<Vec<(String, String)>>,
    log_dir_path: Option<PathBuf>,
    peers: PeersArgs,
    src_path: Option<PathBuf>,
    url: Option<String>,
    version: Option<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The add command must run as the root user"));
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("              Add Faucet Service                 ");
        println!("=================================================");
    }

    let service_user = "safe";
    let service_manager = ServiceController {};
    service_manager.create_service_user(service_user)?;

    let service_log_dir_path =
        config::get_service_log_dir_path(ReleaseType::Faucet, log_dir_path, service_user)?;

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    let release_repo = <dyn SafeReleaseRepoActions>::default_config();

    let (faucet_src_bin_path, version) = if let Some(path) = src_path {
        let version = get_bin_version(&path)?;
        (path, version)
    } else {
        download_and_extract_release(
            ReleaseType::Faucet,
            url.clone(),
            version,
            &*release_repo,
            verbosity,
        )
        .await?
    };

    add_faucet(
        AddFaucetServiceOptions {
            bootstrap_peers: get_peers_from_args(peers).await?,
            env_variables,
            faucet_src_bin_path,
            faucet_install_bin_path: PathBuf::from("/usr/local/bin/faucet"),
            local: false,
            service_data_dir_path: get_faucet_data_dir(),
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

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if let Some(faucet) = &mut node_registry.faucet {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Start Faucet Service                ");
            println!("=================================================");
        }

        let service = FaucetService::new(faucet, Box::new(ServiceController {}));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(ServiceController {}),
            VerbosityLevel::Normal,
        );
        service_manager.start().await?;

        node_registry.save()?;
        return Ok(());
    }

    Err(eyre!("The faucet service has not been added yet"))
}

pub async fn stop(verbosity: VerbosityLevel) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The stop command must run as the root user"));
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if let Some(faucet) = &mut node_registry.faucet {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Stop Faucet Service                 ");
            println!("=================================================");
        }

        let service = FaucetService::new(faucet, Box::new(ServiceController {}));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity);
        service_manager.stop().await?;

        node_registry.save()?;

        return Ok(());
    }

    Err(eyre!("The faucet service has not been added yet"))
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
    if node_registry.faucet.is_none() {
        println!("No faucet service has been created yet. No upgrade required.");
        return Ok(());
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("           Upgrade Faucet Service                ");
        println!("=================================================");
    }

    let (upgrade_bin_path, target_version) =
        download_and_get_upgrade_bin_path(None, ReleaseType::Faucet, url, version, verbosity)
            .await?;
    let faucet = node_registry.faucet.as_mut().unwrap();

    if !force {
        let current_version = Version::parse(&faucet.version)?;
        if target_version <= current_version {
            println!(
                "{} The faucet is already at the latest version",
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
        bootstrap_peers: node_registry.bootstrap_peers.clone(),
        env_variables: env_variables.clone(),
        force,
        start_service: !do_not_start,
        target_bin_path: upgrade_bin_path.clone(),
        target_version: target_version.clone(),
    };
    let service = FaucetService::new(faucet, Box::new(ServiceController {}));
    let mut service_manager =
        ServiceManager::new(service, Box::new(ServiceController {}), verbosity);

    match service_manager.upgrade(options).await {
        Ok(upgrade_result) => {
            print_upgrade_summary(vec![("faucet".to_string(), upgrade_result)]);
            node_registry.save()?;
            Ok(())
        }
        Err(e) => Err(eyre!("Upgrade failed: {e}")),
    }
}
