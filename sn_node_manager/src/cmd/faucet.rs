// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::is_running_as_root;
use crate::{
    add_services::{add_faucet, config::AddFaucetServiceOptions},
    config,
    helpers::download_and_extract_release,
    ServiceManager, VerbosityLevel,
};
use color_eyre::{eyre::eyre, Result};
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_releases::{ReleaseType, SafeReleaseRepositoryInterface};
use sn_service_management::{
    control::{ServiceControl, ServiceController},
    FaucetService, NodeRegistry,
};
use sn_transfers::get_faucet_data_dir;
use std::path::PathBuf;

pub async fn add(
    env_variables: Option<Vec<(String, String)>>,
    log_dir_path: Option<PathBuf>,
    peers: PeersArgs,
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
    let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();

    let (faucet_download_path, version) =
        download_and_extract_release(ReleaseType::Faucet, url.clone(), version, &*release_repo)
            .await?;

    add_faucet(
        AddFaucetServiceOptions {
            bootstrap_peers: get_peers_from_args(peers).await?,
            env_variables,
            faucet_download_bin_path: faucet_download_path,
            faucet_install_bin_path: PathBuf::from("/usr/local/bin/faucet"),
            local: false,
            service_data_dir_path: get_faucet_data_dir(),
            service_log_dir_path,
            url,
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
    if let Some(faucet) = node_registry.faucet.clone() {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Start Faucet Service                ");
            println!("=================================================");
        }

        let service = FaucetService::new(faucet.clone(), Box::new(ServiceController {}));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(ServiceController {}),
            VerbosityLevel::Normal,
        );
        service_manager.start().await?;

        node_registry.faucet = Some(service_manager.service.service_data);
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
    if let Some(faucet) = node_registry.faucet.clone() {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Stop Faucet Service                 ");
            println!("=================================================");
        }

        let service = FaucetService::new(faucet.clone(), Box::new(ServiceController {}));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity);
        service_manager.stop().await?;

        node_registry.faucet = Some(service_manager.service.service_data);
        node_registry.save()?;

        return Ok(());
    }

    Err(eyre!("The faucet service has not been added yet"))
}
