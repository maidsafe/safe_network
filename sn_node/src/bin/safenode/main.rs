// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod rpc;

use clap::Parser;
use eyre::{eyre, Error, Result};
use libp2p::{identity::Keypair, Multiaddr, PeerId};
#[cfg(feature = "metrics")]
use sn_logging::metrics::init_metrics;
use sn_logging::{parse_log_format, LogFormat, LogOutputDest};
use sn_node::{Marker, Node, NodeEvent, NodeEventsReceiver};
use sn_peers_acquisition::{parse_peer_addr, PeersArgs};
use std::{
    env,
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    runtime::Runtime,
    sync::{broadcast::error::RecvError, mpsc},
    time::sleep,
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_core::Level;

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
    /// Specify the logging output destination.
    ///
    /// Valid values are "stdout", "data-dir", or a custom path.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/node/<peer-id>/logs
    ///  - macOS: $HOME/Library/Application Support/safe/node/<peer-id>/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\node\<peer-id>\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, default_value_t = LogOutputDestArg::Stdout, value_parser = parse_log_output, verbatim_doc_comment)]
    log_output_dest: LogOutputDestArg,

    /// Specify the logging format.
    ///
    /// Valid values are "default" or "json".
    ///
    /// If the argument is not used, the default format will be applied.
    #[clap(long, value_parser = parse_log_format, verbatim_doc_comment)]
    log_format: Option<LogFormat>,

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
}

#[derive(Debug)]
// To be sent to the main thread in order to stop/restart the execution of the safenode app.
enum NodeCtrl {
    // Request to stop the exeution of the safenode app, providing an error as a reason for it.
    Stop { delay: Duration, cause: Error },
    // Request to restart the exeution of the safenode app,
    // retrying to join the network, after the requested delay.
    Restart(Duration),
    // Request to update the safenode app, and restart it, after the requested delay.
    Update(Duration),
}

fn main() -> Result<()> {
    let mut opt = Opt::parse();

    let node_socket_addr = SocketAddr::new(opt.ip, opt.port);
    let (root_dir, keypair) = get_root_dir_and_keypair(opt.root_dir)?;

    let (log_output_dest, _log_appender_guard) = init_logging(
        opt.log_output_dest,
        keypair.public().to_peer_id(),
        opt.log_format,
    )?;

    // The original passed in peers may got restarted as well.
    // Hence, try to parse from env_var and add as initial peers,
    // if not presented yet.
    // This is only used for non-local-discocery,
    // i.e. make SAFE_PEERS always being a fall back option for initial peers.
    if !cfg!(feature = "local-discovery") {
        match std::env::var("SAFE_PEERS") {
            Ok(str) => match parse_peer_addr(&str) {
                Ok(peer) => {
                    if !opt
                        .peers
                        .peers
                        .iter()
                        .any(|existing_peer| *existing_peer == peer)
                    {
                        opt.peers.peers.push(peer);
                    }
                }
                Err(err) => error!("Can't parse SAFE_PEERS {str:?} with error {err:?}"),
            },
            Err(err) => error!("Can't get env var SAFE_PEERS with error {err:?}"),
        }
    }

    if opt.peers.peers.is_empty() {
        if !cfg!(feature = "local-discovery") {
            warn!("No peers given. As `local-discovery` feature is disabled, we will not be able to connect to the network.");
        } else {
            info!("No peers given. As `local-discovery` feature is enabled, we will attempt to connect to the network using mDNS.");
        }
    }
    let initial_peers = opt.peers.peers.clone();

    let msg = format!(
        "Running {} v{}",
        env!("CARGO_BIN_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    info!("\n{}\n{}", msg, "=".repeat(msg.len()));
    debug!("Built with git version: {}", sn_build_info::git_info());

    info!("Node started with initial_peers {initial_peers:?}");

    // Create a tokio runtime per `start_node` attempt, this ensures
    // any spawned tasks are closed before we would attempt to run
    // another process with these args.
    let rt = Runtime::new()?;
    #[cfg(feature = "metrics")]
    rt.spawn(init_metrics(std::process::id()));
    rt.block_on(start_node(
        keypair,
        node_socket_addr,
        initial_peers,
        opt.rpc,
        opt.local,
        &log_output_dest,
        root_dir,
    ))?;

    // actively shut down the runtime
    rt.shutdown_timeout(Duration::from_secs(2));

    // we got this far without error, which means (so far) the only thing we should be doing
    // is restarting the node
    start_new_node_process();

    // Command was successful, so we shut down the process
    println!("A new node process has been started successfully.");
    Ok(())
}

/// Start a node with the given configuration.
async fn start_node(
    keypair: Keypair,
    node_socket_addr: SocketAddr,
    peers: Vec<Multiaddr>,
    rpc: Option<SocketAddr>,
    local: bool,
    log_output_dest: &str,
    root_dir: PathBuf,
) -> Result<()> {
    let started_instant = std::time::Instant::now();

    info!("Starting node ...");
    let running_node = Node::run(keypair, node_socket_addr, peers, local, root_dir).await?;

    // write the PID to the root dir
    let pid = std::process::id();
    let pid_file = running_node.root_dir_path().join("safenode.pid");
    let mut file = File::create(&pid_file).await?;
    file.write_all(pid.to_string().as_bytes()).await?;

    // Channel to receive node ctrl cmds from RPC service (if enabled), and events monitoring task
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<NodeCtrl>(5);

    // Monitor `NodeEvents`
    let node_events_rx = running_node.node_events_channel().subscribe();
    monitor_node_events(node_events_rx, ctrl_tx.clone());

    // Start up gRPC interface if enabled by user
    if let Some(addr) = rpc {
        rpc::start_rpc_service(
            addr,
            log_output_dest,
            running_node.clone(),
            ctrl_tx,
            started_instant,
        );
    }

    // Keep the node and gRPC service (if enabled) running.
    // We'll monitor any NodeCtrl cmd to restart/stop/update,
    loop {
        match ctrl_rx.recv().await {
            Some(NodeCtrl::Restart(delay)) => {
                let msg = format!("Node is restarting in {delay:?}...");
                info!("{msg}");
                println!("{msg} Node path: {log_output_dest}");
                sleep(delay).await;

                break Ok(());
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
                Ok(NodeEvent::BehindNat) => {
                    if let Err(err) = ctrl_tx
                        .send(NodeCtrl::Stop {
                            delay: Duration::from_secs(1),
                            cause: eyre!("We have been determined to be behind a NAT. This means we are not reachable externally by other nodes. In the future, the network will implement relays that allow us to still join the network."),
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
                    info!("Currently ignored node event {event:?}");
                }
                Err(RecvError::Lagged(n)) => {
                    warn!("Skipped {n} node events!");
                    continue;
                }
            }
        }
    });
}

fn init_logging(
    log_output_dest: LogOutputDestArg,
    peer_id: PeerId,
    format: Option<LogFormat>,
) -> Result<(String, Option<WorkerGuard>)> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];

    let output_dest = match log_output_dest {
        LogOutputDestArg::Stdout => LogOutputDest::Stdout,
        LogOutputDestArg::DataDir => {
            let path = get_root_dir(peer_id)?.join("logs");
            LogOutputDest::Path(path)
        }
        LogOutputDestArg::Path(path) => LogOutputDest::Path(path),
    };

    #[cfg(not(feature = "otlp"))]
    let log_appender_guard = sn_logging::init_logging(
        logging_targets,
        output_dest.clone(),
        format.unwrap_or(LogFormat::Default),
    )?;
    #[cfg(feature = "otlp")]
    let (_rt, log_appender_guard) = {
        // init logging in a separate runtime if we are sending traces to an opentelemetry server
        let rt = Runtime::new()?;
        let guard = rt.block_on(async {
            sn_logging::init_logging(
                logging_targets,
                output_dest.clone(),
                format.unwrap_or(LogFormat::Default),
            )
        })?;
        (rt, guard)
    };
    Ok((output_dest.to_string(), log_appender_guard))
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

fn get_root_dir(peer_id: PeerId) -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain root directory path".to_string()))?
        .join("safe")
        .join("node")
        .join(peer_id.to_string());

    Ok(dir)
}

/// The keypair is located inside the root directory. At the same time, when no dir is specified,
/// the dir name is derived from the keypair used in the application: the peer ID is used as the directory name.
fn get_root_dir_and_keypair(root_dir: Option<PathBuf>) -> Result<(PathBuf, Keypair)> {
    match root_dir {
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;

            let secret_key_path = dir.join("secret-key");
            Ok((dir, keypair_from_path(secret_key_path)?))
        }
        None => {
            let secret_key = libp2p::identity::ed25519::SecretKey::generate();
            let keypair: Keypair =
                libp2p::identity::ed25519::Keypair::from(secret_key.clone()).into();
            let peer_id = keypair.public().to_peer_id();

            let dir = get_root_dir(peer_id)?;
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
fn start_new_node_process() {
    // Retrieve the current executable's path
    let current_exe = env::current_exe().unwrap();

    // Retrieve the command-line arguments passed to this process
    let args: Vec<String> = env::args().collect();

    info!("Original args are: {args:?}");

    // Create a new Command instance to run the current executable
    let mut cmd = Command::new(current_exe);

    // Set the arguments for the new Command
    cmd.args(&args[1..]); // Exclude the first argument (binary path)

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
            eprintln!("Failed to execute hard-restart command: {}", e);
            error!("Failed to execute hard-restart command: {}", e);

            return;
        }
    };
}
