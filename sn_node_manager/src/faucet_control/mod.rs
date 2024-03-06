// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod config;
#[cfg(test)]
mod tests;

pub use config::{AddFaucetServiceOptions, InstallFaucetServiceCtxBuilder};

use crate::{config::create_owned_dir, VerbosityLevel};
use color_eyre::Result;
use colored::Colorize;
use sn_service_management::{
    control::ServiceControl, FaucetServiceData, NodeRegistry, ServiceStatus,
};

/// Install the faucet as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub fn add_faucet(
    install_options: AddFaucetServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    verbosity: VerbosityLevel,
) -> Result<()> {
    create_owned_dir(
        install_options.service_log_dir_path.clone(),
        &install_options.user,
    )?;

    std::fs::copy(
        install_options.faucet_download_bin_path.clone(),
        install_options.faucet_install_bin_path.clone(),
    )?;

    let install_ctx = InstallFaucetServiceCtxBuilder {
        bootstrap_peers: install_options.bootstrap_peers.clone(),
        env_variables: install_options.env_variables.clone(),
        faucet_path: install_options.faucet_install_bin_path.clone(),
        local: install_options.local,
        log_dir_path: install_options.service_log_dir_path.clone(),
        name: "faucet".to_string(),
        service_user: install_options.user.clone(),
    }
    .build()?;

    match service_control.install(install_ctx) {
        Ok(()) => {
            node_registry.faucet = Some(FaucetServiceData {
                faucet_path: install_options.faucet_install_bin_path.clone(),
                local: false,
                log_dir_path: install_options.service_log_dir_path.clone(),
                pid: None,
                service_name: "faucet".to_string(),
                status: ServiceStatus::Added,
                user: install_options.user.clone(),
                version: install_options.version,
            });
            println!("Faucet service added {}", "✓".green());
            if verbosity != VerbosityLevel::Minimal {
                println!(
                    "  - Bin path: {}",
                    install_options.faucet_install_bin_path.to_string_lossy()
                );
                println!(
                    "  - Data path: {}",
                    install_options.service_data_dir_path.to_string_lossy()
                );
                println!(
                    "  - Log path: {}",
                    install_options.service_log_dir_path.to_string_lossy()
                );
            }
            println!("[!] Note: the service has not been started");
            std::fs::remove_file(install_options.faucet_download_bin_path)?;
            node_registry.save()?;
            Ok(())
        }
        Err(e) => {
            println!("Failed to add faucet service: {e}");
            Err(e.into())
        }
    }
}
