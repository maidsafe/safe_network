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
    helpers::get_bin_version,
    ServiceManager, VerbosityLevel,
};
use color_eyre::{eyre::eyre, Result};
use sn_service_management::{control::ServiceController, DaemonService, NodeRegistry};
use std::{net::Ipv4Addr, path::PathBuf};

pub async fn add(
    address: Ipv4Addr,
    port: u16,
    path: PathBuf,
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

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    add_daemon(
        AddDaemonServiceOptions {
            address,
            port,
            daemon_download_bin_path: path.clone(),
            // TODO: make this cross platform
            daemon_install_bin_path: PathBuf::from("/usr/local/bin/safenodemand"),
            version: get_bin_version(&path)?,
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
    if let Some(daemon) = node_registry.daemon.clone() {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Start Daemon Service                ");
            println!("=================================================");
        }

        let service = DaemonService::new(daemon.clone(), Box::new(ServiceController {}));
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

        node_registry.daemon = Some(service_manager.service.service_data);
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
    if let Some(daemon) = node_registry.daemon.clone() {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Stop Daemon Service                 ");
            println!("=================================================");
        }

        let service = DaemonService::new(daemon.clone(), Box::new(ServiceController {}));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity);
        service_manager.stop().await?;

        node_registry.daemon = Some(service_manager.service.service_data);
        node_registry.save()?;

        return Ok(());
    }

    Err(eyre!("The daemon service has not been added yet"))
}
