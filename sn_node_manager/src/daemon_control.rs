// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{node_control, service::ServiceControl, VerbosityLevel};
use color_eyre::{eyre::OptionExt, Result};
use libp2p::{multiaddr::Protocol, Multiaddr};
use service_manager::{ServiceInstallCtx, ServiceLabel};
use sn_node_rpc_client::RpcActions;
use sn_protocol::node_registry::Node;
use std::{
    ffi::OsString,
    net::Ipv4Addr,
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};

pub fn run_daemon(
    address: Ipv4Addr,
    port: u16,
    daemon_path: PathBuf,
    service_control: &dyn ServiceControl,
    _verbosity: VerbosityLevel,
) -> Result<()> {
    let service_name: ServiceLabel = "safenode-manager-daemon".parse()?;

    let install_ctx = ServiceInstallCtx {
        label: service_name.clone(),
        program: daemon_path,
        args: vec![
            OsString::from("--port"),
            OsString::from(port.to_string()),
            OsString::from("--address"),
            OsString::from(address.to_string()),
        ],
        contents: None,
        username: None,
        working_directory: None,
        environment: None,
    };
    service_control.install(install_ctx)?;
    service_control.start(&service_name.to_string())?;

    Ok(())
}

pub async fn restart_node_process(
    node: &mut Node,
    rpc_client: &dyn RpcActions,
    preserve_peer_id: bool,
    bootstrap_peers: Vec<Multiaddr>,
) -> Result<()> {
    // stop the process
    let pid = node.pid.ok_or_eyre("Could not find node's PeerId")?;
    if let Some(process) = sysinfo::System::new_all().process(Pid::from_u32(pid)) {
        process.kill();
        println!("Process with PID {} has been killed.", pid);
    }

    // start the process
    let node_port = node
        .get_safenode_port()
        .ok_or_eyre("Could not obtain node port")?;
    // todo: deduplicate code inside local.rs
    let mut args = Vec::new();
    for peer in bootstrap_peers {
        args.push("--peer".to_string());
        args.push(peer.to_string());
    }

    args.push("--local".to_string());
    args.push("--rpc".to_string());
    args.push(node.rpc_socket_addr.to_string());
    // resuse the same root dir + ports to preserve the peer id
    if preserve_peer_id {
        args.push("--root-dir".to_string());
        args.push(format!("{:?}", node.data_dir_path));
        args.push("--port".to_string());
        args.push(node_port.to_string());
    }

    let user = users::get_user_by_name(&node.user).ok_or_eyre("Could not obtain UID")?;
    let uid = user.uid();
    println!("Starting node process with args: {args:?} for uid: {uid:?}");

    Command::new(&node.safenode_path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .uid(uid)
        .spawn()?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // update the node registry
    let node_info = rpc_client.node_info().await?;
    let peer_id = node_info.peer_id;
    let network_info = rpc_client.network_info().await?;
    let connected_peers = Some(network_info.connected_peers);
    let listen_addrs = network_info
        .listeners
        .into_iter()
        .map(|addr| addr.with(Protocol::P2p(node_info.peer_id)))
        .collect();

    if preserve_peer_id {
        if peer_id
            != node
                .peer_id
                .ok_or_eyre("Previous peer_id should be present")?
        {
            println!("The peer ID has changed even though it should have been preserved.");
        }
    }

    node.connected_peers = connected_peers;
    node.pid = Some(node_info.pid);
    node.listen_addr = Some(listen_addrs);
    node.peer_id = Some(peer_id);

    Ok(())
}

pub async fn restart_node_service(
    node: &mut Node,
    rpc_client: &dyn RpcActions,
    service_control: &dyn ServiceControl,
    preserve_peer_id: bool,
    bootstrap_peers: Vec<Multiaddr>,
    env_variables: Option<Vec<(String, String)>>,
) -> Result<()> {
    node_control::stop(node, service_control).await?;

    // reuse the same port and root dir to preserve peer id.
    if preserve_peer_id {
        service_control.uninstall(&node.service_name.clone())?;
        let install_ctx = node_control::InstallNodeServiceCtxBuilder {
            local: node.local,
            data_dir_path: node.data_dir_path.clone(),
            genesis: node.genesis,
            name: node.service_name.clone(),
            node_port: node.get_safenode_port(),
            bootstrap_peers,
            rpc_socket_addr: node.rpc_socket_addr,
            log_dir_path: node.log_dir_path.clone(),
            safenode_path: node.safenode_path.clone(),
            service_user: node.user.clone(),
            env_variables,
        }
        .build()?;
        service_control.install(install_ctx)?;
    }

    node_control::start(node, service_control, rpc_client, VerbosityLevel::Normal).await?;
    Ok(())
}
