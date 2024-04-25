// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod rpc_service;

use clap::Parser;
use eyre::{eyre, Result};
use libp2p::{identity::Keypair, PeerId};
#[cfg(feature = "metrics")]
use sn_logging::metrics::init_metrics;
use sn_logging::{Level, LogFormat, LogOutputDest, ReloadHandle};
use sn_node::{Marker, NodeBuilder, NodeEvent, NodeEventsReceiver};
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_protocol::{node::get_safenode_root_dir, node_rpc::NodeCtrl};
use std::{
    env,
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::{
    runtime::Runtime,
    sync::{broadcast::error::RecvError, mpsc},
    time::sleep,
};
use tracing_appender::non_blocking::WorkerGuard;

#[derive(Debug, Clone)]
pub enum LogOutputDestArg {
    Stdout,
    DataDir,
    Path(PathBuf),
}

impl std::fmt::Display for LogOutputDestArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogOutputDestArg::Stdout => write!(f, "stdout"),
            LogOutputDestArg::DataDir => write!(f, "data-dir"),
            LogOutputDestArg::Path(path) => write!(f, "{}", path.display()),
        }
    }
}

pub fn parse_log_output(val: &str) -> Result<LogOutputDestArg> {
    match val {
        "stdout" => Ok(LogOutputDestArg::Stdout),
        "data-dir" => Ok(LogOutputDestArg::DataDir),
        // The path should be a directory, but we can't use something like `is_dir` to check
        // because the path doesn't need to exist. We can create it for the user.
        value => Ok(LogOutputDestArg::Path(PathBuf::from(value))),
    }
}

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser, Debug)]
#[clap(name = "safenode cli", version = env!("CARGO_PKG_VERSION"))]
struct Opt {
    /// Specify whether the node is operating from a home network and situated behind a NAT without port forwarding
    /// capabilities. Setting this to true, activates hole-punching to facilitate direct connections from other nodes.
    ///
    /// If this not enabled and you're behind a NAT, the node is terminated.
    #[clap(long, default_value_t = false)]
    home_network: bool,

    /// Specify the logging output destination.
    ///
    /// Valid values are "stdout", "data-dir", or a custom path.
    ///
    /// `data-dir` is the default value.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/node/<peer-id>/logs
    ///  - macOS: $HOME/Library/Application Support/safe/node/<peer-id>/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\node\<peer-id>\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, default_value_t = LogOutputDestArg::DataDir, value_parser = parse_log_output, verbatim_doc_comment)]
    log_output_dest: LogOutputDestArg,

    /// Specify the logging format.
    ///
    /// Valid values are "default" or "json".
    ///
    /// If the argument is not used, the default format will be applied.
    #[clap(long, value_parser = LogFormat::parse_from_str, verbatim_doc_comment)]
    log_format: Option<LogFormat>,

    /// Specify the maximum number of uncompressed log files to store.
    ///
    /// This argument is ignored if `log_output_dest` is set to "stdout"
    ///
    /// After reaching this limit, the older files are archived to save space.
    /// You can also specify the maximum number of archived log files to keep.
    #[clap(long = "max_log_files", verbatim_doc_comment)]
    max_uncompressed_log_files: Option<usize>,

    /// Specify the maximum number of archived log files to store.
    ///
    /// This argument is ignored if `log_output_dest` is set to "stdout"
    ///
    /// After reaching this limit, the older archived files are deleted.
    #[clap(long = "max_archived_log_files", verbatim_doc_comment)]
    max_compressed_log_files: Option<usize>,

    /// Specify the node's data directory.
    ///
    /// If not provided, the default location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/node/<peer-id>
    ///  - macOS: $HOME/Library/Application Support/safe/node/<peer-id>
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\node\<peer-id>
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, verbatim_doc_comment)]
    root_dir: Option<PathBuf>,

    /// Specify the port to listen on.
    ///
    /// The special value `0` will cause the OS to assign a random port.
    #[clap(long, default_value_t = 0)]
    port: u16,

    /// Specify the IP to listen on.
    ///
    /// The special value `0.0.0.0` binds to all network interfaces available.
    #[clap(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    ip: IpAddr,

    #[command(flatten)]
    peers: PeersArgs,

    /// Enable the admin/control RPC service by providing an IP and port for it to listen on.
    ///
    /// The RPC service can be used for querying information about the running node.
    #[clap(long)]
    rpc: Option<SocketAddr>,

    /// Run the node in local mode.
    ///
    /// When this flag is set, we will not filter out local addresses that we observe.
    #[clap(long)]
    local: bool,

    /// Specify the owner(readable discord user name).
    #[clap(long)]
    owner: String,

    #[cfg(feature = "open-metrics")]
    /// Specify the port for the OpenMetrics server.
    ///
    /// If not specified, a port will be selected at random.
    #[clap(long, default_value_t = 0)]
    metrics_server_port: u16,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let opt = Opt::parse();

    let node_socket_addr = SocketAddr::new(opt.ip, opt.port);
    let (root_dir, keypair) = get_root_dir_and_keypair(&opt.root_dir)?;

    let (log_output_dest, log_reload_handle, _log_appender_guard) =
        init_logging(&opt, keypair.public().to_peer_id())?;

    let rt = Runtime::new()?;
    let bootstrap_peers = rt.block_on(get_peers_from_args(opt.peers))?;
    let msg = format!(
        "Running {} v{}",
        env!("CARGO_BIN_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    info!("\n{}\n{}", msg, "=".repeat(msg.len()));
    debug!(
        "safenode built with git version: {}",
        sn_build_info::git_info()
    );

    info!("Node started with initial_peers {bootstrap_peers:?}");

    // Create a tokio runtime per `run_node` attempt, this ensures
    // any spawned tasks are closed before we would attempt to run
    // another process with these args.
    #[cfg(feature = "metrics")]
    rt.spawn(init_metrics(std::process::id()));
    let restart_options = rt.block_on(async move {
        let mut node_builder = NodeBuilder::new(
            keypair,
            node_socket_addr,
            bootstrap_peers,
            opt.local,
            root_dir,
            opt.owner.clone(),
        );
        node_builder.is_behind_home_network = opt.home_network;
        #[cfg(feature = "open-metrics")]
        let mut node_builder = node_builder;
        #[cfg(feature = "open-metrics")]
        node_builder.metrics_server_port(opt.metrics_server_port);
        let restart_options =
            run_node(node_builder, opt.rpc, &log_output_dest, log_reload_handle).await?;

        Ok::<_, eyre::Report>(restart_options)
    })?;

    // actively shut down the runtime
    rt.shutdown_timeout(Duration::from_secs(2));

    // we got this far without error, which means (so far) the only thing we should be doing
    // is restarting the node
    start_new_node_process(restart_options);

    // Command was successful, so we shut down the process
    println!("A new node process has been started successfully.");
    Ok(())
}

/// Start a node with the given configuration.
/// This function will only return if it receives a Restart NodeCtrl cmd. It optionally contains the node's root dir
/// and it's listening port if we want to retain_peer_id on restart.
async fn run_node(
    node_builder: NodeBuilder,
    rpc: Option<SocketAddr>,
    log_output_dest: &str,
    log_reload_handle: ReloadHandle,
) -> Result<Option<(PathBuf, u16)>> {
    let started_instant = std::time::Instant::now();

    info!("Starting node ...");
    let running_node = node_builder.build_and_run()?;

    println!(
        "
Node started

PeerId is {}
You can check your reward balance by running:
`safe wallet balance --peer-id={}`
    ",
        running_node.peer_id(),
        running_node.peer_id()
    );

    // write the PID to the root dir
    let pid = std::process::id();
    let pid_file = running_node.root_dir_path().join("safenode.pid");
    std::fs::write(pid_file, pid.to_string().as_bytes())?;

    // Channel to receive node ctrl cmds from RPC service (if enabled), and events monitoring task
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<NodeCtrl>(5);

    // Monitor `NodeEvents`
    let node_events_rx = running_node.node_events_channel().subscribe();
    monitor_node_events(node_events_rx, ctrl_tx.clone());

    // Monitor ctrl-c
    let ctrl_tx_clone = ctrl_tx.clone();
    tokio::spawn(async move {
        if let Err(err) = tokio::signal::ctrl_c().await {
            // I/O error, ignore/print the error, but continue to handle as if ctrl-c was received
            warn!("Listening to ctrl-c error: {err}");
        }
        if let Err(err) = ctrl_tx_clone
            .send(NodeCtrl::Stop {
                delay: Duration::from_secs(1),
                cause: eyre!("Ctrl-C received!"),
            })
            .await
        {
            error!("Failed to send node control msg to safenode bin main thread: {err}");
        }
    });

    // Start up gRPC interface if enabled by user
    if let Some(addr) = rpc {
        rpc_service::start_rpc_service(
            addr,
            log_output_dest,
            running_node.clone(),
            ctrl_tx,
            started_instant,
            log_reload_handle,
        );
    }

    // Keep the node and gRPC service (if enabled) running.
    // We'll monitor any NodeCtrl cmd to restart/stop/update,
    loop {
        match ctrl_rx.recv().await {
            Some(NodeCtrl::Restart {
                delay,
                retain_peer_id,
            }) => {
                let res = if retain_peer_id {
                    let root_dir = running_node.root_dir_path();
                    let node_port = running_node.get_node_listening_port().await?;
                    Some((root_dir, node_port))
                } else {
                    None
                };
                let msg = format!("Node is restarting in {delay:?}...");
                info!("{msg}");
                println!("{msg} Node path: {log_output_dest}");
                sleep(delay).await;

                break Ok(res);
            }
            Some(NodeCtrl::Stop { delay, cause }) => {
                let msg = format!("Node is stopping in {delay:?}...");
                info!("{msg}");
                println!("{msg} Node log path: {log_output_dest}");
                sleep(delay).await;
                return Err(cause);
            }
            Some(NodeCtrl::Update(_delay)) => {
                // TODO: implement self-update once safenode app releases are published again
                println!("No self-update supported yet.");
            }
            None => {
                info!("Internal node ctrl cmds channel has been closed, restarting node");
                break Err(eyre!("Internal node ctrl cmds channel has been closed"));
            }
        }
    }
}

fn monitor_node_events(mut node_events_rx: NodeEventsReceiver, ctrl_tx: mpsc::Sender<NodeCtrl>) {
    let _handle = tokio::spawn(async move {
        loop {
            match node_events_rx.recv().await {
                Ok(NodeEvent::ConnectedToNetwork) => Marker::NodeConnectedToNetwork.log(),
                Ok(NodeEvent::ChannelClosed) | Err(RecvError::Closed) => {
                    if let Err(err) = ctrl_tx
                        .send(NodeCtrl::Stop {
                            delay: Duration::from_secs(1),
                            cause: eyre!("Node events channel closed!"),
                        })
                        .await
                    {
                        error!(
                            "Failed to send node control msg to safenode bin main thread: {err}"
                        );
                        break;
                    }
                }
                Ok(NodeEvent::TerminateNode(reason)) => {
                    if let Err(err) = ctrl_tx
                        .send(NodeCtrl::Stop {
                            delay: Duration::from_secs(1),
                            cause: eyre!("Node terminated due to: {reason:?}"),
                        })
                        .await
                    {
                        error!(
                            "Failed to send node control msg to safenode bin main thread: {err}"
                        );
                        break;
                    }
                }
                Ok(event) => {
                    /* we ignore other events */
                    trace!("Currently ignored node event {event:?}");
                }
                Err(RecvError::Lagged(n)) => {
                    warn!("Skipped {n} node events!");
                    continue;
                }
            }
        }
    });
}

fn init_logging(opt: &Opt, peer_id: PeerId) -> Result<(String, ReloadHandle, Option<WorkerGuard>)> {
    let logging_targets = vec![
        ("sn_networking".to_string(), Level::INFO),
        ("safenode".to_string(), Level::DEBUG),
        ("sn_build_info".to_string(), Level::DEBUG),
        ("sn_logging".to_string(), Level::DEBUG),
        ("sn_node".to_string(), Level::DEBUG),
        ("sn_peers_acquisition".to_string(), Level::DEBUG),
        ("sn_protocol".to_string(), Level::DEBUG),
        ("sn_registers".to_string(), Level::DEBUG),
        ("sn_transfers".to_string(), Level::DEBUG),
    ];

    let output_dest = match &opt.log_output_dest {
        LogOutputDestArg::Stdout => LogOutputDest::Stdout,
        LogOutputDestArg::DataDir => {
            let path = get_safenode_root_dir(peer_id)?.join("logs");
            LogOutputDest::Path(path)
        }
        LogOutputDestArg::Path(path) => LogOutputDest::Path(path.clone()),
    };

    #[cfg(not(feature = "otlp"))]
    let (reload_handle, log_appender_guard) = {
        let mut log_builder = sn_logging::LogBuilder::new(logging_targets);
        log_builder.output_dest(output_dest.clone());
        log_builder.format(opt.log_format.unwrap_or(LogFormat::Default));
        if let Some(files) = opt.max_uncompressed_log_files {
            log_builder.max_uncompressed_log_files(files);
        }
        if let Some(files) = opt.max_compressed_log_files {
            log_builder.max_compressed_log_files(files);
        }

        log_builder.initialize()?
    };

    #[cfg(feature = "otlp")]
    let (_rt, reload_handle, log_appender_guard) = {
        // init logging in a separate runtime if we are sending traces to an opentelemetry server
        let rt = Runtime::new()?;
        let (reload_handle, log_appender_guard) = rt.block_on(async {
            let mut log_builder = sn_logging::LogBuilder::new(logging_targets);
            log_builder.output_dest(output_dest.clone());
            log_builder.format(opt.log_format.unwrap_or(LogFormat::Default));
            if let Some(files) = opt.max_uncompressed_log_files {
                log_builder.max_uncompressed_log_files(files);
            }
            if let Some(files) = opt.max_compressed_log_files {
                log_builder.max_compressed_log_files(files);
            }
            log_builder.initialize()
        })?;
        (rt, reload_handle, log_appender_guard)
    };

    Ok((output_dest.to_string(), reload_handle, log_appender_guard))
}

fn create_secret_key_file(path: impl AsRef<Path>) -> Result<std::fs::File, std::io::Error> {
    let mut opt = std::fs::OpenOptions::new();
    opt.write(true).create_new(true);

    // On Unix systems, make sure only the current user can read/write.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opt.mode(0o600);
    }

    opt.open(path)
}

fn keypair_from_path(path: impl AsRef<Path>) -> Result<Keypair> {
    let keypair = match std::fs::read(&path) {
        // If the file is opened successfully, read the key from it
        Ok(key) => {
            let keypair = Keypair::ed25519_from_bytes(key)
                .map_err(|err| eyre!("could not read ed25519 key from file: {err}"))?;

            info!("loaded secret key from file: {:?}", path.as_ref());

            keypair
        }
        // In case the file is not found, generate a new keypair and write it to the file
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let secret_key = libp2p::identity::ed25519::SecretKey::generate();
            let mut file = create_secret_key_file(&path)
                .map_err(|err| eyre!("could not create secret key file: {err}"))?;
            file.write_all(secret_key.as_ref())?;

            info!("generated new key and stored to file: {:?}", path.as_ref());

            libp2p::identity::ed25519::Keypair::from(secret_key).into()
        }
        // Else the file can't be opened, for whatever reason (e.g. permissions).
        Err(err) => {
            return Err(eyre!("failed to read secret key file: {err}"));
        }
    };

    Ok(keypair)
}

/// The keypair is located inside the root directory. At the same time, when no dir is specified,
/// the dir name is derived from the keypair used in the application: the peer ID is used as the directory name.
fn get_root_dir_and_keypair(root_dir: &Option<PathBuf>) -> Result<(PathBuf, Keypair)> {
    match root_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)?;

            let secret_key_path = dir.join("secret-key");
            Ok((dir.clone(), keypair_from_path(secret_key_path)?))
        }
        None => {
            let secret_key = libp2p::identity::ed25519::SecretKey::generate();
            let keypair: Keypair =
                libp2p::identity::ed25519::Keypair::from(secret_key.clone()).into();
            let peer_id = keypair.public().to_peer_id();

            let dir = get_safenode_root_dir(peer_id)?;
            std::fs::create_dir_all(&dir)?;

            let secret_key_path = dir.join("secret-key");

            let mut file = create_secret_key_file(secret_key_path)
                .map_err(|err| eyre!("could not create secret key file: {err}"))?;
            file.write_all(secret_key.as_ref())?;

            Ok((dir, keypair))
        }
    }
}

/// Starts a new process running the binary with the same args as
/// the current process
/// Optionally provide the node's root dir and listen port to retain it's PeerId
fn start_new_node_process(retain_peer_id: Option<(PathBuf, u16)>) {
    // Retrieve the current executable's path
    let current_exe = env::current_exe().expect("could not get current executable path");

    // Retrieve the command-line arguments passed to this process
    let args: Vec<String> = env::args().collect();

    info!("Original args are: {args:?}");
    info!("Current exe is: {current_exe:?}");

    // Convert current exe path to string, log an error and return if it fails
    let current_exe = match current_exe.to_str() {
        Some(s) => {
            // remove "(deleted)" string from current exe path
            if s.contains(" (deleted)") {
                warn!("The current executable path contains ' (deleted)', which may lead to unexpected behavior. This has been removed from the exe location string");
                s.replace(" (deleted)", "")
            } else {
                s.to_string()
            }
        }
        None => {
            error!("Failed to convert current executable path to string");
            return;
        }
    };

    // Create a new Command instance to run the current executable
    let mut cmd = Command::new(current_exe);

    // Set the arguments for the new Command
    cmd.args(&args[1..]); // Exclude the first argument (binary path)

    if let Some((root_dir, port)) = retain_peer_id {
        cmd.arg("--root-dir");
        cmd.arg(format!("{root_dir:?}"));
        cmd.arg("--port");
        cmd.arg(port.to_string());
    }

    warn!(
        "Attempting to start a new process as node process loop has been broken: {:?}",
        cmd
    );
    // Execute the command
    let _handle = match cmd.spawn() {
        Ok(status) => status,
        Err(e) => {
            // Do not return an error as this isn't a critical failure.
            // The current node can continue.
            eprintln!("Failed to execute hard-restart command: {e:?}");
            error!("Failed to execute hard-restart command: {e:?}");

            return;
        }
    };
}
