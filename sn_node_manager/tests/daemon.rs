// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod utils;

use assert_cmd::Command;
use color_eyre::eyre::{bail, OptionExt, Result};
use libp2p::PeerId;
use sn_node_manager::daemon_control::DAEMON_DEFAULT_PORT;
use sn_protocol::safenode_manager_proto::{
    safe_node_manager_client::SafeNodeManagerClient, NodeServiceRestartRequest,
};
use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    time::Duration,
};
use tonic::Request;
use utils::get_service_status;

const CI_USER: &str = "runner";

/// These tests need to execute as the root user.
///
/// They are intended to run on a CI-based environment with a fresh build agent because they will
/// create real services and user accounts, and will not attempt to clean themselves up.
///
/// Each test also needs to run in isolation, otherwise they will interfere with each other.
///
/// If you run them on your own dev machine, do so at your own risk!

#[tokio::test]
async fn restart_node() -> Result<()> {
    // build daemon
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("--bin")
        .arg("safenodemand");

    // 1. Preserve the PeerId
    let node_index_to_restart = 0;
    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("add")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .arg("--peer")
        .arg("/ip4/127.0.0.1/udp/46091/p2p/12D3KooWAWnbQLxqspWeB3M8HB3ab3CSj6FYzsJxEG9XdVnGNCod")
        .assert()
        .success();
    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("start").assert().success();

    let services = get_service_status().await?;
    let old_pid = services[node_index_to_restart]["pid"]
        .as_u64()
        .ok_or_eyre("PID should be present")?;
    assert_eq!(services.len(), 3);

    // start daemon
    let mut cwd = env::current_dir()?;
    cwd.pop();
    let safenodemand_path = cwd.join("target").join("release").join("safenodemand");
    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("daemon")
        .arg("add")
        .arg("--path")
        .arg(format!("{safenodemand_path:?}").as_str())
        .assert()
        .success();
    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("daemon").arg("start").assert().success();

    // restart a node
    let mut rpc_client = get_safenode_manager_rpc_client(SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        DAEMON_DEFAULT_PORT,
    ))
    .await?;
    let node_to_restart = services[node_index_to_restart]["peer_id"]
        .as_str()
        .ok_or_eyre("We should have PeerId")?;
    let node_to_restart = PeerId::from_str(node_to_restart)?;

    let _response = rpc_client
        .restart_node_service(Request::new(NodeServiceRestartRequest {
            peer_id: node_to_restart.to_bytes(),
            delay_millis: 0,
            retain_peer_id: true,
        }))
        .await?;

    // make sure that we still have just 3 services running and pid's are different
    let services = get_service_status().await?;
    assert_eq!(services.len(), 3);
    let new_pid = services[node_index_to_restart]["pid"]
        .as_u64()
        .ok_or_eyre("PID should be present")?;
    assert_ne!(old_pid, new_pid);

    // 2. Start as a fresh node
    let _response = rpc_client
        .restart_node_service(Request::new(NodeServiceRestartRequest {
            peer_id: node_to_restart.to_bytes(),
            delay_millis: 0,
            retain_peer_id: false,
        }))
        .await?;

    // make sure that we still have an extra service, and the new one has the same rpc addr as the old one.
    let services = get_service_status().await?;
    assert_eq!(services.len(), 4);
    let old_rpc_socket_addr = services[node_index_to_restart]["rpc_socket_addr"]
        .as_str()
        .ok_or_eyre("rpc_socket_addr should be present")?;
    let new_rpc_socket_addr = services[3]["rpc_socket_addr"]
        .as_str()
        .ok_or_eyre("rpc_socket_addr should be present")?;
    assert_eq!(old_rpc_socket_addr, new_rpc_socket_addr);

    Ok(())
}

// Connect to a RPC socket addr with retry
pub async fn get_safenode_manager_rpc_client(
    socket_addr: SocketAddr,
) -> Result<SafeNodeManagerClient<tonic::transport::Channel>> {
    // get the new PeerId for the current NodeIndex
    let endpoint = format!("https://{socket_addr}");
    let mut attempts = 0;
    loop {
        if let Ok(rpc_client) = SafeNodeManagerClient::connect(endpoint.clone()).await {
            break Ok(rpc_client);
        }
        attempts += 1;
        println!("Could not connect to rpc {endpoint:?}. Attempts: {attempts:?}/10");
        tokio::time::sleep(Duration::from_secs(1)).await;
        if attempts >= 10 {
            bail!("Failed to connect to {endpoint:?} even after 10 retries");
        }
    }
}
