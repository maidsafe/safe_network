// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use libp2p_identity::PeerId;
use semver::Version;
use sn_node_manager::{
    add_service::{add, AddServiceOptions},
    config::*,
    control::{remove, start, status, stop, upgrade, UpgradeOptions, UpgradeResult},
    helpers::download_and_extract_release,
    local::{kill_network, run_faucet, run_network, LocalNetworkOptions},
    service::{NodeServiceManager, ServiceControl},
    VerbosityLevel,
};
use sn_node_rpc_client::RpcClient;
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_protocol::node_registry::{get_local_node_registry_path, NodeRegistry};
use sn_releases::{ReleaseType, SafeReleaseRepositoryInterface};
use std::{
    net::Ipv4Addr,
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
};

const DEFAULT_NODE_COUNT: u16 = 25;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,

    #[clap(short, long, action = clap::ArgAction::Count, default_value_t = 2)]
    verbose: u8,
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
        /// Specify a port for the node to run on. If not used, a port will be selected at random.
        ///
        /// This option only applies when a single service is being added.
        #[clap(long)]
        port: Option<u16>,
        #[clap(long)]
        /// Specify an Ipv4Addr for the node's RPC service to run on. This is useful if you want to expose the
        /// RPC server outside. The ports are assigned automatically.
        ///
        /// If not set, the RPC server is run locally.
        rpc_address: Option<Ipv4Addr>,
        /// Provide environment variables for the safenode service.
        ///
        /// This is useful to set the safenode's log levels. Each variable should be comma separated without any space.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Provide a safenode binary using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This option can be used to test a safenode binary that has been built from a forked
        /// branch and uploaded somewhere. A typical use case would be for a developer who launches
        /// a testnet to test some changes they have on a fork.
        #[clap(long, conflicts_with = "version")]
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
    /// Run a faucet server for use with a local network.
    #[clap(name = "faucet")]
    Faucet {
        /// Set to build the safenode and faucet binaries.
        ///
        /// This assumes the command is being run from the root of the safe_network repository.
        #[clap(long)]
        build: bool,
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
        /// Set to build the safenode and faucet binaries.
        ///
        /// This assumes the command is being run from the root of the safe_network repository.
        #[clap(long)]
        build: bool,
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
        /// An interval applied between launching each node.
        ///
        /// Units are milliseconds.
        #[clap(long, default_value_t = 200)]
        interval: u64,
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
        /// Set to build the safenode and faucet binaries.
        ///
        /// This assumes the command is being run from the root of the safe_network repository.
        #[clap(long)]
        build: bool,
        /// Set to remove the client data directory and kill any existing local network.
        #[clap(long)]
        clean: bool,
        /// The number of nodes to run.
        #[clap(long, default_value_t = DEFAULT_NODE_COUNT)]
        count: u16,
        /// Path to a faucet binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "faucet_version", conflicts_with = "build")]
        faucet_path: Option<PathBuf>,
        /// The version of the faucet to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long, conflicts_with = "build")]
        faucet_version: Option<String>,
        /// An interval applied between launching each node.
        ///
        /// Units are milliseconds.
        #[clap(long, default_value_t = 200)]
        interval: u64,
        /// Path to a safenode binary
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "node_version", conflicts_with = "build")]
        node_path: Option<PathBuf>,
        /// The version of safenode to use.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long, conflicts_with = "build")]
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
    /// Upgrade safenode services.
    ///
    /// The running node will be stopped, its binary will be replaced, then it will be started
    /// again.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all services will be upgraded.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "upgrade")]
    Upgrade {
        /// Set this flag to upgrade the nodes without automatically starting them.
        ///
        /// Can be useful for testing scenarios.
        #[clap(long)]
        do_not_start: bool,
        /// Set this flag to force the upgrade command to replace binaries without comparing any
        /// version numbers.
        ///
        /// This may be required in a case where we want to 'downgrade' in case an upgrade caused a
        /// problem, or for testing purposes.
        #[clap(long)]
        force: bool,
        /// The peer ID of the service to upgrade
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to upgrade
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Option<String>,
        /// Provide environment variables for the safenode service. This will override the values set during the Add
        /// command.
        ///
        /// This is useful to set the safenode's log levels. Each variable should be comma separated without any space.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Provide a binary to upgrade to, using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This can be useful for testing scenarios.
        #[clap(long, conflicts_with = "version")]
        url: Option<String>,
        /// Upgrade to a specific version rather than the latest version.
        #[clap(long)]
        version: Option<String>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cmd::parse();
    let verbosity = VerbosityLevel::from(args.verbose);

    match args.cmd {
        SubCmd::Add {
            count,
            data_dir_path,
            local,
            log_dir_path,
            peers,
            port,
            rpc_address,
            env_variables,
            url,
            user,
            version,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The add command must run as the root user"));
            }

            if verbosity != VerbosityLevel::Minimal {
                println!("=================================================");
                println!("              Add Safenode Services              ");
                println!("=================================================");
                println!("{} service(s) to be added", count.unwrap_or(1));
            }

            let service_user = user.unwrap_or("safe".to_string());
            let service_manager = NodeServiceManager {};
            service_manager.create_service_user(&service_user)?;

            let service_data_dir_path = get_service_data_dir_path(data_dir_path, &service_user)?;
            let service_log_dir_path = get_service_log_dir_path(log_dir_path, &service_user)?;

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();

            let (safenode_download_path, version) = download_and_extract_release(
                ReleaseType::Safenode,
                url.clone(),
                version,
                &*release_repo,
            )
            .await?;
            let options = AddServiceOptions {
                local,
                genesis: peers.first,
                count,
                bootstrap_peers: get_peers_from_args(peers).await?,
                node_port: port,
                rpc_address,
                safenode_bin_path: safenode_download_path,
                safenode_dir_path: service_data_dir_path.clone(),
                service_data_dir_path,
                service_log_dir_path,
                url,
                user: service_user,
                version,
                env_variables,
            };

            add(options, &mut node_registry, &service_manager, verbosity).await?;

            node_registry.save()?;

            Ok(())
        }
        SubCmd::Faucet {
            build,
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
                get_bin_path(build, path, ReleaseType::Faucet, version, &*release_repo).await?;

            let peers = get_peers_from_args(peers).await?;
            run_faucet(&mut local_node_registry, faucet_path, peers[0].clone()).await?;

            local_node_registry.save()?;

            Ok(())
        }
        SubCmd::Join {
            build,
            count,
            faucet_path,
            faucet_version,
            interval,
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
                build,
                faucet_path,
                ReleaseType::Faucet,
                faucet_version,
                &*release_repo,
            )
            .await?;
            let node_path = get_bin_path(
                build,
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
                interval,
                join: true,
                node_count: count,
                peers,
                safenode_bin_path: node_path,
                skip_validation: true,
            };
            run_network(options, &mut local_node_registry, &NodeServiceManager {}).await?;
            Ok(())
        }
        SubCmd::Kill { keep_directories } => kill_local_network(verbosity, keep_directories),
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
            build,
            clean,
            count,
            faucet_path,
            faucet_version,
            interval,
            node_path,
            node_version,
            skip_validation: _,
        } => {
            // In the clean case, the node registry must be loaded *after* the existing network has
            // been killed, which clears it out.
            let local_node_reg_path = &get_local_node_registry_path()?;
            let mut local_node_registry = if clean {
                let client_data_path = dirs_next::data_dir()
                    .ok_or_else(|| eyre!("Could not obtain user's data directory"))?
                    .join("safe")
                    .join("client");
                if client_data_path.is_dir() {
                    std::fs::remove_dir_all(client_data_path)?;
                }
                kill_local_network(verbosity.clone(), false)?;
                NodeRegistry::load(local_node_reg_path)?
            } else {
                let local_node_registry = NodeRegistry::load(local_node_reg_path)?;
                if !local_node_registry.nodes.is_empty() {
                    return Err(eyre!("A local network is already running")
                        .suggestion("Use the kill command to destroy the network then try again"));
                }
                local_node_registry
            };

            if verbosity != VerbosityLevel::Minimal {
                println!("=================================================");
                println!("             Launching Local Network             ");
                println!("=================================================");
            }

            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            let faucet_path = get_bin_path(
                build,
                faucet_path,
                ReleaseType::Faucet,
                faucet_version,
                &*release_repo,
            )
            .await?;
            let node_path = get_bin_path(
                build,
                node_path,
                ReleaseType::Safenode,
                node_version,
                &*release_repo,
            )
            .await?;

            let options = LocalNetworkOptions {
                faucet_bin_path: faucet_path,
                join: false,
                interval,
                node_count: count,
                peers: None,
                safenode_bin_path: node_path,
                skip_validation: true,
            };
            run_network(options, &mut local_node_registry, &NodeServiceManager {}).await?;

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

            if verbosity != VerbosityLevel::Minimal {
                println!("=================================================");
                println!("             Start Safenode Services             ");
                println!("=================================================");
            }

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if let Some(ref name) = service_name {
                let node = node_registry
                    .nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;

                let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
                start(node, &NodeServiceManager {}, &rpc_client, verbosity).await?;
                node_registry.save()?;
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

                let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
                start(node, &NodeServiceManager {}, &rpc_client, verbosity).await?;
                node_registry.save()?;
            } else {
                let mut failed_services = Vec::new();
                let node_count = node_registry.nodes.len();
                for i in 0..node_count {
                    let rpc_client =
                        RpcClient::from_socket_addr(node_registry.nodes[i].rpc_socket_addr);
                    let result = start(
                        &mut node_registry.nodes[i],
                        &NodeServiceManager {},
                        &rpc_client,
                        verbosity.clone(),
                    )
                    .await;
                    match result {
                        Ok(()) => {
                            node_registry.save()?;
                        }
                        Err(e) => {
                            failed_services
                                .push((node_registry.nodes[i].service_name.clone(), e.to_string()));
                        }
                    }
                }

                if !failed_services.is_empty() {
                    println!("Failed to start {} service(s):", failed_services.len());
                    for failed in failed_services.iter() {
                        println!("{} {}: {}", "✕".red(), failed.0, failed.1);
                    }
                    return Err(eyre!("Failed to start one or more services").suggestion(
                        "However, any services that were successfully started will be usable.",
                    ));
                }
            }

            Ok(())
        }
        SubCmd::Status {
            details,
            fail,
            json,
        } => {
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
                return Ok(());
            }

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
            do_not_start,
            force,
            peer_id,
            service_name,
            env_variables: provided_env_variable,
            url,
            version,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The upgrade command must run as the root user"));
            }

            if verbosity != VerbosityLevel::Minimal {
                println!("=================================================");
                println!("           Upgrade Safenode Services             ");
                println!("=================================================");
            }

            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            let (upgrade_bin_path, target_version) = if let Some(version) = version {
                let (upgrade_bin_path, version) = download_and_extract_release(
                    ReleaseType::Safenode,
                    None,
                    Some(version),
                    &*release_repo,
                )
                .await?;
                (upgrade_bin_path, Version::parse(&version)?)
            } else if let Some(url) = url {
                let (upgrade_bin_path, version) = download_and_extract_release(
                    ReleaseType::Safenode,
                    Some(url),
                    None,
                    &*release_repo,
                )
                .await?;
                (upgrade_bin_path, Version::parse(&version)?)
            } else {
                println!("Retrieving latest version of safenode...");
                let latest_version = release_repo
                    .get_latest_version(&ReleaseType::Safenode)
                    .await?;
                let latest_version = Version::parse(&latest_version)?;
                println!("Latest version is {latest_version}");
                let (upgrade_bin_path, _) = download_and_extract_release(
                    ReleaseType::Safenode,
                    None,
                    Some(latest_version.to_string()),
                    &*release_repo,
                )
                .await?;
                (upgrade_bin_path, latest_version)
            };

            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if !force {
                let node_versions = node_registry
                    .nodes
                    .iter()
                    .map(|n| {
                        Version::parse(&n.version).map_err(|_| eyre!("Failed to parse Version"))
                    })
                    .collect::<Result<Vec<Version>>>()?;
                let any_nodes_need_upgraded = node_versions
                    .iter()
                    .any(|current_version| current_version < &target_version);
                if !any_nodes_need_upgraded {
                    println!("{} All nodes are at the latest version", "✓".green());
                    return Ok(());
                }
            }

            let mut upgrade_summary = Vec::new();

            if let Some(ref name) = service_name {
                let node = node_registry
                    .nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;

                let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
                // use the passed in env variable or re-use the one that we supplied during 'add()'
                let env_variables = if provided_env_variable.is_some() {
                    &provided_env_variable
                } else {
                    &node_registry.environment_variables
                };
                let options = UpgradeOptions {
                    bootstrap_peers: node_registry.bootstrap_peers.clone(),
                    env_variables: env_variables.clone(),
                    force,
                    start_node: !do_not_start,
                    target_safenode_path: upgrade_bin_path.clone(),
                    target_version: target_version.clone(),
                };

                match upgrade(options, node, &NodeServiceManager {}, &rpc_client).await {
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

                let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
                // use the passed in env variable or re-use the one that we supplied during 'add()'
                let env_variables = if provided_env_variable.is_some() {
                    &provided_env_variable
                } else {
                    &node_registry.environment_variables
                };
                let options = UpgradeOptions {
                    bootstrap_peers: node_registry.bootstrap_peers.clone(),
                    env_variables: env_variables.clone(),
                    force,
                    start_node: !do_not_start,
                    target_safenode_path: upgrade_bin_path.clone(),
                    target_version: target_version.clone(),
                };

                match upgrade(options, node, &NodeServiceManager {}, &rpc_client).await {
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
                    let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
                    // use the passed in env variable or re-use the one that we supplied during 'add()'
                    let env_variables = if provided_env_variable.is_some() {
                        &provided_env_variable
                    } else {
                        &node_registry.environment_variables
                    };
                    let options = UpgradeOptions {
                        bootstrap_peers: node_registry.bootstrap_peers.clone(),
                        env_variables: env_variables.clone(),
                        force,
                        start_node: !do_not_start,
                        target_safenode_path: upgrade_bin_path.clone(),
                        target_version: target_version.clone(),
                    };

                    match upgrade(options, node, &NodeServiceManager {}, &rpc_client).await {
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
                        println!("- {service_name} did not require an upgrade");
                    }
                    UpgradeResult::Upgraded(previous_version, new_version) => {
                        println!(
                            "{} {service_name} upgraded from {previous_version} to {new_version}",
                            "✓".green()
                        );
                    }
                    UpgradeResult::Forced(previous_version, target_version) => {
                        println!(
                            "{} Forced {service_name} version change from {previous_version} to {target_version}.",
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

async fn get_bin_path(
    build: bool,
    path: Option<PathBuf>,
    release_type: ReleaseType,
    version: Option<String>,
    release_repo: &dyn SafeReleaseRepositoryInterface,
) -> Result<PathBuf> {
    if build {
        build_binary(&release_type)?;
        Ok(PathBuf::from("target")
            .join("release")
            .join(release_type.to_string()))
    } else if let Some(path) = path {
        Ok(path)
    } else {
        let (download_path, _) =
            download_and_extract_release(release_type, None, version, release_repo).await?;
        Ok(download_path)
    }
}

fn build_binary(bin_type: &ReleaseType) -> Result<()> {
    let mut args = vec!["build", "--release"];
    let bin_name = bin_type.to_string();
    args.push("--bin");
    args.push(&bin_name);

    // Keep features consistent to avoid recompiling.
    if cfg!(feature = "chaos") {
        println!("*** Building testnet with CHAOS enabled. Watch out. ***");
        args.push("--features");
        args.push("chaos");
    }
    if cfg!(feature = "statemap") {
        args.extend(["--features", "statemap"]);
    }
    if cfg!(feature = "otlp") {
        args.extend(["--features", "otlp"]);
    }
    if cfg!(feature = "local-discovery") {
        args.extend(["--features", "local-discovery"]);
    }
    if cfg!(feature = "network-contacts") {
        args.extend(["--features", "network-contacts"]);
    }
    if cfg!(feature = "websockets") {
        args.extend(["--features", "websockets"]);
    }
    if cfg!(feature = "open-metrics") {
        args.extend(["--features", "open-metrics"]);
    }

    let build_binary_msg = format!("Building {} binary", bin_name);
    let banner = "=".repeat(build_binary_msg.len());
    println!("{}\n{}\n{}", banner, build_binary_msg, banner);

    let mut build_result = Command::new("cargo");
    let _ = build_result.args(args.clone());

    if let Ok(val) = std::env::var("CARGO_TARGET_DIR") {
        let _ = build_result.env("CARGO_TARGET_DIR", val);
    }

    let build_result = build_result
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !build_result.status.success() {
        return Err(eyre!("Failed to build binaries"));
    }

    Ok(())
}

fn kill_local_network(verbosity: VerbosityLevel, keep_directories: bool) -> Result<()> {
    let local_reg_path = &get_local_node_registry_path()?;
    let local_node_registry = NodeRegistry::load(local_reg_path)?;
    if local_node_registry.nodes.is_empty() {
        println!("No local network is currently running");
    } else {
        if verbosity != VerbosityLevel::Minimal {
            println!("=================================================");
            println!("             Killing Local Network               ");
            println!("=================================================");
        }
        kill_network(&local_node_registry, keep_directories)?;
        std::fs::remove_file(local_reg_path)?;
    }
    Ok(())
}

// Since delimiter is on, we get element of the csv and not the entire csv.
fn parse_environment_variables(env_var: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = env_var.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(eyre!(
            "Environment variable must be in the format KEY=VALUE or KEY=INNER_KEY=VALUE.\nMultiple key-value pairs can be given with a comma between them."
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}
