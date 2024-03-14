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
        /// Specify a port for the safenode service(s).
        ///
        /// If not used, ports will be selected at random.
        ///
        /// If multiple services are being added and this argument is used, you must specify a
        /// range. For example, '12000-12004'. The length of the range must match the number of
        /// services, which in this case would be 5. The range must also go from lower to higher.
        #[clap(long, value_parser = parse_port_range)]
        port: Option<PortRange>,
        #[clap(long)]
        /// Specify an Ipv4Addr for the node's RPC server to run on.
        ///
        /// Useful if you want to expose the RPC server pubilcly. Ports are assigned automatically.
        ///
        /// If not set, the RPC server is run locally.
        rpc_address: Option<Ipv4Addr>,
        /// Provide environment variables for the safenode service.
        ///
        /// Useful to set safenode's log levels. Variables should be comma separated without
        /// spaces.
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
    #[clap(subcommand)]
    Daemon(DaemonSubCmd),
    #[clap(subcommand)]
    Faucet(FaucetSubCmd),
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
        /// Required if we want to downgrade, or for testing purposes.
        #[clap(long)]
        force: bool,
        /// The peer ID of the service to upgrade
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to upgrade
        #[clap(long, conflicts_with = "peer_id")]
        service_name: Option<String>,
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
        /// Provide a binary to upgrade to using a URL.
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

/// Manage Daemon service.
#[derive(Subcommand, Debug)]
pub enum DaemonSubCmd {
    /// Add a daemon service for issuing commands via RPC.
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
        /// Specify a port for the daemon to listen on.
        #[clap(long, default_value_t = 12500)]
        port: u16,
        /// The path of the safenodemand binary
        #[clap(long)]
        path: PathBuf,
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

/// Manage faucet services.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum FaucetSubCmd {
    /// Add a faucet service.
    ///
    /// This command must run as the root/administrative user.
    ///
    /// Windows is not supported for running a faucet.
    #[clap(name = "add")]
    Add {
        /// Provide environment variables for the faucet service.
        ///
        /// Useful for setting log levels. Each variable should be comma separated without any
        /// space.
        ///
        /// Example: --env SN_LOG=all,RUST_LOG=libp2p=debug
        #[clap(name = "env", long, use_value_delimiter = true, value_parser = parse_environment_variables)]
        env_variables: Option<Vec<(String, String)>>,
        /// Provide the path for the log directory for the faucet.
        ///
        /// If not provided, the default location /var/log/faucet.
        #[clap(long, verbatim_doc_comment)]
        log_dir_path: Option<PathBuf>,
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
        /// The version of the faucet
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
        /// Useful to set safenode's log levels. Variables should be comma separated without
        /// spaces.
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
            env_variables,
            local,
            log_dir_path,
            peers,
            port,
            rpc_address,
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
                peers,
                port,
                rpc_address,
                url,
                user,
                version,
                verbosity,
            )
            .await
        }
        SubCmd::Daemon(DaemonSubCmd::Add {
            address,
            port,
            path,
        }) => cmd::daemon::add(address, port, path, verbosity).await,
        SubCmd::Daemon(DaemonSubCmd::Start {}) => cmd::daemon::start(verbosity).await,
        SubCmd::Daemon(DaemonSubCmd::Stop {}) => cmd::daemon::stop(verbosity).await,
        SubCmd::Faucet(faucet_command) => match faucet_command {
            FaucetSubCmd::Add {
                env_variables,
                log_dir_path,
                peers,
                url,
                version,
            } => {
                cmd::faucet::add(env_variables, log_dir_path, peers, url, version, verbosity).await
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
        SubCmd::Kill { keep_directories } => cmd::local::kill(keep_directories, verbosity),
        SubCmd::Remove {
            keep_directories,
            peer_id,
            service_name,
        } => cmd::node::remove(keep_directories, peer_id, service_name, verbosity).await,
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
        SubCmd::Start {
            peer_id,
            service_name,
        } => cmd::node::start(peer_id, service_name, verbosity).await,
        SubCmd::Status {
            details,
            fail,
            json,
        } => cmd::node::status(details, fail, json).await,
        SubCmd::Stop {
            peer_id,
            service_name,
        } => cmd::node::stop(peer_id, service_name, verbosity).await,
        SubCmd::Upgrade {
            do_not_start,
            force,
            peer_id,
            service_name,
            env_variables: provided_env_variable,
            url,
            version,
        } => {
            cmd::node::upgrade(
                do_not_start,
                force,
                peer_id,
                provided_env_variable,
                service_name,
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
