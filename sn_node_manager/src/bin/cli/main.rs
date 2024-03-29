// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use sn_node_manager::{
    add_services::config::{parse_port_range, PortRange},
    cmd, VerbosityLevel,
};
use sn_peers_acquisition::PeersArgs;
use std::{net::Ipv4Addr, path::PathBuf};

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
    /// Add one or more safenode services.
    ///
    /// By default, the latest safenode binary will be downloaded; however, it is possible to
    /// provide a binary either by specifying a URL, a local path, or a specific version number.
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
        /// Provide environment variables for the safenode service.
        ///
        /// Useful to set log levels. Variables should be comma separated without spaces.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
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
        /// Specify a port for the open metrics server.
        ///
        /// This argument should only be used with a safenode binary that has the open-metrics
        /// feature enabled.
        ///
        /// If not used, ports will be selected at random.
        ///
        /// If multiple services are being added and this argument is used, you must specify a
        /// range. For example, '12000-12004'. The length of the range must match the number of
        /// services, which in this case would be 5. The range must also go from lower to higher.
        #[clap(long, value_parser = parse_port_range)]
        metrics_port: Option<PortRange>,
        /// Specify a port for the safenode service(s).
        ///
        /// If not used, ports will be selected at random.
        ///
        /// If multiple services are being added and this argument is used, you must specify a
        /// range. For example, '12000-12004'. The length of the range must match the number of
        /// services, which in this case would be 5. The range must also go from lower to higher.
        #[clap(long, value_parser = parse_port_range)]
        node_port: Option<PortRange>,
        /// Provide a path for the safenode binary to be used by the service.
        ///
        /// Useful for creating the service using a custom built binary.
        #[clap(long)]
        path: Option<PathBuf>,
        #[command(flatten)]
        peers: PeersArgs,
        /// Specify an Ipv4Addr for the node's RPC server to run on.
        ///
        /// Useful if you want to expose the RPC server pubilcly. Ports are assigned automatically.
        ///
        /// If not set, the RPC server is run locally.
        #[clap(long)]
        rpc_address: Option<Ipv4Addr>,
        /// Specify a port for the RPC service(s).
        ///
        /// If not used, ports will be selected at random.
        ///
        /// If multiple services are being added and this argument is used, you must specify a
        /// range. For example, '12000-12004'. The length of the range must match the number of
        /// services, which in this case would be 5. The range must also go from lower to higher.
        #[clap(long, value_parser = parse_port_range)]
        rpc_port: Option<PortRange>,
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
        /// Provide a specific version of safenode to be installed.
        ///
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
        ///
        /// The binary will be downloaded.
        #[clap(long)]
        version: Option<String>,
    },
    /// Get node reward balances.
    #[clap(name = "balance")]
    Balance {
        /// Display the balance for a specific service using its peer ID.
        ///
        /// The argument can be used multiple times.
        #[clap(long)]
        peer_id: Vec<String>,
        /// Display the balance for a specific service using its name.
        ///
        /// The argument can be used multiple times.
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Vec<String>,
    },
    #[clap(subcommand)]
    Daemon(DaemonSubCmd),
    #[clap(subcommand)]
    Faucet(FaucetSubCmd),
    #[clap(subcommand)]
    Local(LocalSubCmd),
    /// Remove safenode service(s).
    ///
    /// Either peer ID(s) or service name(s) must be supplied.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "remove")]
    Remove {
        /// The peer ID of the service to remove.
        ///
        /// The argument can be used multiple times to remove many services.
        #[clap(long)]
        peer_id: Vec<String>,
        /// The name of the service to remove.
        ///
        /// The argument can be used multiple times to remove many services.
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Vec<String>,
        /// Set this flag to keep the node's data and log directories.
        #[clap(long)]
        keep_directories: bool,
    },
    /// Start safenode service(s).
    ///
    /// If no peer ID(s) or service name(s) are supplied, all services will be started.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "start")]
    Start {
        /// An interval applied between launching each service.
        ///
        /// Units are milliseconds.
        #[clap(long, default_value_t = 200)]
        interval: u64,
        /// The peer ID of the service to start.
        ///
        /// The argument can be used multiple times to start many services.
        #[clap(long)]
        peer_id: Vec<String>,
        /// The name of the service to start.
        ///
        /// The argument can be used multiple times to start many services.
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Vec<String>,
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
    /// Stop safenode service(s).
    ///
    /// If no peer ID(s) or service name(s) are supplied, all services will be stopped.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "stop")]
    Stop {
        /// The peer ID of the service to stop.
        ///
        /// The argument can be used multiple times to stop many services.
        #[clap(long)]
        peer_id: Vec<String>,
        /// The name of the service to stop.
        ///
        /// The argument can be used multiple times to stop many services.
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Vec<String>,
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
        /// Provide environment variables for the safenode service.
        ///
        /// Values set when the service was added will be overridden.
        ///
        /// Useful to set safenode's log levels. Variables should be comma separated without
        /// spaces.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Set this flag to force the upgrade command to replace binaries without comparing any
        /// version numbers.
        ///
        /// Required if we want to downgrade, or for testing purposes.
        #[clap(long)]
        force: bool,
        /// The peer ID of the service to upgrade
        #[clap(long)]
        peer_id: Vec<String>,
        /// The name of the service to upgrade
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Vec<String>,
        /// Provide a binary to upgrade to using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This can be useful for testing scenarios.
        #[clap(long, conflicts_with = "version")]
        url: Option<String>,
        /// Upgrade to a specific version rather than the latest version.
        ///
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
        #[clap(long)]
        version: Option<String>,
    },
}

/// Manage the RPC service.
#[derive(Subcommand, Debug)]
pub enum DaemonSubCmd {
    /// Add a daemon service for issuing commands via RPC.
    ///
    /// By default, the latest safenodemand binary will be downloaded; however, it is possible to
    /// provide a binary either by specifying a URL, a local path, or a specific version number.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "add")]
    Add {
        /// Specify an Ipv4Addr for the daemon to listen on.
        ///
        /// This is useful for managing nodes remotely.
        ///
        /// If not set, the daemon listens locally.
        #[clap(long, default_value_t = Ipv4Addr::new(127, 0, 0, 1))]
        address: Ipv4Addr,
        /// Provide environment variables for the daemon service.
        ///
        /// Useful to set log levels. Variables should be comma separated without spaces.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Specify a port for the daemon to listen on.
        #[clap(long, default_value_t = 12500)]
        port: u16,
        /// Provide a path for the daemon binary to be used by the service.
        ///
        /// Useful for creating the daemon service using a custom built binary.
        #[clap(long)]
        path: Option<PathBuf>,
        /// Provide a faucet binary using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This option can be used to test a faucet binary that has been built from a forked
        /// branch and uploaded somewhere. A typical use case would be for a developer who launches
        /// a testnet to test some changes they have on a fork.
        #[clap(long, conflicts_with = "version")]
        url: Option<String>,
        /// Provide a specific version of the daemon to be installed.
        ///
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
        ///
        /// The binary will be downloaded.
        #[clap(long)]
        version: Option<String>,
    },
    /// Start the daemon service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "start")]
    Start {},
    /// Stop the daemon service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "stop")]
    Stop {},
}

/// Manage the faucet service.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum FaucetSubCmd {
    /// Add a faucet service.
    ///
    /// By default, the latest faucet binary will be downloaded; however, it is possible to provide
    /// a binary either by specifying a URL, a local path, or a specific version number.
    ///
    /// This command must run as the root/administrative user.
    ///
    /// Windows is not supported for running a faucet.
    #[clap(name = "add")]
    Add {
        /// Provide environment variables for the faucet service.
        ///
        /// Useful to set log levels. Variables should be comma separated without spaces.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Provide the path for the log directory for the faucet.
        ///
        /// If not provided, the default location /var/log/faucet.
        #[clap(long, verbatim_doc_comment)]
        log_dir_path: Option<PathBuf>,
        /// Provide a path for the faucet binary to be used by the service.
        ///
        /// Useful for creating the faucet service using a custom built binary.
        #[clap(long)]
        path: Option<PathBuf>,
        #[command(flatten)]
        peers: PeersArgs,
        /// Provide a faucet binary using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This option can be used to test a faucet binary that has been built from a forked
        /// branch and uploaded somewhere. A typical use case would be for a developer who launches
        /// a testnet to test some changes they have on a fork.
        #[clap(long, conflicts_with = "version")]
        url: Option<String>,
        /// Provide a specific version of the faucet to be installed.
        ///
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
        ///
        /// The binary will be downloaded.
        #[clap(long)]
        version: Option<String>,
    },
    /// Start the faucet service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "start")]
    Start {},
    /// Stop the faucet service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "stop")]
    Stop {},
    /// Upgrade the faucet.
    ///
    /// The running faucet will be stopped, its binary will be replaced, then it will be started
    /// again.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "upgrade")]
    Upgrade {
        /// Set this flag to upgrade the faucet without starting it.
        ///
        /// Can be useful for testing scenarios.
        #[clap(long)]
        do_not_start: bool,
        /// Set this flag to force the upgrade command to replace binaries without comparing any
        /// version numbers.
        ///
        /// Required if we want to downgrade, or for testing purposes.
        #[clap(long)]
        force: bool,
        /// Provide environment variables for the faucet service.
        ///
        /// Values set when the service was added will be overridden.
        ///
        /// Useful to set log levels. Variables should be comma separated without spaces.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Provide a binary to upgrade to using a URL.
        ///
        /// The binary must be inside a zip or gzipped tar archive.
        ///
        /// This can be useful for testing scenarios.
        #[clap(long, conflicts_with = "version")]
        url: Option<String>,
        /// Upgrade to a specific version rather than the latest version.
        ///
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
        #[clap(long)]
        version: Option<String>,
    },
}

/// Manage local networks.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum LocalSubCmd {
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
        /// This option requires the command run from the root of the safe_network repository.
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
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
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
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
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
        /// This option requires the command run from the root of the safe_network repository.
        #[clap(long)]
        build: bool,
        /// Set to remove the client data directory and kill any existing local network.
        #[clap(long)]
        clean: bool,
        /// The number of nodes to run.
        #[clap(long, default_value_t = DEFAULT_NODE_COUNT)]
        count: u16,
        /// Path to a faucet binary.
        ///
        /// The path and version arguments are mutually exclusive.
        #[clap(long, conflicts_with = "faucet_version", conflicts_with = "build")]
        faucet_path: Option<PathBuf>,
        /// The version of the faucet to use.
        ///
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
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
        /// The version number should be in the form X.Y.Z, with no 'v' prefix.
        ///
        /// The version and path arguments are mutually exclusive.
        #[clap(long, conflicts_with = "build")]
        node_version: Option<String>,
        /// Set to skip the network validation process
        #[clap(long)]
        skip_validation: bool,
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
            env_variables,
            local,
            log_dir_path,
            metrics_port,
            node_port,
            path,
            peers,
            rpc_address,
            rpc_port,
            url,
            user,
            version,
        } => {
            cmd::node::add(
                count,
                data_dir_path,
                env_variables,
                local,
                log_dir_path,
                metrics_port,
                node_port,
                peers,
                rpc_address,
                rpc_port,
                path,
                url,
                user,
                version,
                verbosity,
            )
            .await
        }
        SubCmd::Balance {
            peer_id: peer_ids,
            service_name: service_names,
        } => cmd::node::balance(peer_ids, service_names, verbosity).await,
        SubCmd::Daemon(DaemonSubCmd::Add {
            address,
            env_variables,
            port,
            path,
            url,
            version,
        }) => cmd::daemon::add(address, env_variables, port, path, url, version, verbosity).await,
        SubCmd::Daemon(DaemonSubCmd::Start {}) => cmd::daemon::start(verbosity).await,
        SubCmd::Daemon(DaemonSubCmd::Stop {}) => cmd::daemon::stop(verbosity).await,
        SubCmd::Faucet(faucet_command) => match faucet_command {
            FaucetSubCmd::Add {
                env_variables,
                log_dir_path,
                path,
                peers,
                url,
                version,
            } => {
                cmd::faucet::add(
                    env_variables,
                    log_dir_path,
                    peers,
                    path,
                    url,
                    version,
                    verbosity,
                )
                .await
            }
            FaucetSubCmd::Start {} => cmd::faucet::start(verbosity).await,
            FaucetSubCmd::Stop {} => cmd::faucet::stop(verbosity).await,
            FaucetSubCmd::Upgrade {
                do_not_start,
                force,
                env_variables: provided_env_variable,
                url,
                version,
            } => {
                cmd::faucet::upgrade(
                    do_not_start,
                    force,
                    provided_env_variable,
                    url,
                    version,
                    verbosity,
                )
                .await
            }
        },
        SubCmd::Local(local_command) => match local_command {
            LocalSubCmd::Join {
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
                cmd::local::join(
                    build,
                    count,
                    faucet_path,
                    faucet_version,
                    interval,
                    node_path,
                    node_version,
                    peers,
                    true,
                )
                .await
            }
            LocalSubCmd::Kill { keep_directories } => cmd::local::kill(keep_directories, verbosity),
            LocalSubCmd::Run {
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
                cmd::local::run(
                    build,
                    clean,
                    count,
                    faucet_path,
                    faucet_version,
                    interval,
                    node_path,
                    node_version,
                    true,
                    verbosity,
                )
                .await
            }
        },
        SubCmd::Remove {
            keep_directories,
            peer_id: peer_ids,
            service_name: service_names,
        } => cmd::node::remove(keep_directories, peer_ids, service_names, verbosity).await,
        SubCmd::Start {
            interval,
            peer_id: peer_ids,
            service_name: service_names,
        } => cmd::node::start(interval, peer_ids, service_names, verbosity).await,
        SubCmd::Status {
            details,
            fail,
            json,
        } => cmd::node::status(details, fail, json).await,
        SubCmd::Stop {
            peer_id: peer_ids,
            service_name: service_names,
        } => cmd::node::stop(peer_ids, service_names, verbosity).await,
        SubCmd::Upgrade {
            do_not_start,
            force,
            peer_id: peer_ids,
            service_name: service_names,
            env_variables: provided_env_variable,
            url,
            version,
        } => {
            cmd::node::upgrade(
                do_not_start,
                force,
                peer_ids,
                provided_env_variable,
                service_names,
                url,
                version,
                verbosity,
            )
            .await
        }
    }
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
