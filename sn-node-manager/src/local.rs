// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::node::{Node, NodeRegistry, NodeStatus};
use crate::service::ServiceControl;
use color_eyre::{eyre::eyre, Result};
use colored::Colorize;
use libp2p::{Multiaddr, PeerId};
#[cfg(test)]
use mockall::automock;
use sn_node_rpc_client::{RpcActions, RpcClient};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;
use sysinfo::{Pid, ProcessExt, System, SystemExt};

#[cfg_attr(test, automock)]
pub trait Launcher {
    fn get_safenode_path(&self) -> PathBuf;
    fn get_safenode_version(&self) -> Result<String>;
    fn launch_faucet(&self, genesis_multiaddr: &Multiaddr) -> Result<u32>;
    fn launch_node(&self, port: u16, rpc_port: u16, peers: Vec<Multiaddr>) -> Result<()>;
    fn wait(&self, delay: u64);
}

#[derive(Default)]
pub struct LocalSafeLauncher {
    pub faucet_bin_path: PathBuf,
    pub safenode_bin_path: PathBuf,
}

impl Launcher for LocalSafeLauncher {
    fn get_safenode_path(&self) -> PathBuf {
        self.safenode_bin_path.clone()
    }

    fn get_safenode_version(&self) -> Result<String> {
        let mut cmd = Command::new(&self.safenode_bin_path)
            .arg("--version")
            .stdout(Stdio::piped())
            .spawn()?;

        let mut output = String::new();
        cmd.stdout
            .as_mut()
            .ok_or_else(|| eyre!("Failed to capture stdout"))?
            .read_to_string(&mut output)?;

        let version = output
            .split_whitespace()
            .nth(2)
            .ok_or_else(|| eyre!("Failed to parse version"))?
            .to_string();

        Ok(version)
    }

    fn launch_faucet(&self, genesis_multiaddr: &Multiaddr) -> Result<u32> {
        let args = vec![
            "--peer".to_string(),
            genesis_multiaddr.to_string(),
            "server".to_string(),
        ];
        let child = Command::new(self.faucet_bin_path.clone())
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(child.id())
    }

    fn launch_node(&self, port: u16, rpc_port: u16, peers: Vec<Multiaddr>) -> Result<()> {
        let mut args = Vec::new();
        if peers.is_empty() {
            args.push("--first".to_string())
        } else {
            for peer in peers {
                args.push("--peer".to_string());
                args.push(peer.to_string());
            }
        }
        args.push("--local".to_string());
        args.push("--port".to_string());
        args.push(port.to_string());
        args.push("--rpc".to_string());
        args.push(format!("127.0.0.1:{rpc_port}"));

        Command::new(self.safenode_bin_path.clone())
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        Ok(())
    }

    /// Provide a delay for the service to start or stop.
    ///
    /// This is wrapped mainly just for unit testing.
    fn wait(&self, delay: u64) {
        std::thread::sleep(std::time::Duration::from_secs(delay));
    }
}

pub fn kill_network(node_registry: &NodeRegistry, keep_directories: bool) -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();

    // It's possible that the faucet was not spun up because the network failed the validation
    // process. If it wasn't running, we obviously don't need to do anything.
    if let Some(pid) = node_registry.faucet_pid {
        // If we're here, the faucet was spun up. However, it's possible for the process to have
        // died since then. In that case, we don't need to do anything.
        if let Some(process) = system.process(Pid::from(pid as usize)) {
            process.kill();
            println!("{} Killed faucet", "✓".green());
        }
    }

    for node in node_registry.nodes.iter() {
        println!("{}:", node.service_name);
        // If the PID is not set it means the `status` command ran and determined the node was
        // already dead anyway, so we don't need to do anything.
        if let Some(pid) = node.pid {
            // It could be possible that None would be returned here, if the process had already
            // died, but the `status` command had not ran. In that case, we don't need to do
            // anything anyway.
            if let Some(process) = system.process(Pid::from(pid as usize)) {
                process.kill();
                println!("  {} Killed process", "✓".green());
            }
        }

        if !keep_directories {
            // The data directory must be set for a running node.
            // At this point we don't allow path overrides, so deleting the data directory will clear
            // the log directory also.
            let data_dir_path = node.data_dir_path.as_ref().unwrap();
            std::fs::remove_dir_all(data_dir_path)?;
            println!(
                "  {} Removed {}",
                "✓".green(),
                data_dir_path.to_string_lossy()
            );
        }
    }

    Ok(())
}

pub struct LocalNetworkOptions {
    pub faucet_bin_path: PathBuf,
    pub join: bool,
    pub node_count: u16,
    pub peers: Option<Vec<Multiaddr>>,
    pub safenode_bin_path: PathBuf,
    pub skip_validation: bool,
}

pub async fn run_network(
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    network_options: LocalNetworkOptions,
) -> Result<()> {
    let launcher = LocalSafeLauncher {
        safenode_bin_path: network_options.safenode_bin_path.to_path_buf(),
        faucet_bin_path: network_options.faucet_bin_path.to_path_buf(),
    };

    let (peers, start) = if network_options.join {
        if let Some(peers) = network_options.peers {
            (peers, 1)
        } else {
            let peer = node_registry
                .nodes
                .iter()
                .find_map(|n| n.get_multiaddr())
                .ok_or_else(|| eyre!("Unable to obtain a peer to connect to"))?;
            (vec![peer], 1)
        }
    } else {
        let port = service_control.get_available_port()?;
        let rpc_port = service_control.get_available_port()?;
        let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{rpc_port}"));
        let genesis_multiaddr = run_node(
            true,
            port,
            rpc_port,
            vec![],
            &launcher,
            node_registry,
            &rpc_client,
        )
        .await?;
        (vec![genesis_multiaddr], 2)
    };

    for _ in start..=network_options.node_count {
        let port = service_control.get_available_port()?;
        let rpc_port = service_control.get_available_port()?;
        let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{rpc_port}"));
        run_node(
            false,
            port,
            rpc_port,
            peers.clone(),
            &launcher,
            node_registry,
            &rpc_client,
        )
        .await?;

        // We save the node registry for each launch because it's possible any node can fail to
        // launch, or maybe the validation will fail. In the error case, we will want to use the
        // `kill` command for the nodes that we did spin up. The `kill` command works on the basis
        // of what's in the node registry.
        node_registry.save()?;
    }

    if !network_options.skip_validation {
        println!("Waiting for 10 seconds before validating the network...");
        std::thread::sleep(std::time::Duration::from_secs(10));
        validate_network(node_registry, peers.clone()).await?;
    }

    if !network_options.join {
        println!("Launching the faucet server...");
        let faucet_pid = launcher.launch_faucet(&peers[0])?;
        node_registry.faucet_pid = Some(faucet_pid);
    }

    Ok(())
}

pub async fn run_node(
    genesis: bool,
    port: u16,
    rpc_port: u16,
    peer: Vec<Multiaddr>,
    launcher: &dyn Launcher,
    node_registry: &mut NodeRegistry,
    rpc_client: &dyn RpcActions,
) -> Result<Multiaddr> {
    let version = launcher.get_safenode_version()?;
    let number = (node_registry.nodes.len() as u16) + 1;

    println!("Launching node {number}...");
    launcher.launch_node(port, rpc_port, peer.clone())?;
    launcher.wait(2);

    let node_info = rpc_client.node_info().await?;
    let peer_id = node_info.peer_id;

    node_registry.nodes.push(Node {
        genesis,
        service_name: format!("safenode-local{number}"),
        user: get_username()?,
        number,
        port,
        rpc_port,
        version: version.clone(),
        status: NodeStatus::Running,
        pid: Some(node_info.pid),
        peer_id: Some(peer_id),
        log_dir_path: Some(node_info.log_path),
        data_dir_path: Some(node_info.data_path),
        safenode_path: Some(launcher.get_safenode_path()),
    });

    Ok(Multiaddr::from_str(&format!(
        "/ip4/127.0.0.1/tcp/{port}/p2p/{peer_id}"
    ))?)
}

///
/// Private Helpers
///

#[cfg(target_os = "windows")]
fn get_username() -> Result<String> {
    Ok(std::env::var("USERNAME")?)
}

#[cfg(not(target_os = "windows"))]
fn get_username() -> Result<String> {
    Ok(std::env::var("USER")?)
}

async fn validate_network(node_registry: &mut NodeRegistry, peers: Vec<Multiaddr>) -> Result<()> {
    let mut all_peers = node_registry
        .nodes
        .iter()
        .map(|n| n.peer_id.unwrap())
        .collect::<Vec<PeerId>>();
    // The additional peers are peers being managed outwith the node manager. This only applies
    // when we've joined a network not being managed by the node manager. Otherwise, this list will
    // be empty.
    let additional_peers = peers
        .into_iter()
        .filter_map(|addr| {
            addr.to_string()
                .rsplit('/')
                .next()
                .and_then(|id_str| PeerId::from_str(id_str).ok())
        })
        .collect::<Vec<PeerId>>();
    all_peers.extend(additional_peers);

    for node in node_registry.nodes.iter() {
        let rpc_client = RpcClient::new(&format!("https://127.0.0.1:{}", node.rpc_port));
        let net_info = rpc_client.network_info().await?;
        let peers = net_info.connected_peers;
        println!("Node {} has {} peers", node.peer_id.unwrap(), peers.len());

        // Look for peers that are not supposed to be present in the network. This can happen if
        // the node has connected to peers on other networks.
        let invalid_peers: Vec<PeerId> = peers
            .iter()
            .filter(|peer| !all_peers.contains(peer))
            .cloned()
            .collect();
        if !invalid_peers.is_empty() {
            for invalid_peer in invalid_peers.iter() {
                println!("Invalid peer found: {}", invalid_peer);
            }
            return Err(eyre!("Network validation failed",));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use libp2p_identity::PeerId;
    use mockall::mock;
    use mockall::predicate::*;
    use sn_node_rpc_client::{
        NetworkInfo, NodeInfo, RecordAddress, Result as RpcResult, RpcActions,
    };
    use std::str::FromStr;

    mock! {
        pub RpcClient {}
        #[async_trait]
        impl RpcActions for RpcClient {
            async fn node_info(&self) -> RpcResult<NodeInfo>;
            async fn network_info(&self) -> RpcResult<NetworkInfo>;
            async fn record_addresses(&self) -> RpcResult<Vec<RecordAddress>>;
            async fn gossipsub_subscribe(&self, topic: &str) -> RpcResult<()>;
            async fn gossipsub_unsubscribe(&self, topic: &str) -> RpcResult<()>;
            async fn gossipsub_publish(&self, topic: &str, message: &str) -> RpcResult<()>;
            async fn node_restart(&self, delay_millis: u64) -> RpcResult<()>;
            async fn node_stop(&self, delay_millis: u64) -> RpcResult<()>;
            async fn node_update(&self, delay_millis: u64) -> RpcResult<()>;
        }
    }

    #[tokio::test]
    async fn run_node_should_launch_the_genesis_node() -> Result<()> {
        let mut mock_launcher = MockLauncher::new();
        let mut node_registry = NodeRegistry {
            save_path: PathBuf::new(),
            nodes: vec![],
            faucet_pid: None,
        };
        let mut mock_rpc_client = MockRpcClient::new();

        let peer_id = PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?;
        let port = 12000;
        let rpc_port = 13000;
        let node_multiaddr =
            Multiaddr::from_str(&format!("/ip4/127.0.0.1/tcp/{port}/p2p/{peer_id}"))?;

        mock_launcher
            .expect_get_safenode_version()
            .times(1)
            .returning(|| Ok("0.100.12".to_string()));
        mock_launcher
            .expect_launch_node()
            .with(eq(port), eq(rpc_port), eq(vec![]))
            .times(1)
            .returning(|_, _, _| Ok(()));
        mock_launcher
            .expect_wait()
            .with(eq(2))
            .times(1)
            .returning(|_| ());
        mock_launcher
            .expect_get_safenode_path()
            .times(1)
            .returning(|| PathBuf::from("/usr/local/bin/safenode"));

        mock_rpc_client
            .expect_node_info()
            .times(1)
            .returning(move || {
                Ok(NodeInfo {
                    pid: 1000,
                    peer_id,
                    data_path: PathBuf::from(format!("~/.local/share/safe/{peer_id}")),
                    log_path: PathBuf::from(format!("~/.local/share/safe/{peer_id}/logs")),
                    version: "0.100.12".to_string(),
                    uptime: std::time::Duration::from_secs(1), // the service was just started
                })
            });

        let multiaddr = run_node(
            true,
            port,
            rpc_port,
            vec![],
            &mock_launcher,
            &mut node_registry,
            &mock_rpc_client,
        )
        .await?;

        assert_eq!(multiaddr, node_multiaddr);
        assert_eq!(node_registry.nodes.len(), 1);
        assert!(node_registry.nodes[0].genesis);
        assert_eq!(node_registry.nodes[0].version, "0.100.12");
        assert_eq!(node_registry.nodes[0].service_name, "safenode-local1");
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(PathBuf::from(format!("~/.local/share/safe/{peer_id}")))
        );
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(PathBuf::from(format!("~/.local/share/safe/{peer_id}/logs")))
        );
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].pid, Some(1000));
        assert_eq!(node_registry.nodes[0].port, port);
        assert_eq!(node_registry.nodes[0].rpc_port, rpc_port);
        assert_eq!(node_registry.nodes[0].status, NodeStatus::Running);
        assert_eq!(
            node_registry.nodes[0].safenode_path,
            Some(PathBuf::from("/usr/local/bin/safenode"))
        );

        Ok(())
    }

    #[tokio::test]
    async fn run_node_should_launch_an_additional_node() -> Result<()> {
        let peer_id = PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?;
        let genesis_peer_addr =
            Multiaddr::from_str(&format!("/ip4/127.0.0.1/tcp/12000/p2p/{peer_id}"))?;

        let mut mock_launcher = MockLauncher::new();
        let mut node_registry = NodeRegistry {
            save_path: PathBuf::new(),
            nodes: vec![Node {
                genesis: true,
                service_name: "safenode-local1".to_string(),
                user: get_username()?,
                number: 1,
                port: 12000,
                rpc_port: 13000,
                version: "0.100.12".to_string(),
                status: NodeStatus::Running,
                pid: Some(1000),
                peer_id: Some(peer_id),
                log_dir_path: Some(PathBuf::from(format!("~/.local/share/safe/{peer_id}/logs"))),
                data_dir_path: Some(PathBuf::from(format!("~/.local/share/safe/{peer_id}"))),
                safenode_path: Some(PathBuf::from("/usr/local/bin/safenode")),
            }],
            faucet_pid: None,
        };
        let mut mock_rpc_client = MockRpcClient::new();

        let peer_id = PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?;
        let port = 12001;
        let rpc_port = 13001;
        let node_peer_addr =
            Multiaddr::from_str(&format!("/ip4/127.0.0.1/tcp/{port}/p2p/{peer_id}"))?;

        mock_launcher
            .expect_get_safenode_version()
            .times(1)
            .returning(|| Ok("0.100.12".to_string()));
        mock_launcher
            .expect_launch_node()
            .with(eq(port), eq(rpc_port), eq(vec![genesis_peer_addr.clone()]))
            .times(1)
            .returning(|_, _, _| Ok(()));
        mock_launcher
            .expect_wait()
            .with(eq(2))
            .times(1)
            .returning(|_| ());
        mock_launcher
            .expect_get_safenode_path()
            .times(1)
            .returning(|| PathBuf::from("/usr/local/bin/safenode"));

        mock_rpc_client
            .expect_node_info()
            .times(1)
            .returning(move || {
                Ok(NodeInfo {
                    pid: 1001,
                    peer_id,
                    data_path: PathBuf::from(format!("~/.local/share/safe/{peer_id}")),
                    log_path: PathBuf::from(format!("~/.local/share/safe/{peer_id}/logs")),
                    version: "0.100.12".to_string(),
                    uptime: std::time::Duration::from_secs(1), // the service was just started
                })
            });

        let multiaddr = run_node(
            false,
            port,
            rpc_port,
            vec![genesis_peer_addr.clone()],
            &mock_launcher,
            &mut node_registry,
            &mock_rpc_client,
        )
        .await?;

        assert_eq!(multiaddr, node_peer_addr);
        assert_eq!(node_registry.nodes.len(), 2);
        assert!(node_registry.nodes[0].genesis);
        assert_eq!(node_registry.nodes[1].version, "0.100.12");
        assert_eq!(node_registry.nodes[1].service_name, "safenode-local2");
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(PathBuf::from(format!("~/.local/share/safe/{peer_id}")))
        );
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(PathBuf::from(format!("~/.local/share/safe/{peer_id}/logs")))
        );
        assert_eq!(node_registry.nodes[1].number, 2);
        assert_eq!(node_registry.nodes[1].pid, Some(1001));
        assert_eq!(node_registry.nodes[1].port, port);
        assert_eq!(node_registry.nodes[1].rpc_port, rpc_port);
        assert_eq!(node_registry.nodes[1].status, NodeStatus::Running);
        assert_eq!(
            node_registry.nodes[1].safenode_path,
            Some(PathBuf::from("/usr/local/bin/safenode"))
        );

        Ok(())
    }
}
