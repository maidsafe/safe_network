// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    add_services::{
        add_daemon, add_faucet, add_node,
        config::{
            AddDaemonServiceOptions, AddFaucetServiceOptions, AddNodeServiceOptions,
            InstallNodeServiceCtxBuilder, PortRange,
        },
    },
    VerbosityLevel,
};
use assert_fs::prelude::*;
use assert_matches::assert_matches;
use color_eyre::Result;
use libp2p::Multiaddr;
use mockall::{mock, predicate::*, Sequence};
use predicates::prelude::*;
use service_manager::ServiceInstallCtx;
use sn_service_management::control::ServiceControl;
use sn_service_management::error::Result as ServiceControlResult;
use sn_service_management::{
    DaemonServiceData, FaucetServiceData, NodeRegistry, NodeServiceData, ServiceStatus,
};
use std::{
    ffi::OsString,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};

#[cfg(not(target_os = "windows"))]
const SAFENODE_FILE_NAME: &str = "safenode";
#[cfg(target_os = "windows")]
const SAFENODE_FILE_NAME: &str = "safenode.exe";
#[cfg(not(target_os = "windows"))]
const FAUCET_FILE_NAME: &str = "faucet";
#[cfg(target_os = "windows")]
const FAUCET_FILE_NAME: &str = "faucet.exe";
#[cfg(not(target_os = "windows"))]
const DAEMON_FILE_NAME: &str = "safenodemand";
#[cfg(target_os = "windows")]
const DAEMON_FILE_NAME: &str = "safenodemand.exe";

mock! {
    pub ServiceControl {}
    impl ServiceControl for ServiceControl {
        fn create_service_user(&self, username: &str) -> ServiceControlResult<()>;
        fn get_available_port(&self) -> ServiceControlResult<u16>;
        fn install(&self, install_ctx: ServiceInstallCtx) -> ServiceControlResult<()>;
        fn get_process_pid(&self, bin_path: &Path) -> ServiceControlResult<u32>;
        fn is_service_process_running(&self, pid: u32) -> bool;
        fn start(&self, service_name: &str) -> ServiceControlResult<()>;
        fn stop(&self, service_name: &str) -> ServiceControlResult<()>;
        fn uninstall(&self, service_name: &str) -> ServiceControlResult<()>;
        fn wait(&self, delay: u64);
    }
}

#[cfg(target_os = "windows")]
fn get_username() -> String {
    std::env::var("USERNAME").expect("Failed to get username")
}

#[cfg(not(target_os = "windows"))]
fn get_username() -> String {
    std::env::var("USER").expect("Failed to get username")
}

#[tokio::test]
async fn add_genesis_node_should_use_latest_version_and_add_one_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };

    let mut mock_service_control = MockServiceControl::new();
    let mut seq = Sequence::new();
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(8081))
        .in_sequence(&mut seq);

    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: vec![],
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        env_variables: None,
        genesis: true,
        local: true,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        metrics_port: None,
        name: "safenode1".to_string(),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: None,
            env_variables: None,
            genesis: true,
            local: true,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());

    node_reg_path.assert(predicates::path::is_file());
    assert_eq!(node_registry.nodes.len(), 1);
    assert!(node_registry.nodes[0].genesis);
    assert_eq!(node_registry.nodes[0].version, latest_version);
    assert_eq!(node_registry.nodes[0].service_name, "safenode1");
    assert_eq!(node_registry.nodes[0].user, get_username());
    assert_eq!(node_registry.nodes[0].number, 1);
    assert_eq!(
        node_registry.nodes[0].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081)
    );
    assert_eq!(
        node_registry.nodes[0].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode1")
    );
    assert_eq!(
        node_registry.nodes[0].data_dir_path,
        node_data_dir.to_path_buf().join("safenode1")
    );
    assert_matches!(node_registry.nodes[0].status, ServiceStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_genesis_node_should_return_an_error_if_there_is_already_a_genesis_node() -> Result<()>
{
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mock_service_control = MockServiceControl::new();

    let latest_version = "0.96.4";
    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![NodeServiceData {
            genesis: true,
            local: false,
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            version: latest_version.to_string(),
            status: ServiceStatus::Added,
            listen_addr: None,
            pid: None,
            peer_id: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            connected_peers: None,
        }],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };

    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("safenode1");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let custom_rpc_address = Ipv4Addr::new(127, 0, 0, 1);

    let result = add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: None,
            env_variables: None,
            genesis: true,
            local: true,
            metrics_port: None,
            node_port: None,
            rpc_address: Some(custom_rpc_address),
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await;

    assert_eq!(
        Err("A genesis node already exists".to_string()),
        result.map_err(|e| e.to_string())
    );

    Ok(())
}

#[tokio::test]
async fn add_genesis_node_should_return_an_error_if_count_is_greater_than_1() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("safenode1");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let result = add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(3),
            env_variables: None,
            genesis: true,
            local: true,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await;

    assert_eq!(
        Err("A genesis node can only be added as a single node".to_string()),
        result.map_err(|e| e.to_string())
    );

    Ok(())
}

#[tokio::test]
async fn add_node_should_use_latest_version_and_add_three_services() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();

    // Expected calls for first installation
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(8081))
        .in_sequence(&mut seq);

    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: vec![],
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        env_variables: None,
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        metrics_port: None,
        name: "safenode1".to_string(),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Expected calls for second installation
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(8083))
        .in_sequence(&mut seq);
    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: vec![],
        data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
        env_variables: None,
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
        metrics_port: None,
        name: "safenode2".to_string(),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode2")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Expected calls for third installation
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(8085))
        .in_sequence(&mut seq);
    let install_ctx = InstallNodeServiceCtxBuilder {
        data_dir_path: node_data_dir.to_path_buf().join("safenode3"),
        bootstrap_peers: vec![],
        env_variables: None,
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode3"),
        metrics_port: None,
        name: "safenode3".to_string(),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8085),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode3")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(3),
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    assert_eq!(node_registry.nodes.len(), 3);
    assert_eq!(node_registry.nodes[0].version, latest_version);
    assert_eq!(node_registry.nodes[0].service_name, "safenode1");
    assert_eq!(node_registry.nodes[0].user, get_username());
    assert_eq!(node_registry.nodes[0].number, 1);
    assert_eq!(
        node_registry.nodes[0].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081)
    );
    assert_eq!(
        node_registry.nodes[0].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode1")
    );
    assert_eq!(
        node_registry.nodes[0].data_dir_path,
        node_data_dir.to_path_buf().join("safenode1")
    );
    assert_matches!(node_registry.nodes[0].status, ServiceStatus::Added);
    assert_eq!(node_registry.nodes[1].version, latest_version);
    assert_eq!(node_registry.nodes[1].service_name, "safenode2");
    assert_eq!(node_registry.nodes[1].user, get_username());
    assert_eq!(node_registry.nodes[1].number, 2);
    assert_eq!(
        node_registry.nodes[1].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083)
    );
    assert_eq!(
        node_registry.nodes[1].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode2")
    );
    assert_eq!(
        node_registry.nodes[1].data_dir_path,
        node_data_dir.to_path_buf().join("safenode2")
    );
    assert_matches!(node_registry.nodes[1].status, ServiceStatus::Added);
    assert_eq!(node_registry.nodes[2].version, latest_version);
    assert_eq!(node_registry.nodes[2].service_name, "safenode3");
    assert_eq!(node_registry.nodes[2].user, get_username());
    assert_eq!(node_registry.nodes[2].number, 3);
    assert_eq!(
        node_registry.nodes[2].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8085)
    );
    assert_eq!(
        node_registry.nodes[2].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode3")
    );
    assert_eq!(
        node_registry.nodes[2].data_dir_path,
        node_data_dir.to_path_buf().join("safenode3")
    );
    assert_matches!(node_registry.nodes[2].status, ServiceStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_node_should_update_the_bootstrap_peers_inside_node_registry() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut old_peers  = vec![Multiaddr::from_str("/ip4/64.227.35.186/udp/33188/quic-v1/p2p/12D3KooWDrx4zfUuJgz7jSusC28AZRDRbj7eo3WKZigPsw9tVKs3")?];
    let new_peers = vec![Multiaddr::from_str("/ip4/178.62.78.116/udp/45442/quic-v1/p2p/12D3KooWLH4E68xFqoSKuF2JPQQhzaAg7GNvN1vpxoLMgJq6Zqz8")?];

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: old_peers.clone(),
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();

    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(12001))
        .in_sequence(&mut seq);

    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: new_peers.clone(),
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        env_variables: None,
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        metrics_port: None,
        name: "safenode1".to_string(),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: new_peers.clone(),
            count: None,
            env_variables: None,
            local: false,
            genesis: false,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());

    old_peers.extend(new_peers);
    assert_eq!(node_registry.bootstrap_peers, old_peers);

    assert_eq!(node_registry.nodes.len(), 1);
    assert_eq!(node_registry.nodes[0].version, latest_version);
    assert_eq!(node_registry.nodes[0].service_name, "safenode1");
    assert_eq!(node_registry.nodes[0].user, get_username());
    assert_eq!(node_registry.nodes[0].number, 1);
    assert_eq!(
        node_registry.nodes[0].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001)
    );
    assert_eq!(
        node_registry.nodes[0].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode1")
    );
    assert_eq!(
        node_registry.nodes[0].data_dir_path,
        node_data_dir.to_path_buf().join("safenode1")
    );
    assert_matches!(node_registry.nodes[0].status, ServiceStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_node_should_update_the_environment_variables_inside_node_registry() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let env_variables = Some(vec![
        ("SN_LOG".to_owned(), "all".to_owned()),
        ("RUST_LOG".to_owned(), "libp2p=debug".to_owned()),
    ]);

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();

    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(12001))
        .in_sequence(&mut seq);
    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: vec![],
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        env_variables: env_variables.clone(),
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        metrics_port: None,
        name: "safenode1".to_string(),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: None,
            env_variables: env_variables.clone(),
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());

    assert_eq!(node_registry.environment_variables, env_variables);

    assert_eq!(node_registry.nodes.len(), 1);
    assert_eq!(node_registry.nodes[0].version, latest_version);
    assert_eq!(node_registry.nodes[0].service_name, "safenode1");
    assert_eq!(node_registry.nodes[0].user, get_username());
    assert_eq!(node_registry.nodes[0].number, 1);
    assert_eq!(
        node_registry.nodes[0].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001)
    );
    assert_eq!(
        node_registry.nodes[0].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode1")
    );
    assert_eq!(
        node_registry.nodes[0].data_dir_path,
        node_data_dir.to_path_buf().join("safenode1")
    );
    assert_matches!(node_registry.nodes[0].status, ServiceStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_new_node_should_add_another_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let latest_version = "0.96.4";
    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![NodeServiceData {
            genesis: true,
            local: false,
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            version: latest_version.to_string(),
            status: ServiceStatus::Added,
            pid: None,
            peer_id: None,
            listen_addr: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            connected_peers: None,
        }],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("safenode1");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(8083))
        .in_sequence(&mut seq);
    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: vec![],
        data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
        env_variables: None,
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
        metrics_port: None,
        name: "safenode2".to_string(),
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083),
        node_port: None,
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode2")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: None,
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_src_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    assert_eq!(node_registry.nodes.len(), 2);
    assert_eq!(node_registry.nodes[1].version, latest_version);
    assert_eq!(node_registry.nodes[1].service_name, "safenode2");
    assert_eq!(node_registry.nodes[1].user, get_username());
    assert_eq!(node_registry.nodes[1].number, 2);
    assert_eq!(
        node_registry.nodes[1].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083)
    );
    assert_eq!(
        node_registry.nodes[1].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode2")
    );
    assert_eq!(
        node_registry.nodes[1].data_dir_path,
        node_data_dir.to_path_buf().join("safenode2")
    );
    assert_matches!(node_registry.nodes[0].status, ServiceStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_node_should_use_custom_ports_for_one_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let custom_port = 12000;

    let mut seq = Sequence::new();

    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(12001))
        .in_sequence(&mut seq);
    let install_ctx = InstallNodeServiceCtxBuilder {
        bootstrap_peers: vec![],
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        env_variables: None,
        genesis: false,
        local: false,
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        metrics_port: None,
        name: "safenode1".to_string(),
        node_port: Some(custom_port),
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        service_user: get_username(),
    }
    .build()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: None,
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: Some(PortRange::Single(custom_port)),
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());

    assert_eq!(node_registry.nodes.len(), 1);
    assert_eq!(node_registry.nodes[0].version, latest_version);
    assert_eq!(node_registry.nodes[0].service_name, "safenode1");
    assert_eq!(node_registry.nodes[0].user, get_username());
    assert_eq!(node_registry.nodes[0].number, 1);
    assert_eq!(
        node_registry.nodes[0].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001)
    );
    assert_eq!(
        node_registry.nodes[0].log_dir_path,
        node_logs_dir.to_path_buf().join("safenode1")
    );
    assert_eq!(
        node_registry.nodes[0].data_dir_path,
        node_data_dir.to_path_buf().join("safenode1")
    );
    assert_matches!(node_registry.nodes[0].status, ServiceStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_node_should_use_a_custom_port_range() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();

    // First service
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(15000))
        .in_sequence(&mut seq);
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:15000"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode1")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode1")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--port"),
                OsString::from("12000"),
            ],
            contents: None,
            environment: None,
            label: "safenode1".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode1")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Second service
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(15001))
        .in_sequence(&mut seq);
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:15001"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode2")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode2")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--port"),
                OsString::from("12001"),
            ],
            contents: None,
            environment: None,
            label: "safenode2".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode2")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Third service
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(15002))
        .in_sequence(&mut seq);
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:15002"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode3")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode3")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--port"),
                OsString::from("12002"),
            ],
            contents: None,
            environment: None,
            label: "safenode3".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode3")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(3),
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: Some(PortRange::Range(12000, 12002)),
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());
    assert_eq!(node_registry.nodes.len(), 3);

    Ok(())
}

#[tokio::test]
async fn add_node_should_return_an_error_if_port_and_node_count_do_not_match() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let result = add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(2),
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: Some(PortRange::Range(12000, 12002)),
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &MockServiceControl::new(),
        VerbosityLevel::Normal,
    )
    .await;

    match result {
        Ok(_) => panic!("This test should result in an error"),
        Err(e) => {
            assert_eq!(
                format!("The number of services to add (2) does not match the number of ports (3)"),
                e.to_string()
            )
        }
    }

    Ok(())
}

#[tokio::test]
async fn add_node_should_return_an_error_if_multiple_services_are_specified_with_a_single_port(
) -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let result = add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(2),
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: Some(PortRange::Single(12000)),
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &MockServiceControl::new(),
        VerbosityLevel::Normal,
    )
    .await;

    match result {
        Ok(_) => panic!("This test should result in an error"),
        Err(e) => {
            assert_eq!(
                format!("The number of services to add (2) does not match the number of ports (1)"),
                e.to_string()
            )
        }
    }

    Ok(())
}

#[tokio::test]
async fn add_node_should_use_a_custom_port_range_for_metrics_server() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();

    // First service
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(15000))
        .in_sequence(&mut seq);
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:15000"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode1")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode1")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--metrics-server-port"),
                OsString::from("12000"),
            ],
            contents: None,
            environment: None,
            label: "safenode1".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode1")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Second service
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(15001))
        .in_sequence(&mut seq);
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:15001"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode2")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode2")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--metrics-server-port"),
                OsString::from("12001"),
            ],
            contents: None,
            environment: None,
            label: "safenode2".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode2")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Third service
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(15002))
        .in_sequence(&mut seq);
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:15002"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode3")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode3")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--metrics-server-port"),
                OsString::from("12002"),
            ],
            contents: None,
            environment: None,
            label: "safenode3".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode3")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(3),
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: Some(PortRange::Range(12000, 12002)),
            node_port: None,
            rpc_address: None,
            rpc_port: None,
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());
    assert_eq!(node_registry.nodes.len(), 3);

    Ok(())
}

#[tokio::test]
async fn add_node_should_use_a_custom_port_range_for_the_rpc_server() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        faucet: None,
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        daemon: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let mut seq = Sequence::new();

    // First service
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:20000"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode1")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode1")
                        .to_string_lossy()
                        .to_string(),
                ),
            ],
            contents: None,
            environment: None,
            label: "safenode1".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode1")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Second service
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:20001"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode2")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode2")
                        .to_string_lossy()
                        .to_string(),
                ),
            ],
            contents: None,
            environment: None,
            label: "safenode2".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode2")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    // Third service
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--rpc"),
                OsString::from("127.0.0.1:20002"),
                OsString::from("--root-dir"),
                OsString::from(
                    node_data_dir
                        .to_path_buf()
                        .join("safenode3")
                        .to_string_lossy()
                        .to_string(),
                ),
                OsString::from("--log-output-dest"),
                OsString::from(
                    node_logs_dir
                        .to_path_buf()
                        .join("safenode3")
                        .to_string_lossy()
                        .to_string(),
                ),
            ],
            contents: None,
            environment: None,
            label: "safenode3".parse()?,
            program: node_data_dir
                .to_path_buf()
                .join("safenode3")
                .join(SAFENODE_FILE_NAME),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add_node(
        AddNodeServiceOptions {
            bootstrap_peers: vec![],
            count: Some(3),
            env_variables: None,
            genesis: false,
            local: false,
            metrics_port: None,
            node_port: None,
            rpc_address: None,
            rpc_port: Some(PortRange::Range(20000, 20002)),
            safenode_dir_path: temp_dir.to_path_buf(),
            safenode_src_path: safenode_download_path.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )
    .await?;

    safenode_download_path.assert(predicate::path::missing());
    node_data_dir.assert(predicate::path::is_dir());
    node_logs_dir.assert(predicate::path::is_dir());
    assert_eq!(node_registry.nodes.len(), 3);
    assert_eq!(
        node_registry.nodes[0].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 20000)
    );
    assert_eq!(
        node_registry.nodes[1].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 20001)
    );
    assert_eq!(
        node_registry.nodes[2].rpc_socket_addr,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 20002)
    );
    Ok(())
}

#[tokio::test]
async fn add_faucet_should_add_a_faucet_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let faucet_logs_dir = temp_dir.child("logs");
    faucet_logs_dir.create_dir_all()?;
    let faucet_data_dir = temp_dir.child("data");
    faucet_data_dir.create_dir_all()?;
    let faucet_install_dir = temp_dir.child("install");
    faucet_install_dir.create_dir_all()?;
    let faucet_install_path = faucet_install_dir.child(FAUCET_FILE_NAME);
    let faucet_download_path = temp_dir.child(FAUCET_FILE_NAME);
    faucet_download_path.write_binary(b"fake faucet bin")?;

    let mut node_registry = NodeRegistry {
        bootstrap_peers: vec![],
        daemon: None,
        faucet: None,
        environment_variables: None,
        nodes: vec![],
        save_path: node_reg_path.to_path_buf(),
    };

    let mut mock_service_control = MockServiceControl::new();

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--log-output-dest"),
                OsString::from(faucet_logs_dir.to_path_buf().as_os_str()),
                OsString::from("server"),
            ],
            contents: None,
            environment: Some(vec![("SN_LOG".to_string(), "all".to_string())]),
            label: "faucet".parse()?,
            program: faucet_install_path.to_path_buf(),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()));

    add_faucet(
        AddFaucetServiceOptions {
            bootstrap_peers: vec![],
            env_variables: Some(vec![("SN_LOG".to_string(), "all".to_string())]),
            faucet_src_bin_path: faucet_download_path.to_path_buf(),
            faucet_install_bin_path: faucet_install_path.to_path_buf(),
            local: false,
            service_data_dir_path: faucet_data_dir.to_path_buf(),
            service_log_dir_path: faucet_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
        VerbosityLevel::Normal,
    )?;

    faucet_download_path.assert(predicate::path::missing());
    faucet_install_path.assert(predicate::path::is_file());
    faucet_logs_dir.assert(predicate::path::is_dir());

    node_reg_path.assert(predicates::path::is_file());

    let saved_faucet = node_registry.faucet.unwrap();
    assert_eq!(saved_faucet.faucet_path, faucet_install_path.to_path_buf());
    assert!(!saved_faucet.local);
    assert_eq!(saved_faucet.log_dir_path, faucet_logs_dir.to_path_buf());
    assert!(saved_faucet.pid.is_none());
    assert_eq!(saved_faucet.service_name, "faucet");
    assert_eq!(saved_faucet.status, ServiceStatus::Added);
    assert_eq!(saved_faucet.user, get_username());
    assert_eq!(saved_faucet.version, latest_version);

    Ok(())
}

#[tokio::test]
async fn add_faucet_should_return_an_error_if_a_faucet_service_was_already_created() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let faucet_logs_dir = temp_dir.child("logs");
    faucet_logs_dir.create_dir_all()?;
    let faucet_data_dir = temp_dir.child("data");
    faucet_data_dir.create_dir_all()?;
    let faucet_install_dir = temp_dir.child("install");
    faucet_install_dir.create_dir_all()?;
    let faucet_install_path = faucet_install_dir.child(FAUCET_FILE_NAME);
    let faucet_download_path = temp_dir.child(FAUCET_FILE_NAME);
    faucet_download_path.write_binary(b"fake faucet bin")?;

    let mut node_registry = NodeRegistry {
        bootstrap_peers: vec![],
        daemon: None,
        faucet: Some(FaucetServiceData {
            faucet_path: faucet_download_path.to_path_buf(),
            local: false,
            log_dir_path: PathBuf::from("/var/log/faucet"),
            pid: Some(1000),
            service_name: "faucet".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: latest_version.to_string(),
        }),
        environment_variables: None,
        nodes: vec![],
        save_path: node_reg_path.to_path_buf(),
    };

    let result = add_faucet(
        AddFaucetServiceOptions {
            bootstrap_peers: vec![],
            env_variables: Some(vec![("SN_LOG".to_string(), "all".to_string())]),
            faucet_src_bin_path: faucet_download_path.to_path_buf(),
            faucet_install_bin_path: faucet_install_path.to_path_buf(),
            local: false,
            service_data_dir_path: faucet_data_dir.to_path_buf(),
            service_log_dir_path: faucet_logs_dir.to_path_buf(),
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &MockServiceControl::new(),
        VerbosityLevel::Normal,
    );

    match result {
        Ok(_) => panic!("This test should result in an error"),
        Err(e) => {
            assert_eq!(
                format!("A faucet service has already been created"),
                e.to_string()
            )
        }
    }

    Ok(())
}

#[tokio::test]
async fn add_daemon_should_add_a_daemon_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let daemon_install_dir = temp_dir.child("install");
    daemon_install_dir.create_dir_all()?;
    let daemon_install_path = daemon_install_dir.child(DAEMON_FILE_NAME);
    let daemon_download_path = temp_dir.child(DAEMON_FILE_NAME);
    daemon_download_path.write_binary(b"fake daemon bin")?;

    let mut node_registry = NodeRegistry {
        bootstrap_peers: vec![],
        daemon: None,
        faucet: None,
        environment_variables: None,
        nodes: vec![],
        save_path: node_reg_path.to_path_buf(),
    };

    let mut mock_service_control = MockServiceControl::new();

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(ServiceInstallCtx {
            args: vec![
                OsString::from("--port"),
                OsString::from("8080"),
                OsString::from("--address"),
                OsString::from("127.0.0.1"),
            ],
            contents: None,
            environment: Some(vec![("SN_LOG".to_string(), "ALL".to_string())]),
            label: "safenodemand".parse()?,
            program: daemon_install_path.to_path_buf(),
            username: Some(get_username()),
            working_directory: None,
        }))
        .returning(|_| Ok(()));

    add_daemon(
        AddDaemonServiceOptions {
            address: Ipv4Addr::new(127, 0, 0, 1),
            daemon_install_bin_path: daemon_install_path.to_path_buf(),
            daemon_src_bin_path: daemon_download_path.to_path_buf(),
            env_variables: Some(vec![("SN_LOG".to_string(), "ALL".to_string())]),
            port: 8080,
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &mock_service_control,
    )?;

    daemon_download_path.assert(predicate::path::missing());
    daemon_install_path.assert(predicate::path::is_file());

    node_reg_path.assert(predicates::path::is_file());

    let saved_daemon = node_registry.daemon.unwrap();
    assert_eq!(saved_daemon.daemon_path, daemon_install_path.to_path_buf());
    assert!(saved_daemon.pid.is_none());
    assert_eq!(saved_daemon.service_name, "safenodemand");
    assert_eq!(saved_daemon.status, ServiceStatus::Added);
    assert_eq!(saved_daemon.version, latest_version);

    Ok(())
}

#[tokio::test]
async fn add_daemon_should_return_an_error_if_a_daemon_service_was_already_created() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let daemon_install_dir = temp_dir.child("install");
    daemon_install_dir.create_dir_all()?;
    let daemon_install_path = daemon_install_dir.child(DAEMON_FILE_NAME);
    let daemon_download_path = temp_dir.child(DAEMON_FILE_NAME);
    daemon_download_path.write_binary(b"fake daemon bin")?;

    let mut node_registry = NodeRegistry {
        bootstrap_peers: vec![],
        daemon: Some(DaemonServiceData {
            daemon_path: PathBuf::from("/usr/local/bin/safenodemand"),
            endpoint: Some(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                8080,
            )),
            pid: Some(1234),
            service_name: "safenodemand".to_string(),
            status: ServiceStatus::Running,
            version: latest_version.to_string(),
        }),
        faucet: None,
        environment_variables: None,
        nodes: vec![],
        save_path: node_reg_path.to_path_buf(),
    };

    let result = add_daemon(
        AddDaemonServiceOptions {
            address: Ipv4Addr::new(127, 0, 0, 1),
            daemon_install_bin_path: daemon_install_path.to_path_buf(),
            daemon_src_bin_path: daemon_download_path.to_path_buf(),
            env_variables: Some(Vec::new()),
            port: 8080,
            user: get_username(),
            version: latest_version.to_string(),
        },
        &mut node_registry,
        &MockServiceControl::new(),
    );

    match result {
        Ok(_) => panic!("This test should result in an error"),
        Err(e) => {
            assert_eq!(
                format!("A safenodemand service has already been created"),
                e.to_string()
            )
        }
    }

    Ok(())
}
