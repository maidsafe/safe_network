// Copyright (C) 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod add_service;
mod config;
mod control;
mod helpers;
mod node;
mod service;

use crate::add_service::{add, AddServiceOptions};
use crate::config::{get_node_registry_path, get_service_data_dir_path, get_service_log_dir_path};
use crate::control::{remove, start, status, stop, upgrade, UpgradeResult};
use crate::helpers::download_and_extract_safenode;
use crate::node::NodeRegistry;
use crate::service::{NodeServiceManager, ServiceControl};
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use libp2p_identity::PeerId;
use semver::Version;
use sn_node_rpc_client::RpcClient;
use sn_peers_acquisition::{parse_peers_args, PeersArgs};
use sn_releases::{ReleaseType, SafeReleaseRepositoryInterface};
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
    /// Add one or more new safenode services.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "add")]
    Add {
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
        #[command(flatten)]
        peers: PeersArgs,
        /// Provide a safenode binary using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This option can be used to test a safenode binary that has been built from a forked
        /// branch and uploaded somewhere. A typical use case would be for a developer who launches
        /// a testnet to test some changes they have on a fork.
        #[clap(long)]
        url: Option<String>,
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
    /// Remove a safenode service.
    ///
    /// Either a peer ID or the service name must be supplied.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "remove")]
    Remove {
        /// The peer ID of the service to remove.
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to remove.
        #[clap(long)]
        service_name: Option<String>,
        /// Set this flag to keep the nodes data and log directories.
        #[clap(long)]
        keep_directories: bool,
    },
    /// Start a safenode service.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all services will be started.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "start")]
    Start {
        /// The peer ID of the service to start
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to start
        #[clap(long)]
        service_name: Option<String>,
    },
    /// Get the status of services.
    #[clap(name = "status")]
    Status {
        /// Set this flag to display more details
        #[clap(long)]
        details: bool,
    },
    /// Stop a safenode service.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all services will be stopped.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "stop")]
    Stop {
        /// The peer ID of the service to stop
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to stop
        #[clap(long)]
        service_name: Option<String>,
    },
    /// Upgrade a safenode service.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all services will be upgraded.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "upgrade")]
    Upgrade {
        /// The peer ID of the service to upgrade
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to upgrade
        #[clap(long)]
        service_name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cmd::parse();
    match args.cmd {
        SubCmd::Add {
            count,
            data_dir_path,
            log_dir_path,
            peers,
            url,
            user,
            version,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The add command must run as the root user"));
            }

            if url.is_some() && version.is_some() {
                return Err(
                    eyre!("The url and version arguments are mutually exclusive").suggestion(
                        "Please try again specifying either url or version, but not both.",
                    ),
                );
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

            add(
                AddServiceOptions {
                    count,
                    peers: parse_peers_args(peers).await.unwrap_or(vec![]),
                    safenode_dir_path: service_data_dir_path.clone(),
                    service_data_dir_path,
                    service_log_dir_path,
                    url,
                    user: service_user,
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
        SubCmd::Remove {
            peer_id,
            service_name,
            keep_directories,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The remove command must run as the root user"));
            }
            if peer_id.is_none() && service_name.is_none() {
                return Err(eyre!("Either a peer ID or a service name must be supplied"));
            }
            validate_peer_id_and_service_name_args(service_name.clone(), peer_id.clone())?;

            println!("=================================================");
            println!("           Remove Safenode Services              ");
            println!("=================================================");

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if let Some(ref name) = service_name {
                let node = node_registry
                    .nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;
                remove(node, &NodeServiceManager {}, keep_directories).await?;
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(peer_id)?;
                let node = node_registry
                    .nodes
                    .iter_mut()
                    .find(|x| x.peer_id == Some(peer_id))
                    .ok_or_else(|| {
                        eyre!(format!(
                            "Could not find node with peer ID '{}'",
                            peer_id.to_string()
                        ))
                    })?;
                remove(node, &NodeServiceManager {}, keep_directories).await?;
            }

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
                    .nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;

                let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                start(node, &NodeServiceManager {}, &rpc_client).await?;
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(peer_id)?;
                let node = node_registry
                    .nodes
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
                for node in node_registry.nodes.iter_mut() {
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
                    .nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;
                stop(node, &NodeServiceManager {}).await?;
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(peer_id)?;
                let node = node_registry
                    .nodes
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
                for node in node_registry.nodes.iter_mut() {
                    stop(node, &NodeServiceManager {}).await?;
                }
            }

            node_registry.save(&get_node_registry_path()?)?;

            Ok(())
        }
        SubCmd::Upgrade {
            peer_id,
            service_name,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The upgrade command must run as the root user"));
            }

            validate_peer_id_and_service_name_args(service_name.clone(), peer_id.clone())?;

            println!("=================================================");
            println!("           Upgrade Safenode Services             ");
            println!("=================================================");

            println!("Retrieving latest version of safenode...");
            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            let latest_version = release_repo
                .get_latest_version(&ReleaseType::Safenode)
                .await
                .map(|v| Version::parse(&v).unwrap())?;
            println!("Latest version is {latest_version}");

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            let any_nodes_need_upgraded = node_registry.nodes.iter().any(|n| {
                let current_version = Version::parse(&n.version).unwrap();
                current_version < latest_version
            });

            if !any_nodes_need_upgraded {
                println!("{} All nodes are at the latest version", "✓".green());
                return Ok(());
            }

            let (safenode_download_path, _) =
                download_and_extract_safenode(None, Some(latest_version.to_string()), release_repo)
                    .await?;

            let mut upgrade_summary = Vec::new();

            if let Some(ref name) = service_name {
                let node = node_registry
                    .nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;

                let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                let result = upgrade(
                    node,
                    &safenode_download_path,
                    &latest_version,
                    &NodeServiceManager {},
                    &rpc_client,
                )
                .await;

                match result {
                    Ok(upgrade_result) => {
                        upgrade_summary.push((node.service_name.clone(), upgrade_result));
                    }
                    Err(e) => {
                        upgrade_summary.push((
                            node.service_name.clone(),
                            UpgradeResult::Error(format!("Error: {}", e)),
                        ));
                    }
                }
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(peer_id)?;
                let node = node_registry
                    .nodes
                    .iter_mut()
                    .find(|x| x.peer_id == Some(peer_id))
                    .ok_or_else(|| {
                        eyre!(format!(
                            "Could not find node with peer ID '{}'",
                            peer_id.to_string()
                        ))
                    })?;

                let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                let result = upgrade(
                    node,
                    &safenode_download_path,
                    &latest_version,
                    &NodeServiceManager {},
                    &rpc_client,
                )
                .await;

                match result {
                    Ok(upgrade_result) => {
                        upgrade_summary.push((node.service_name.clone(), upgrade_result));
                    }
                    Err(e) => {
                        upgrade_summary.push((
                            node.service_name.clone(),
                            UpgradeResult::Error(format!("Error: {}", e)),
                        ));
                    }
                }
            } else {
                for node in node_registry.nodes.iter_mut() {
                    let rpc_client =
                        RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
                    let result = upgrade(
                        node,
                        &safenode_download_path,
                        &latest_version,
                        &NodeServiceManager {},
                        &rpc_client,
                    )
                    .await;

                    match result {
                        Ok(upgrade_result) => {
                            upgrade_summary.push((node.service_name.clone(), upgrade_result));
                        }
                        Err(e) => {
                            upgrade_summary.push((
                                node.service_name.clone(),
                                UpgradeResult::Error(format!("Error: {}", e)),
                            ));
                        }
                    }
                }
            }

            node_registry.save(&get_node_registry_path()?)?;

            println!("Upgrade summary:");
            for (service_name, upgrade_result) in upgrade_summary {
                match upgrade_result {
                    UpgradeResult::NotRequired => {
                        println!("- {service_name} was at the latest version");
                    }
                    UpgradeResult::Upgraded(previous_version, new_version) => {
                        println!(
                            "{} {service_name} upgraded from {previous_version} to {new_version}",
                            "✓".green()
                        );
                    }
                    UpgradeResult::Error(msg) => {
                        println!("{} {service_name} was not upgraded: {}", "✕".red(), msg);
                    }
                }
            }

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
