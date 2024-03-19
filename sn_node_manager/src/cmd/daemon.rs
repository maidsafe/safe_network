// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::is_running_as_root;
use crate::{
    add_services::{add_daemon, config::AddDaemonServiceOptions},
    config,
    helpers::{download_and_extract_release, get_bin_version},
    ServiceManager, VerbosityLevel,
};
use color_eyre::{eyre::eyre, Result};
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::{
    control::{ServiceControl, ServiceController},
    DaemonService, NodeRegistry,
};
use std::{net::Ipv4Addr, path::PathBuf};

pub async fn add(
    address: Ipv4Addr,
    env_variables: Option<Vec<(String, String)>>,
    port: u16,
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
        println!("              Add Daemon Service                 ");
        println!("=================================================");
    }

    let service_user = "safe";
    let service_manager = ServiceController {};
    service_manager.create_service_user(service_user)?;

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    let release_repo = <dyn SafeReleaseRepoActions>::default_config();

    let (daemon_src_bin_path, version) = if let Some(path) = src_path {
        let version = get_bin_version(&path)?;
        (path, version)
    } else {
        download_and_extract_release(
            ReleaseType::SafenodeManagerDaemon,
            url.clone(),
            version,
            &*release_repo,
        )
        .await?
    };

    // At the moment we don't have the option to provide a user for running the service. Since
    // `safenodemand` requires manipulation of services, the user running it must either be root or
    // have root access. For now we will just use the `root` user. The user option gets ignored on
    // Windows anyway, so there shouldn't be a cross-platform issue here.
    add_daemon(
        AddDaemonServiceOptions {
            address,
            env_variables,
            daemon_install_bin_path: config::get_daemon_install_path(),
            daemon_src_bin_path,
            port,
            user: "root".to_string(),
            version,
        },
        &mut node_registry,
        &ServiceController {},
    )?;
    Ok(())
}

pub async fn start(verbosity: VerbosityLevel) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The start command must run as the root user"));
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if let Some(daemon) = &mut node_registry.daemon {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Start Daemon Service                ");
            println!("=================================================");
        }

        let service = DaemonService::new(daemon, Box::new(ServiceController {}));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity);
        service_manager.start().await?;

        println!(
            "Endpoint: {}",
            service_manager
                .service
                .service_data
                .endpoint
                .map_or("-".to_string(), |e| e.to_string())
        );

        node_registry.save()?;
        return Ok(());
    }

    Err(eyre!("The daemon service has not been added yet"))
}

pub async fn stop(verbosity: VerbosityLevel) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The stop command must run as the root user"));
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if let Some(daemon) = &mut node_registry.daemon {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Stop Daemon Service                 ");
            println!("=================================================");
        }

        let service = DaemonService::new(daemon, Box::new(ServiceController {}));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity);
        service_manager.stop().await?;

        node_registry.save()?;

        return Ok(());
    }

    Err(eyre!("The daemon service has not been added yet"))
}
