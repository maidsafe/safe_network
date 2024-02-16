// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    node_control::{
        add,
        config::{AddServiceOptions, InstallNodeServiceCtxBuilder},
        remove, start, stop,
    },
    service::MockServiceControl,
    VerbosityLevel,
};
use assert_fs::prelude::*;
use assert_matches::assert_matches;
use async_trait::async_trait;
use color_eyre::Result;
use libp2p::Multiaddr;
use libp2p_identity::PeerId;
use mockall::{mock, predicate::*, Sequence};
use predicates::prelude::*;
use sn_node_rpc_client::{NetworkInfo, NodeInfo, RecordAddress, Result as RpcResult, RpcActions};
use sn_protocol::node_registry::{Node, NodeRegistry, NodeStatus};
use sn_releases::{
    ArchiveType, Platform, ProgressCallback, ReleaseType, Result as SnReleaseResult,
    SafeReleaseRepositoryInterface,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};

#[cfg(not(target_os = "windows"))]
const SAFENODE_FILE_NAME: &str = "safenode";
#[cfg(target_os = "windows")]
const SAFENODE_FILE_NAME: &str = "safenode.exe";

mock! {
    pub SafeReleaseRepository {}
    #[async_trait]
    impl SafeReleaseRepositoryInterface for SafeReleaseRepository {
        async fn get_latest_version(&self, release_type: &ReleaseType) -> SnReleaseResult<String>;
        async fn download_release_from_s3(
            &self,
            release_type: &ReleaseType,
            version: &str,
            platform: &Platform,
            archive_type: &ArchiveType,
            download_dir: &Path,
            callback: &ProgressCallback
        ) -> SnReleaseResult<PathBuf>;
        async fn download_release(
            &self,
            url: &str,
            dest_dir_path: &Path,
            callback: &ProgressCallback,
        ) -> SnReleaseResult<PathBuf>;
        fn extract_release_archive(&self, archive_path: &Path, extract_dir: &Path) -> SnReleaseResult<PathBuf>;
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
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        faucet_pid: None,
    };

    let mut mock_service_control = MockServiceControl::new();
    let mut seq = Sequence::new();
    mock_service_control
        .expect_get_available_port()
        .times(1)
        .returning(|| Ok(8081))
        .in_sequence(&mut seq);

    let install_ctx = InstallNodeServiceCtxBuilder {
        local: true,
        genesis: true,
        name: "safenode1".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        bootstrap_peers: vec![],
        env_variables: None,
    }
    .execute()?;
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add(
        AddServiceOptions {
            local: true,
            genesis: true,
            count: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            node_port: None,
            bootstrap_peers: vec![],
            rpc_address: None,
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
    assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

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
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![Node {
            genesis: true,
            local: false,
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            version: latest_version.to_string(),
            status: NodeStatus::Added,
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
        faucet_pid: None,
    };

    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("safenode1");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let custom_rpc_address = Ipv4Addr::new(127, 0, 0, 1);

    let result = add(
        AddServiceOptions {
            local: true,
            genesis: true,
            count: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            node_port: None,
            bootstrap_peers: vec![],
            rpc_address: Some(custom_rpc_address),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        faucet_pid: None,
    };

    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("safenode1");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;
    let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
    safenode_download_path.write_binary(b"fake safenode bin")?;

    let custom_rpc_address = Ipv4Addr::new(127, 0, 0, 1);

    let result = add(
        AddServiceOptions {
            local: true,
            genesis: true,
            count: Some(3),
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            node_port: None,
            bootstrap_peers: vec![],
            rpc_address: Some(custom_rpc_address),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        faucet_pid: None,
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
        local: false,
        genesis: false,
        name: "safenode1".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        bootstrap_peers: vec![],
        env_variables: None,
    }
    .execute()?;

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
        local: false,
        genesis: false,
        name: "safenode2".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode2")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
        bootstrap_peers: vec![],
        env_variables: None,
    }
    .execute()?;

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
        local: false,
        genesis: false,
        name: "safenode3".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode3")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8085),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode3"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode3"),
        bootstrap_peers: vec![],
        env_variables: None,
    }
    .execute()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add(
        AddServiceOptions {
            local: false,
            genesis: false,
            count: Some(3),
            bootstrap_peers: vec![],
            node_port: None,
            rpc_address: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
    assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);
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
    assert_matches!(node_registry.nodes[1].status, NodeStatus::Added);
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
    assert_matches!(node_registry.nodes[2].status, NodeStatus::Added);

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
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: old_peers.clone(),
        environment_variables: None,
        faucet_pid: None,
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
        local: false,
        genesis: false,
        name: "safenode1".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        bootstrap_peers: new_peers.clone(),
        env_variables: None,
    }
    .execute()?;
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add(
        AddServiceOptions {
            local: false,
            genesis: false,
            count: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            bootstrap_peers: new_peers.clone(),
            node_port: None,
            rpc_address: Some(Ipv4Addr::new(127, 0, 0, 1)),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
    assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

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
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        faucet_pid: None,
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
        local: false,
        genesis: false,
        name: "safenode1".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        bootstrap_peers: vec![],
        env_variables: env_variables.clone(),
    }
    .execute()?;
    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add(
        AddServiceOptions {
            local: false,
            genesis: false,
            count: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            bootstrap_peers: vec![],
            node_port: None,
            rpc_address: Some(Ipv4Addr::new(127, 0, 0, 1)),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: env_variables.clone(),
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
    assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_new_node_should_add_another_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let latest_version = "0.96.4";
    let mut node_registry = NodeRegistry {
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![Node {
            genesis: true,
            local: false,
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            version: latest_version.to_string(),
            status: NodeStatus::Added,
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
        faucet_pid: None,
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
        local: false,
        genesis: false,
        name: "safenode2".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode2")
            .join(SAFENODE_FILE_NAME),
        node_port: None,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
        bootstrap_peers: vec![],
        env_variables: None,
    }
    .execute()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add(
        AddServiceOptions {
            local: false,
            genesis: false,
            count: None,
            bootstrap_peers: vec![],
            node_port: None,
            rpc_address: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
    assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_node_should_use_custom_ports_for_one_service() -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut mock_service_control = MockServiceControl::new();

    let mut node_registry = NodeRegistry {
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        faucet_pid: None,
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
        local: false,
        genesis: false,
        name: "safenode1".to_string(),
        safenode_path: node_data_dir
            .to_path_buf()
            .join("safenode1")
            .join(SAFENODE_FILE_NAME),
        node_port: Some(custom_port),
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001),
        service_user: get_username(),
        log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
        data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
        bootstrap_peers: vec![],
        env_variables: None,
    }
    .execute()?;

    mock_service_control
        .expect_install()
        .times(1)
        .with(eq(install_ctx))
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    add(
        AddServiceOptions {
            local: false,
            genesis: false,
            count: None,
            safenode_bin_path: safenode_download_path.to_path_buf(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            bootstrap_peers: vec![],
            node_port: Some(custom_port),
            rpc_address: Some(Ipv4Addr::new(127, 0, 0, 1)),
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
    assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

    Ok(())
}

#[tokio::test]
async fn add_node_should_return_error_if_custom_port_is_used_and_more_than_one_service_is_used(
) -> Result<()> {
    let tmp_data_dir = assert_fs::TempDir::new()?;
    let node_reg_path = tmp_data_dir.child("node_reg.json");

    let mut node_registry = NodeRegistry {
        save_path: node_reg_path.to_path_buf(),
        nodes: vec![],
        bootstrap_peers: vec![],
        environment_variables: None,
        faucet_pid: None,
    };
    let latest_version = "0.96.4";
    let temp_dir = assert_fs::TempDir::new()?;
    let node_data_dir = temp_dir.child("data");
    node_data_dir.create_dir_all()?;
    let node_logs_dir = temp_dir.child("logs");
    node_logs_dir.create_dir_all()?;

    let custom_port = 12000;

    let result = add(
        AddServiceOptions {
            local: true,
            genesis: false,
            count: Some(3),
            safenode_bin_path: PathBuf::new(),
            safenode_dir_path: temp_dir.to_path_buf(),
            service_data_dir_path: node_data_dir.to_path_buf(),
            service_log_dir_path: node_logs_dir.to_path_buf(),
            bootstrap_peers: vec![],
            node_port: Some(custom_port),
            rpc_address: None,
            url: None,
            user: get_username(),
            version: latest_version.to_string(),
            env_variables: None,
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
                format!("Custom node port can only be used when adding a single service"),
                e.to_string()
            )
        }
    }

    Ok(())
}

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
async fn start_should_start_a_newly_installed_service() -> Result<()> {
    let mut mock_service_control = MockServiceControl::new();
    let mut mock_rpc_client = MockRpcClient::new();

    mock_service_control
        .expect_start()
        .with(eq("Safenode service 1"))
        .times(1)
        .returning(|_| Ok(()));
    mock_service_control
        .expect_wait()
        .with(eq(3000))
        .times(1)
        .returning(|_| ());
    mock_rpc_client.expect_node_info().times(1).returning(|| {
        Ok(NodeInfo {
            pid: 1000,
            peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
            data_path: PathBuf::from("~/.local/share/safe/service1"),
            log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
            version: "0.98.1".to_string(),
            uptime: std::time::Duration::from_secs(1), // the service was just started
        })
    });
    mock_rpc_client
        .expect_network_info()
        .times(1)
        .returning(|| {
            Ok(NetworkInfo {
                connected_peers: Vec::new(),
                listeners: Vec::new(),
            })
        });

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "Safenode service 1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Added,
        pid: None,
        listen_addr: None,
        peer_id: None,
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };
    start(
        &mut node,
        &mock_service_control,
        &mock_rpc_client,
        VerbosityLevel::Normal,
    )
    .await?;

    assert_eq!(node.pid, Some(1000));
    assert_eq!(
        node.peer_id,
        Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
        )?)
    );
    assert_matches!(node.status, NodeStatus::Running);

    Ok(())
}

#[tokio::test]
async fn start_should_start_a_stopped_service() -> Result<()> {
    let mut mock_service_control = MockServiceControl::new();
    let mut mock_rpc_client = MockRpcClient::new();

    mock_service_control
        .expect_start()
        .with(eq("Safenode service 2"))
        .times(1)
        .returning(|_| Ok(()));
    mock_service_control
        .expect_wait()
        .with(eq(3000))
        .times(1)
        .returning(|_| ());
    mock_rpc_client.expect_node_info().times(1).returning(|| {
        Ok(NodeInfo {
            pid: 1001,
            peer_id: PeerId::from_str("12D3KooWAAqZWsjhdZTX7tniJ7Dwye3nEbp1dx1wE96sbgL51obs")?,
            data_path: PathBuf::from("~/.local/share/safe/service1"),
            log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
            version: "0.98.1".to_string(),
            uptime: std::time::Duration::from_secs(1),
        })
    });
    mock_rpc_client
        .expect_network_info()
        .times(1)
        .returning(|| {
            Ok(NetworkInfo {
                connected_peers: Vec::new(),
                listeners: Vec::new(),
            })
        });

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "Safenode service 2".to_string(),
        user: "safe".to_string(),
        number: 2,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083),
        status: NodeStatus::Stopped,
        pid: Some(1001),
        listen_addr: None,
        peer_id: Some(PeerId::from_str(
            "12D3KooWAAqZWsjhdZTX7tniJ7Dwye3nEbp1dx1wE96sbgL51obs",
        )?),
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };
    start(
        &mut node,
        &mock_service_control,
        &mock_rpc_client,
        VerbosityLevel::Normal,
    )
    .await?;

    assert_matches!(node.status, NodeStatus::Running);
    assert_eq!(node.pid, Some(1001));
    assert_eq!(
        node.peer_id,
        Some(PeerId::from_str(
            "12D3KooWAAqZWsjhdZTX7tniJ7Dwye3nEbp1dx1wE96sbgL51obs"
        )?)
    );

    Ok(())
}

#[tokio::test]
async fn start_should_not_attempt_to_start_a_running_service() -> Result<()> {
    let mut mock_service_control = MockServiceControl::new();
    let mut mock_rpc_client = MockRpcClient::new();

    mock_service_control
        .expect_is_service_process_running()
        .with(eq(1000))
        .times(1)
        .returning(|_| true);
    mock_service_control
        .expect_start()
        .with(eq("Safenode service 1"))
        .times(0)
        .returning(|_| Ok(()));
    mock_rpc_client.expect_node_info().times(0).returning(|| {
        Ok(NodeInfo {
            pid: 1001,
            peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
            data_path: PathBuf::from("~/.local/share/safe/service1"),
            log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
            version: "0.98.1".to_string(),
            uptime: std::time::Duration::from_secs(24 * 60 * 60),
        })
    });

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "Safenode service 1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Running,
        pid: Some(1000),
        listen_addr: None,
        peer_id: Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
        )?),
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };
    start(
        &mut node,
        &mock_service_control,
        &mock_rpc_client,
        VerbosityLevel::Normal,
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn start_should_start_a_service_marked_as_running_but_had_since_stopped() -> Result<()> {
    let mut mock_service_control = MockServiceControl::new();
    let mut mock_rpc_client = MockRpcClient::new();

    mock_service_control
        .expect_is_service_process_running()
        .with(eq(1000))
        .times(1)
        .returning(|_| true);
    mock_service_control
        .expect_start()
        .with(eq("Safenode service 1"))
        .times(0)
        .returning(|_| Ok(()));
    mock_rpc_client.expect_node_info().times(0).returning(|| {
        Ok(NodeInfo {
            pid: 1002,
            peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
            data_path: PathBuf::from("~/.local/share/safe/service1"),
            log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
            version: "0.98.1".to_string(),
            uptime: std::time::Duration::from_secs(1),
        })
    });

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "Safenode service 1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Running,
        listen_addr: None,
        pid: Some(1000),
        peer_id: Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
        )?),
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };
    start(
        &mut node,
        &mock_service_control,
        &mock_rpc_client,
        VerbosityLevel::Normal,
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn stop_should_stop_a_running_service() -> Result<()> {
    let mut mock_service_control = MockServiceControl::new();

    let mut seq = Sequence::new();
    mock_service_control
        .expect_is_service_process_running()
        .with(eq(1000))
        .times(1)
        .returning(|_| true)
        .in_sequence(&mut seq);
    mock_service_control
        .expect_stop()
        .with(eq("Safenode service 1"))
        .times(1)
        .returning(|_| Ok(()))
        .in_sequence(&mut seq);

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "Safenode service 1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Running,
        pid: Some(1000),
        listen_addr: None,
        peer_id: Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
        )?),
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: Some(vec![PeerId::from_str(
            "12D3KooWKbV9vUmZQdHmTwrQqHrqAQpM7GUWHJXeK1xLeh2LVpuc",
        )?]),
    };
    stop(&mut node, &mock_service_control).await?;

    assert_eq!(node.pid, None);
    // The peer ID should be retained on a service stop.
    assert_eq!(
        node.peer_id,
        Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
        )?)
    );
    assert_matches!(node.status, NodeStatus::Stopped);
    assert_matches!(node.connected_peers, None);

    Ok(())
}

#[tokio::test]
async fn stop_should_not_return_error_for_attempt_to_stop_installed_service() -> Result<()> {
    let mock_service_control = MockServiceControl::new();

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "safenode1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Added,
        pid: None,
        listen_addr: None,
        peer_id: None,
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };

    let result = stop(&mut node, &mock_service_control).await;

    match result {
        Ok(()) => Ok(()),
        Err(_) => {
            panic!("The stop command should be idempotent and do nothing for a stopped service");
        }
    }

    // Ok(())
}

#[tokio::test]
async fn stop_should_return_ok_when_attempting_to_stop_service_that_was_already_stopped(
) -> Result<()> {
    let mock_service_control = MockServiceControl::new();

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "Safenode service 1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Stopped,
        pid: None,
        peer_id: None,
        listen_addr: None,
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };

    stop(&mut node, &mock_service_control).await?;

    assert_eq!(node.pid, None);
    assert_matches!(node.status, NodeStatus::Stopped);

    Ok(())
}

#[tokio::test]
async fn remove_should_remove_an_added_node() -> Result<()> {
    let temp_dir = assert_fs::TempDir::new()?;
    let log_dir = temp_dir.child("safenode1-logs");
    log_dir.create_dir_all()?;
    let data_dir = temp_dir.child("safenode1-data");
    data_dir.create_dir_all()?;
    let safenode_bin = data_dir.child("safenode");
    safenode_bin.write_binary(b"fake safenode binary")?;

    let mut mock_service_control = MockServiceControl::new();
    mock_service_control
        .expect_uninstall()
        .with(eq("safenode1"))
        .times(1)
        .returning(|_| Ok(()));

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "safenode1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Stopped,
        pid: None,
        peer_id: None,
        listen_addr: None,
        log_dir_path: log_dir.to_path_buf(),
        data_dir_path: data_dir.to_path_buf(),
        safenode_path: safenode_bin.to_path_buf(),
        connected_peers: None,
    };

    remove(&mut node, &mock_service_control, false).await?;

    assert_matches!(node.status, NodeStatus::Removed);
    log_dir.assert(predicate::path::missing());
    data_dir.assert(predicate::path::missing());

    Ok(())
}

#[tokio::test]
async fn remove_should_return_an_error_if_attempting_to_remove_a_running_node() -> Result<()> {
    let mut mock_service_control = MockServiceControl::new();
    mock_service_control
        .expect_is_service_process_running()
        .with(eq(1000))
        .times(1)
        .returning(|_| true);

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "safenode1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Running,
        pid: Some(1000),
        listen_addr: None,
        peer_id: Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
        )?),
        log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
        data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
        connected_peers: None,
    };

    let result = remove(&mut node, &mock_service_control, false).await;
    match result {
        Ok(_) => panic!("This test should result in an error"),
        Err(e) => assert_eq!("A running node cannot be removed", e.to_string()),
    }

    Ok(())
}

#[tokio::test]
async fn remove_should_return_an_error_for_a_node_that_was_marked_running_but_was_not_actually_running(
) -> Result<()> {
    let temp_dir = assert_fs::TempDir::new()?;
    let log_dir = temp_dir.child("safenode1-logs");
    log_dir.create_dir_all()?;
    let data_dir = temp_dir.child("safenode1-data");
    data_dir.create_dir_all()?;
    let safenode_bin = data_dir.child("safenode");
    safenode_bin.write_binary(b"fake safenode binary")?;

    let mut mock_service_control = MockServiceControl::new();
    mock_service_control
        .expect_is_service_process_running()
        .with(eq(1000))
        .times(1)
        .returning(|_| false);

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "safenode1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Running,
        pid: Some(1000),
        listen_addr: None,
        peer_id: Some(PeerId::from_str(
            "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
        )?),
        log_dir_path: log_dir.to_path_buf(),
        data_dir_path: data_dir.to_path_buf(),
        safenode_path: safenode_bin.to_path_buf(),
        connected_peers: None,
    };

    let result = remove(&mut node, &mock_service_control, false).await;
    match result {
        Ok(_) => panic!("This test should result in an error"),
        Err(e) => assert_eq!(
            "This node was marked as running but it had actually stopped",
            e.to_string()
        ),
    }

    assert_eq!(node.pid, None);
    assert_matches!(node.status, NodeStatus::Stopped);

    Ok(())
}

#[tokio::test]
async fn remove_should_remove_an_added_node_and_keep_directories() -> Result<()> {
    let temp_dir = assert_fs::TempDir::new()?;
    let log_dir = temp_dir.child("safenode1-logs");
    log_dir.create_dir_all()?;
    let data_dir = temp_dir.child("safenode1-data");
    data_dir.create_dir_all()?;
    let safenode_bin = data_dir.child("safenode");
    safenode_bin.write_binary(b"fake safenode binary")?;

    let mut mock_service_control = MockServiceControl::new();
    mock_service_control
        .expect_uninstall()
        .with(eq("safenode1"))
        .times(1)
        .returning(|_| Ok(()));

    let mut node = Node {
        genesis: false,
        local: false,
        version: "0.98.1".to_string(),
        service_name: "safenode1".to_string(),
        user: "safe".to_string(),
        number: 1,
        rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        status: NodeStatus::Stopped,
        pid: None,
        peer_id: None,
        listen_addr: None,
        log_dir_path: log_dir.to_path_buf(),
        data_dir_path: data_dir.to_path_buf(),
        safenode_path: safenode_bin.to_path_buf(),
        connected_peers: None,
    };

    remove(&mut node, &mock_service_control, true).await?;

    assert_eq!(node.data_dir_path, data_dir.to_path_buf());
    assert_eq!(node.log_dir_path, log_dir.to_path_buf());
    assert_matches!(node.status, NodeStatus::Removed);

    log_dir.assert(predicate::path::is_dir());
    data_dir.assert(predicate::path::is_dir());

    Ok(())
}
