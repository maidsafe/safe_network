// Copyright (C) 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod config;
mod control;
mod install;
mod node;
mod service;

use crate::config::{get_node_registry_path, get_service_data_dir_path, get_service_log_dir_path};
use crate::control::{start, status, stop};
use crate::install::{install, InstallOptions};
use crate::node::NodeRegistry;
use crate::service::{NodeServiceManager, ServiceControl};
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Help, Result};
use libp2p_identity::PeerId;
use sn_node_rpc_client::RpcClient;
use sn_releases::SafeReleaseRepositoryInterface;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
pub enum SubCmd {
    /// Install safenode as a service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "install")]
    Install {
        /// The number of service instances
        #[clap(long)]
        count: Option<u16>,
        /// Provide the path for the data directory for the installed node.
        ///
        /// This path is a prefix. Each installed node will have its own directory underneath it.
        ///
        /// If not provided, the default location is platform specific:
        ///  - Linux: /var/safenode-manager/services
        ///  - macOS: /var/safenode-manager/services
        ///  - Windows: C:\ProgramData\safenode\services
        #[clap(long, verbatim_doc_comment)]
        data_dir_path: Option<PathBuf>,
        /// Provide the path for the log directory for the installed node.
        ///
        /// This path is a prefix. Each installed node will have its own directory underneath it.
        ///
        /// If not provided, the default location is platform specific:
        ///  - Linux: /var/log/safenode
        ///  - macOS: /var/log/safenode
        ///  - Windows: C:\ProgramData\safenode\logs
        #[clap(long, verbatim_doc_comment)]
        log_dir_path: Option<PathBuf>,
        /// The user the service should run as.
        ///
        /// If the account does not exist, it will be created.
        ///
        /// On Windows this argument will have no effect.
        #[clap(long)]
        user: Option<String>,
        /// The version of safenode
        #[clap(long)]
        version: Option<String>,
    },
    /// Start an installed safenode service.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all installed services will be started.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "start")]
    Start {
        /// The peer ID of the service to start.
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to start.
        #[clap(long)]
        service_name: Option<String>,
    },
    /// Get the status of installed services.
    #[clap(name = "status")]
    Status {
        /// Set to display more details
        #[clap(long)]
        details: bool,
    },
    /// Stop an installed safenode service.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all installed services will be stopped.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "stop")]
    Stop {
        /// The peer ID of the service to stop.
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to stop.
        #[clap(long)]
        service_name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cmd::parse();
    match args.cmd {
        SubCmd::Install {
            count,
            log_dir_path,
            data_dir_path,
            user,
            version,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The install command must run as the root user"));
            }

            println!("=================================================");
            println!("              Add Safenode Services              ");
            println!("=================================================");
            println!("{} service(s) to be added", count.unwrap_or(1));

            let service_user = user.unwrap_or("safe".to_string());
            let service_manager = NodeServiceManager {};
            service_manager.create_service_user(&service_user)?;

            let service_data_dir_path = get_service_data_dir_path(data_dir_path, &service_user)?;
            let service_log_dir_path = get_service_log_dir_path(log_dir_path, &service_user)?;

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();

            install(
                InstallOptions {
                    safenode_dir_path: get_safenode_install_path()?,
                    service_data_dir_path,
                    service_log_dir_path,
                    user: service_user,
                    count,
                    version,
                },
                &mut node_registry,
                &service_manager,
                release_repo,
            )
            .await?;

            node_registry.save(&get_node_registry_path()?)?;

            Ok(())
        }
        SubCmd::Start {
            peer_id,
            service_name,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The start command must run as the root user"));
            }

            validate_peer_id_and_service_name_args(service_name.clone(), peer_id.clone())?;

            println!("=================================================");
            println!("             Start Safenode Services             ");
            println!("=================================================");

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if let Some(ref name) = service_name {
                let node = node_registry
                    .installed_nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;

                let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                start(node, &NodeServiceManager {}, &rpc_client).await?;
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(peer_id)?;
                let node = node_registry
                    .installed_nodes
                    .iter_mut()
                    .find(|x| x.peer_id == Some(peer_id))
                    .ok_or_else(|| {
                        eyre!(format!(
                            "Could not find node with peer ID '{}'",
                            peer_id.to_string()
                        ))
                    })?;

                let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                start(node, &NodeServiceManager {}, &rpc_client).await?;
            } else {
                for node in node_registry.installed_nodes.iter_mut() {
                    let rpc_client =
                        RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                    start(node, &NodeServiceManager {}, &rpc_client).await?;
                }
            }

            node_registry.save(&get_node_registry_path()?)?;

            Ok(())
        }
        SubCmd::Status { details } => {
            println!("=================================================");
            println!("                Safenode Services                ");
            println!("=================================================");

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            status(&mut node_registry, &NodeServiceManager {}, details).await?;
            node_registry.save(&get_node_registry_path()?)?;

            Ok(())
        }
        SubCmd::Stop {
            peer_id,
            service_name,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The stop command must run as the root user"));
            }

            validate_peer_id_and_service_name_args(service_name.clone(), peer_id.clone())?;

            println!("=================================================");
            println!("              Stop Safenode Services             ");
            println!("=================================================");

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if let Some(ref name) = service_name {
                let node = node_registry
                    .installed_nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;
                stop(node, &NodeServiceManager {}).await?;
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(peer_id)?;
                let node = node_registry
                    .installed_nodes
                    .iter_mut()
                    .find(|x| x.peer_id == Some(peer_id))
                    .ok_or_else(|| {
                        eyre!(format!(
                            "Could not find node with peer ID '{}'",
                            peer_id.to_string()
                        ))
                    })?;
                stop(node, &NodeServiceManager {}).await?;
            } else {
                for node in node_registry.installed_nodes.iter_mut() {
                    stop(node, &NodeServiceManager {}).await?;
                }
            }

            node_registry.save(&get_node_registry_path()?)?;

            Ok(())
        }
    }
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    users::get_effective_uid() == 0
}

#[cfg(windows)]
fn is_running_as_root() -> bool {
    // The Windows implementation for this will be much more complex.
    true
}

#[cfg(unix)]
fn get_safenode_install_path() -> Result<PathBuf> {
    Ok(PathBuf::from("/usr/local/bin"))
}

#[cfg(windows)]
fn get_safenode_install_path() -> Result<PathBuf> {
    let path = PathBuf::from("C:\\Program Files\\Maidsafe\\safenode");
    if !path.exists() {
        std::fs::create_dir_all(path.clone())?;
    }
    Ok(path)
}

fn validate_peer_id_and_service_name_args(
    service_name: Option<String>,
    peer_id: Option<String>,
) -> Result<()> {
    if service_name.is_some() && peer_id.is_some() {
        return Err(
            eyre!("The service name and peer ID are mutually exclusive").suggestion(
                "Please try again using either the peer ID or the service name, but not both.",
            ),
        );
    }
    Ok(())
}
