// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    config::create_owned_dir,
    service::{InstallNodeServiceConfig, ServiceControl},
    VerbosityLevel,
};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use libp2p::Multiaddr;
use sn_protocol::node_registry::{Node, NodeRegistry, NodeStatus};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

/// This is just a set of config parameters that is used inside the `add()` function.
pub struct AddServiceOptions {
    pub count: Option<u16>,
    pub genesis: bool,
    pub local: bool,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub node_port: Option<u16>,
    pub rpc_address: Option<Ipv4Addr>,
    pub safenode_bin_path: PathBuf,
    pub safenode_dir_path: PathBuf,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub url: Option<String>,
    pub user: String,
    pub version: String,
    pub env_variables: Option<Vec<(String, String)>>,
}

/// Install safenode as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub async fn add(
    options: AddServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if options.genesis {
        if let Some(count) = options.count {
            if count > 1 {
                return Err(eyre!("A genesis node can only be added as a single node"));
            }
        }

        let genesis_node = node_registry.nodes.iter().find(|n| n.genesis);
        if genesis_node.is_some() {
            return Err(eyre!("A genesis node already exists"));
        }
    }

    if options.count.is_some() && options.node_port.is_some() {
        let count = options.count.unwrap();
        if count > 1 {
            return Err(eyre!(
                "Custom node port can only be used when adding a single service"
            ));
        }
    }

    let safenode_file_name = options
        .safenode_bin_path
        .file_name()
        .ok_or_else(|| eyre!("Could not get filename from the safenode download path"))?
        .to_string_lossy()
        .to_string();

    //  store the bootstrap peers and the provided env variable.
    {
        let mut should_save = false;
        let new_bootstrap_peers: Vec<_> = options
            .bootstrap_peers
            .iter()
            .filter(|peer| !node_registry.bootstrap_peers.contains(peer))
            .collect();
        if !new_bootstrap_peers.is_empty() {
            node_registry
                .bootstrap_peers
                .extend(new_bootstrap_peers.into_iter().cloned());
            should_save = true;
        }

        if options.env_variables.is_some() {
            node_registry.environment_variables = options.env_variables.clone();
            should_save = true;
        }

        if should_save {
            node_registry.save()?;
        }
    }

    let mut added_service_data = vec![];
    let mut failed_service_data = vec![];

    let current_node_count = node_registry.nodes.len() as u16;
    let target_node_count = current_node_count + options.count.unwrap_or(1);

    let mut node_number = current_node_count + 1;
    while node_number <= target_node_count {
        let rpc_free_port = service_control.get_available_port()?;
        let rpc_socket_addr = if let Some(addr) = options.rpc_address {
            SocketAddr::new(IpAddr::V4(addr), rpc_free_port)
        } else {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), rpc_free_port)
        };

        let service_name = format!("safenode{node_number}");
        let service_data_dir_path = options.service_data_dir_path.join(service_name.clone());
        let service_safenode_path = service_data_dir_path.join(safenode_file_name.clone());
        let service_log_dir_path = options.service_log_dir_path.join(service_name.clone());

        create_owned_dir(service_data_dir_path.clone(), &options.user)?;
        create_owned_dir(service_log_dir_path.clone(), &options.user)?;

        std::fs::copy(
            options.safenode_bin_path.clone(),
            service_safenode_path.clone(),
        )?;
        let install_ctx = InstallNodeServiceConfig {
            local: options.local,
            data_dir_path: service_data_dir_path.clone(),
            genesis: options.genesis,
            log_dir_path: service_log_dir_path.clone(),
            name: service_name.clone(),
            node_port: options.node_port,
            bootstrap_peers: options.bootstrap_peers.clone(),
            rpc_socket_addr,
            safenode_path: service_safenode_path.clone(),
            service_user: options.user.clone(),
            env_variables: options.env_variables.clone(),
        }
        .build_service_install_ctx()?;

        match service_control.install(install_ctx) {
            Ok(()) => {
                added_service_data.push((
                    service_name.clone(),
                    service_safenode_path.to_string_lossy().into_owned(),
                    service_data_dir_path.to_string_lossy().into_owned(),
                    service_log_dir_path.to_string_lossy().into_owned(),
                    rpc_socket_addr,
                ));

                node_registry.nodes.push(Node {
                    genesis: options.genesis,
                    local: options.local,
                    service_name,
                    user: options.user.clone(),
                    number: node_number,
                    rpc_socket_addr,
                    version: options.version.clone(),
                    status: NodeStatus::Added,
                    listen_addr: None,
                    pid: None,
                    peer_id: None,
                    log_dir_path: service_log_dir_path.clone(),
                    data_dir_path: service_data_dir_path.clone(),
                    safenode_path: service_safenode_path,
                    connected_peers: None,
                });
                // We save the node registry for each service because it's possible any number of
                // services could fail to be added.
                node_registry.save()?;
            }
            Err(e) => {
                failed_service_data.push((service_name.clone(), e.to_string()));
            }
        }

        node_number += 1;
    }

    std::fs::remove_file(options.safenode_bin_path)?;

    if !added_service_data.is_empty() {
        println!("Services Added:");
        for install in added_service_data.iter() {
            println!(" {} {}", "✓".green(), install.0);
            if verbosity != VerbosityLevel::Minimal {
                println!("    - Safenode path: {}", install.1);
                println!("    - Data path: {}", install.2);
                println!("    - Log path: {}", install.3);
                println!("    - RPC port: {}", install.4);
            }
        }
        println!("[!] Note: newly added services have not been started");
    }

    if !failed_service_data.is_empty() {
        println!("Failed to add {} service(s):", failed_service_data.len());
        for failed in failed_service_data.iter() {
            println!("{} {}: {}", "✕".red(), failed.0, failed.1);
        }
        return Err(eyre!("Failed to add one or more services")
            .suggestion("However, any services that were successfully added will be usable."));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::MockServiceControl;
    use assert_fs::prelude::*;
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use mockall::{mock, predicate::*, Sequence};
    use predicates::prelude::*;
    use sn_releases::{
        ArchiveType, Platform, ProgressCallback, ReleaseType, Result as SnReleaseResult,
        SafeReleaseRepositoryInterface,
    };
    use std::{path::Path, str::FromStr};

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

        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;
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
    async fn add_genesis_node_should_return_an_error_if_there_is_already_a_genesis_node(
    ) -> Result<()> {
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

        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;

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
        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;

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
        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;

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

        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;
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
        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;
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
        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;

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
        let install_ctx = InstallNodeServiceConfig {
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
        .build_service_install_ctx()?;

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
}
