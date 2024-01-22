// Copyright (C) 2024 MaidSafe.net limited.
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
mod local;
mod service;

use crate::add_service::{add, AddServiceOptions};
use crate::config::*;
use crate::control::{remove, start, status, stop, upgrade, UpgradeResult};
use crate::helpers::download_and_extract_release;
use crate::local::{kill_network, run_faucet, run_network, LocalNetworkOptions};
use crate::service::{NodeServiceManager, ServiceControl};
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use libp2p_identity::PeerId;
use semver::Version;
use sn_node_rpc_client::RpcClient;
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_protocol::node_registry::{get_local_node_registry_path, NodeRegistry};
use sn_releases::{ReleaseType, SafeReleaseRepositoryInterface};
use std::path::PathBuf;
use std::str::FromStr;

const DEFAULT_NODE_COUNT: u16 = 25;

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
        /// The number of service instances.
        ///
        /// If the --first argument is used, the count has to be one, so --count and --first are
        /// mutually exclusive.
        #[clap(long, conflicts_with = "first")]
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
        /// Set this flag to launch safenode with the --local flag.
        ///
        /// This is useful for building a service-based local network.
        #[clap(long)]
        local: bool,
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
        /// Specify a port for the node to run on.
        ///
        /// If not used, a port will be selected at random.
        ///
        /// This option only applies when a single service is being added.
        #[clap(long)]
        port: Option<u16>,
        /// Specify a port for the node's RPC service to run on.
        ///
        /// If not used, a port will be selected at random.
        ///
        /// This option only applies when a single service is being added.
        #[clap(long)]
        rpc_port: Option<u16>,
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
    /// Add one or more new safenode services.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "faucet")]
    Faucet {
        /// Path to a faucet binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "version")]
        path: Option<PathBuf>,
        #[command(flatten)]
        peers: PeersArgs,
        /// The version of the faucet to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long)]
        version: Option<String>,
    },
    /// Kill the running local network.
    #[clap(name = "kill")]
    Kill {
        /// Set this flag to keep the node's data and log directories.
        #[clap(long)]
        keep_directories: bool,
    },
    /// Join an existing local network.
    ///
    /// The existing network can be managed outwith the node manager. If this is the case, use the
    /// `--peer` argument to specify an initial peer to connect to.
    ///
    /// If no `--peer` argument is supplied, the nodes will be added to the existing local network
    /// being managed by the node manager.
    #[clap(name = "join")]
    Join {
        /// The number of nodes to run.
        #[clap(long, default_value_t = DEFAULT_NODE_COUNT)]
        count: u16,
        /// Path to a faucet binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "faucet_version")]
        faucet_path: Option<PathBuf>,
        /// The version of the faucet to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long)]
        faucet_version: Option<String>,
        /// Path to a safenode binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "node_version")]
        node_path: Option<PathBuf>,
        /// The version of safenode to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long)]
        node_version: Option<String>,
        #[command(flatten)]
        peers: PeersArgs,
        /// Set to skip the network validation process
        #[clap(long)]
        skip_validation: bool,
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
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Option<String>,
        /// Set this flag to keep the node's data and log directories.
        #[clap(long)]
        keep_directories: bool,
    },
    /// Run a local network.
    ///
    /// This will run safenode processes on the current machine to form a local network. A faucet
    /// service will also run for dispensing tokens.
    ///
    /// Paths can be supplied for safenode and faucet binaries, but otherwise, the latest versions
    /// will be downloaded.
    #[clap(name = "run")]
    Run {
        /// The number of nodes to run.
        #[clap(long, default_value_t = DEFAULT_NODE_COUNT)]
        count: u16,
        /// Path to a faucet binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "faucet_version")]
        faucet_path: Option<PathBuf>,
        /// The version of the faucet to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long)]
        faucet_version: Option<String>,
        /// Path to a safenode binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "node_version")]
        node_path: Option<PathBuf>,
        /// The version of safenode to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long)]
        node_version: Option<String>,
        /// Set to skip the network validation process
        #[clap(long)]
        skip_validation: bool,
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
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Option<String>,
    },
    /// Get the status of services.
    #[clap(name = "status")]
    Status {
        /// Set this flag to display more details
        #[clap(long)]
        details: bool,
        /// Set this flag to return an error if any nodes are not running
        #[clap(long)]
        fail: bool,
        /// Set this flag to output the status as a JSON document
        #[clap(long, conflicts_with = "details")]
        json: bool,
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
        #[clap(long, conflicts_with = "peer_id")]
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
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Option<String>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cmd::parse();
    match args.cmd {
        SubCmd::Add {
            count,
            data_dir_path,
            local,
            log_dir_path,
            peers,
            port,
            rpc_port,
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
                    local,
                    genesis: peers.first,
                    count,
                    peers: get_peers_from_args(peers).await?,
                    port,
                    rpc_port,
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

            node_registry.save()?;

            Ok(())
        }
        SubCmd::Faucet {
            path,
            peers,
            version,
        } => {
            println!("=================================================");
            println!("                 Running Faucet                  ");
            println!("=================================================");

            let local_node_reg_path = &get_local_node_registry_path()?;
            let mut local_node_registry = NodeRegistry::load(local_node_reg_path)?;
            if !local_node_registry.nodes.is_empty() {
                return Err(eyre!("A local network is already running")
                    .suggestion("Use the kill command to destroy the network then try again"));
            }

            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            let faucet_path =
                get_bin_path(path, ReleaseType::Faucet, version, &*release_repo).await?;

            let peers = get_peers_from_args(peers).await?;
            run_faucet(&mut local_node_registry, faucet_path, peers[0].clone()).await?;

            local_node_registry.save()?;

            Ok(())
        }
        SubCmd::Join {
            count,
            faucet_path,
            faucet_version,
            node_path,
            node_version,
            peers,
            skip_validation: _,
        } => {
            println!("=================================================");
            println!("             Joining Local Network               ");
            println!("=================================================");

            let local_node_reg_path = &get_local_node_registry_path()?;
            let mut local_node_registry = NodeRegistry::load(local_node_reg_path)?;

            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            let faucet_path = get_bin_path(
                faucet_path,
                ReleaseType::Faucet,
                faucet_version,
                &*release_repo,
            )
            .await?;
            let node_path = get_bin_path(
                node_path,
                ReleaseType::Safenode,
                node_version,
                &*release_repo,
            )
            .await?;

            // If no peers are obtained we will attempt to join the existing local network, if one
            // is running.
            let peers = match get_peers_from_args(peers).await {
                Ok(peers) => Some(peers),
                Err(e) => match e {
                    sn_peers_acquisition::error::Error::PeersNotObtained => None,
                    _ => return Err(e.into()),
                },
            };
            let options = LocalNetworkOptions {
                faucet_bin_path: faucet_path,
                join: true,
                node_count: count,
                peers,
                safenode_bin_path: node_path,
                skip_validation: true,
            };
            run_network(&mut local_node_registry, &NodeServiceManager {}, options).await?;
            Ok(())
        }
        SubCmd::Kill { keep_directories } => {
            let local_reg_path = &get_local_node_registry_path()?;
            let local_node_registry = NodeRegistry::load(local_reg_path)?;
            if local_node_registry.nodes.is_empty() {
                println!("No local network is currently running");
            } else {
                println!("=================================================");
                println!("             Killing Local Network               ");
                println!("=================================================");
                kill_network(&local_node_registry, keep_directories)?;
                std::fs::remove_file(local_reg_path)?;
            }
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

            node_registry.save()?;

            Ok(())
        }
        SubCmd::Run {
            count,
            faucet_path,
            faucet_version,
            node_path,
            node_version,
            skip_validation: _,
        } => {
            let local_node_reg_path = &get_local_node_registry_path()?;
            let mut local_node_registry = NodeRegistry::load(local_node_reg_path)?;
            if !local_node_registry.nodes.is_empty() {
                return Err(eyre!("A local network is already running")
                    .suggestion("Use the kill command to destroy the network then try again"));
            }

            println!("=================================================");
            println!("             Launching Local Network             ");
            println!("=================================================");

            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            let faucet_path = get_bin_path(
                faucet_path,
                ReleaseType::Faucet,
                faucet_version,
                &*release_repo,
            )
            .await?;
            let node_path = get_bin_path(
                node_path,
                ReleaseType::Safenode,
                node_version,
                &*release_repo,
            )
            .await?;

            let options = LocalNetworkOptions {
                faucet_bin_path: faucet_path,
                join: false,
                node_count: count,
                peers: None,
                safenode_bin_path: node_path,
                skip_validation: true,
            };
            run_network(&mut local_node_registry, &NodeServiceManager {}, options).await?;

            local_node_registry.save()?;

            Ok(())
        }
        SubCmd::Start {
            peer_id,
            service_name,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The start command must run as the root user"));
            }

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

            node_registry.save()?;

            Ok(())
        }
        SubCmd::Status {
            details,
            fail,
            json,
        } => {
            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if !node_registry.nodes.is_empty() {
                if !json {
                    println!("=================================================");
                    println!("                Safenode Services                ");
                    println!("=================================================");
                }
                status(
                    &mut node_registry,
                    &NodeServiceManager {},
                    details,
                    json,
                    fail,
                )
                .await?;
                node_registry.save()?;
            }

            let mut local_node_registry = NodeRegistry::load(&get_local_node_registry_path()?)?;
            if !local_node_registry.nodes.is_empty() {
                if !json {
                    println!("=================================================");
                    println!("                Local Network                    ");
                    println!("=================================================");
                }
                status(
                    &mut local_node_registry,
                    &NodeServiceManager {},
                    details,
                    json,
                    fail,
                )
                .await?;
                local_node_registry.save()?;
            }

            Ok(())
        }
        SubCmd::Stop {
            peer_id,
            service_name,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The stop command must run as the root user"));
            }

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

            node_registry.save()?;

            Ok(())
        }
        SubCmd::Upgrade {
            peer_id,
            service_name,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The upgrade command must run as the root user"));
            }

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

            let (safenode_download_path, _) = download_and_extract_release(
                ReleaseType::Safenode,
                None,
                Some(latest_version.to_string()),
                &*release_repo,
            )
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

            node_registry.save()?;

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

async fn get_bin_path(
    path_option: Option<PathBuf>,
    release_type: ReleaseType,
    version: Option<String>,
    release_repo: &dyn SafeReleaseRepositoryInterface,
) -> Result<PathBuf> {
    if let Some(path) = path_option {
        Ok(path)
    } else {
        let (download_path, _) =
            download_and_extract_release(release_type, None, version, release_repo).await?;
        Ok(download_path)
    }
}
