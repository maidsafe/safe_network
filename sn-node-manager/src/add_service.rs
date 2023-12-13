// Copyright (C) 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::config::create_owned_dir;
use crate::helpers::download_and_extract_safenode;
use crate::node::{Node, NodeRegistry, NodeStatus};
use crate::service::{ServiceConfig, ServiceControl};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use libp2p::Multiaddr;
use sn_releases::SafeReleaseRepositoryInterface;
use std::path::PathBuf;

pub struct AddServiceOptions {
    pub count: Option<u16>,
    pub safenode_dir_path: PathBuf,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub peers: Vec<Multiaddr>,
    pub port: Option<u16>,
    pub rpc_port: Option<u16>,
    pub url: Option<String>,
    pub user: String,
    pub version: Option<String>,
}

/// Install safenode as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub async fn add(
    install_options: AddServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    release_repo: Box<dyn SafeReleaseRepositoryInterface>,
) -> Result<()> {
    if install_options.count.is_some()
        && (install_options.port.is_some() || install_options.rpc_port.is_some())
    {
        let count = install_options.count.unwrap();
        if count > 1 {
            return Err(eyre!(
                "Custom ports can only be used when adding a single service"
            ));
        }
    }

    if install_options.port.is_some() {
        let port = install_options.port.unwrap();
        if !service_control.is_port_free(port) {
            return Err(eyre!("Port {port} is already in use")
                .suggestion("Please try again with an available port"));
        }
    }
    if install_options.rpc_port.is_some() {
        let rpc_port = install_options.rpc_port.unwrap();
        if !service_control.is_port_free(rpc_port) {
            return Err(eyre!("Port {rpc_port} is already in use")
                .suggestion("Please try again with an available port"));
        }
    }

    let (safenode_download_path, version) =
        download_and_extract_safenode(install_options.url, install_options.version, release_repo)
            .await?;
    let safenode_file_name = safenode_download_path
        .file_name()
        .ok_or_else(|| eyre!("Could not get filename from the safenode download path"))?
        .to_string_lossy()
        .to_string();

    let mut added_service_data = vec![];
    let current_node_count = node_registry.nodes.len() as u16;
    let target_node_count = current_node_count + install_options.count.unwrap_or(1);
    let mut node_number = current_node_count + 1;
    while node_number <= target_node_count {
        let node_port = if let Some(port) = install_options.port {
            port
        } else {
            service_control.get_available_port()?
        };
        let rpc_port = if let Some(port) = install_options.rpc_port {
            port
        } else {
            service_control.get_available_port()?
        };

        let service_name = format!("safenode{node_number}");
        let service_data_dir_path = install_options
            .service_data_dir_path
            .join(service_name.clone());
        let service_safenode_path = service_data_dir_path.join(safenode_file_name.clone());
        let service_log_dir_path = install_options
            .service_log_dir_path
            .join(service_name.clone());

        create_owned_dir(service_data_dir_path.clone(), &install_options.user)?;
        create_owned_dir(service_log_dir_path.clone(), &install_options.user)?;

        std::fs::copy(
            safenode_download_path.clone(),
            service_safenode_path.clone(),
        )?;

        service_control.install(ServiceConfig {
            name: service_name.clone(),
            safenode_path: service_safenode_path.clone(),
            node_port,
            rpc_port,
            service_user: install_options.user.clone(),
            log_dir_path: service_log_dir_path.clone(),
            data_dir_path: service_data_dir_path.clone(),
            peers: install_options.peers.clone(),
        })?;

        added_service_data.push((
            service_name.clone(),
            service_safenode_path.to_string_lossy().into_owned(),
            service_data_dir_path.to_string_lossy().into_owned(),
            service_log_dir_path.to_string_lossy().into_owned(),
            node_port,
            rpc_port,
        ));

        node_registry.nodes.push(Node {
            service_name,
            user: install_options.user.clone(),
            number: node_number,
            port: node_port,
            rpc_port,
            version: version.clone(),
            status: NodeStatus::Added,
            pid: None,
            peer_id: None,
            log_dir_path: Some(service_log_dir_path.clone()),
            data_dir_path: Some(service_data_dir_path.clone()),
            safenode_path: Some(service_safenode_path),
        });

        node_number += 1;
    }

    std::fs::remove_file(safenode_download_path)?;

    println!("Services Added:");
    for install in added_service_data.iter() {
        println!(" {} {}", "✓".green(), install.0);
        println!("    - Safenode path: {}", install.1);
        println!("    - Data path: {}", install.2);
        println!("    - Log path: {}", install.3);
        println!("    - Service port: {}", install.4);
        println!("    - RPC port: {}", install.5);
    }

    println!("[!] Note: newly added services have not been started");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::MockServiceControl;
    use assert_fs::prelude::*;
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use mockall::mock;
    use mockall::predicate::*;
    use mockall::Sequence;
    use predicates::prelude::*;
    use sn_releases::{
        ArchiveType, Platform, ProgressCallback, ReleaseType, Result as SnReleaseResult,
        SafeReleaseRepositoryInterface,
    };
    use std::path::Path;

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
    async fn add_first_node_should_use_latest_version_and_add_one_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry { nodes: vec![] };
        let latest_version = "0.96.4";
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;
        let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
        safenode_download_path.write_binary(b"fake safenode bin")?;

        let mut seq = Sequence::new();
        mock_release_repo
            .expect_get_latest_version()
            .times(1)
            .returning(|_| Ok(latest_version.to_string()))
            .in_sequence(&mut seq);

        mock_release_repo
            .expect_download_release_from_s3()
            .with(
                eq(&ReleaseType::Safenode),
                eq(latest_version),
                always(), // Varies per platform
                eq(&ArchiveType::TarGz),
                always(), // Temporary directory which doesn't really matter
                always(), // Callback for progress bar which also doesn't matter
            )
            .times(1)
            .returning(move |_, _, _, _, _, _| {
                Ok(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                )))
            })
            .in_sequence(&mut seq);

        let safenode_download_path_clone = safenode_download_path.to_path_buf().clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                always(), // We will extract to a temporary directory
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_download_path_clone.clone()))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8080))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8081))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode1".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode1")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8080,
                rpc_port: 8081,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                count: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                peers: vec![],
                port: None,
                rpc_port: None,
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
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
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode1"))
        );
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode1"))
        );
        assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

        Ok(())
    }

    #[tokio::test]
    async fn add_first_node_should_use_latest_version_and_add_three_services() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry { nodes: vec![] };

        let latest_version = "0.96.4";
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;
        let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
        safenode_download_path.write_binary(b"fake safenode bin")?;

        let mut seq = Sequence::new();
        mock_release_repo
            .expect_get_latest_version()
            .times(1)
            .returning(|_| Ok(latest_version.to_string()))
            .in_sequence(&mut seq);

        mock_release_repo
            .expect_download_release_from_s3()
            .with(
                eq(&ReleaseType::Safenode),
                eq(latest_version),
                always(), // Varies per platform
                eq(&ArchiveType::TarGz),
                always(), // Temporary directory which doesn't really matter
                always(), // Callback for progress bar which also doesn't matter
            )
            .times(1)
            .returning(move |_, _, _, _, _, _| {
                Ok(PathBuf::from(&format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                )))
            })
            .in_sequence(&mut seq);

        let safenode_download_path_clone = safenode_download_path.to_path_buf().clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                always(),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_download_path_clone.clone()))
            .in_sequence(&mut seq);

        // Expected calls for first installation
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8080))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8081))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode1".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode1")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8080,
                rpc_port: 8081,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        // Expected calls for second installation
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8082))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8083))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode2".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode2")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8082,
                rpc_port: 8083,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        // Expected calls for third installation
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8084))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8085))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode3".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode3")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8084,
                rpc_port: 8085,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode3"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode3"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                count: Some(3),
                peers: vec![],
                port: None,
                rpc_port: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.nodes.len(), 3);
        assert_eq!(node_registry.nodes[0].version, latest_version);
        assert_eq!(node_registry.nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.nodes[0].user, get_username());
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode1"))
        );
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode1"))
        );
        assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);
        assert_eq!(node_registry.nodes[1].version, latest_version);
        assert_eq!(node_registry.nodes[1].service_name, "safenode2");
        assert_eq!(node_registry.nodes[1].user, get_username());
        assert_eq!(node_registry.nodes[1].number, 2);
        assert_eq!(node_registry.nodes[1].port, 8082);
        assert_eq!(node_registry.nodes[1].rpc_port, 8083);
        assert_eq!(
            node_registry.nodes[1].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode2"))
        );
        assert_eq!(
            node_registry.nodes[1].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode2"))
        );
        assert_matches!(node_registry.nodes[1].status, NodeStatus::Added);
        assert_eq!(node_registry.nodes[2].version, latest_version);
        assert_eq!(node_registry.nodes[2].service_name, "safenode3");
        assert_eq!(node_registry.nodes[2].user, get_username());
        assert_eq!(node_registry.nodes[2].number, 3);
        assert_eq!(node_registry.nodes[2].port, 8084);
        assert_eq!(node_registry.nodes[2].rpc_port, 8085);
        assert_eq!(
            node_registry.nodes[2].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode3"))
        );
        assert_eq!(
            node_registry.nodes[2].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode3"))
        );
        assert_matches!(node_registry.nodes[2].status, NodeStatus::Added);

        Ok(())
    }

    #[tokio::test]
    async fn add_first_node_should_use_specific_version_and_add_one_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry { nodes: vec![] };

        let specific_version = "0.95.0";
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;
        let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
        safenode_download_path.write_binary(b"fake safenode bin")?;

        let mut seq = Sequence::new();
        mock_release_repo
            .expect_download_release_from_s3()
            .with(
                eq(&ReleaseType::Safenode),
                eq(specific_version),
                always(), // Varies per platform
                eq(&ArchiveType::TarGz),
                always(), // Temporary directory which doesn't really matter
                always(), // Callback for progress bar which also doesn't matter
            )
            .times(1)
            .returning(move |_, _, _, _, _, _| {
                Ok(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    specific_version
                )))
            })
            .in_sequence(&mut seq);

        let safenode_download_path_clone = safenode_download_path.to_path_buf().clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    specific_version
                ))),
                always(),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_download_path_clone.clone()))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8080))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8081))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode1".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode1")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8080,
                rpc_port: 8081,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                count: None,
                peers: vec![],
                port: None,
                rpc_port: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                url: None,
                user: get_username(),
                version: Some(specific_version.to_string()),
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.nodes.len(), 1);
        assert_eq!(node_registry.nodes[0].version, specific_version);
        assert_eq!(node_registry.nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.nodes[0].user, get_username());
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode1"))
        );
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode1"))
        );
        assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

        Ok(())
    }

    #[tokio::test]
    async fn add_new_node_should_add_another_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let latest_version = "0.96.4";
        let mut node_registry = NodeRegistry {
            nodes: vec![Node {
                service_name: "safenode1".to_string(),
                user: "safe".to_string(),
                number: 1,
                port: 8080,
                rpc_port: 8081,
                version: latest_version.to_string(),
                status: NodeStatus::Added,
                pid: None,
                peer_id: None,
                log_dir_path: Some(PathBuf::from("/var/log/safenode/safenode1")),
                data_dir_path: Some(PathBuf::from("/var/safenode-manager/services/safenode1")),
                safenode_path: Some(PathBuf::from(
                    "/var/safenode-manager/services/safenode1/safenode",
                )),
            }],
        };
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("safenode1");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;
        let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
        safenode_download_path.write_binary(b"fake safenode bin")?;

        let mut seq = Sequence::new();
        mock_release_repo
            .expect_get_latest_version()
            .times(1)
            .returning(|_| Ok(latest_version.to_string()))
            .in_sequence(&mut seq);

        mock_release_repo
            .expect_download_release_from_s3()
            .with(
                eq(&ReleaseType::Safenode),
                eq(latest_version),
                always(), // Varies per platform
                eq(&ArchiveType::TarGz),
                always(), // Temporary directory which doesn't really matter
                always(), // Callback for progress bar which also doesn't matter
            )
            .times(1)
            .returning(move |_, _, _, _, _, _| {
                Ok(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                )))
            })
            .in_sequence(&mut seq);

        let safenode_download_path_clone = safenode_download_path.to_path_buf().clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                always(),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_download_path_clone.clone()))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8082))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8083))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode2".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode2")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8082,
                rpc_port: 8083,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                count: None,
                peers: vec![],
                port: None,
                rpc_port: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.nodes.len(), 2);
        assert_eq!(node_registry.nodes[1].version, latest_version);
        assert_eq!(node_registry.nodes[1].service_name, "safenode2");
        assert_eq!(node_registry.nodes[1].user, get_username());
        assert_eq!(node_registry.nodes[1].number, 2);
        assert_eq!(node_registry.nodes[1].port, 8082);
        assert_eq!(node_registry.nodes[1].rpc_port, 8083);
        assert_eq!(
            node_registry.nodes[1].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode2"))
        );
        assert_eq!(
            node_registry.nodes[1].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode2"))
        );
        assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

        Ok(())
    }

    #[tokio::test]
    async fn add_should_add_a_service_with_safenode_from_a_url() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let url = "https://sn-node.s3.eu-west-2.amazonaws.com/jacderida/file-upload-address/safenode-charlie-x86_64-unknown-linux-musl.tar.gz";

        let mut node_registry = NodeRegistry { nodes: vec![] };
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;
        let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
        safenode_download_path.write_binary(b"fake safenode bin")?;

        let mut seq = Sequence::new();

        mock_release_repo
            .expect_download_release()
            .with(
                eq(url),
                always(), // Temporary directory which doesn't really matter
                always(), // Callback for progress bar which also doesn't matter
            )
            .times(1)
            .returning(move |_, _, _| {
                Ok(PathBuf::from(
                    "/tmp/safenode-charlie-x86_64-unknown-linux-musl.tar.gz",
                ))
            })
            .in_sequence(&mut seq);

        let safenode_download_path_clone = safenode_download_path.to_path_buf().clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(
                    "/tmp/safenode-charlie-x86_64-unknown-linux-musl.tar.gz",
                )),
                always(), // We will extract to a temporary directory
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_download_path_clone.clone()))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8080))
            .in_sequence(&mut seq);
        mock_service_control
            .expect_get_available_port()
            .times(1)
            .returning(|| Ok(8081))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode1".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode1")
                    .join(SAFENODE_FILE_NAME),
                node_port: 8080,
                rpc_port: 8081,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                count: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                peers: vec![],
                port: None,
                rpc_port: None,
                url: Some(url.to_string()),
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        safenode_download_path.assert(predicate::path::missing());
        node_data_dir.assert(predicate::path::is_dir());
        node_logs_dir.assert(predicate::path::is_dir());

        assert_eq!(node_registry.nodes.len(), 1);
        assert_eq!(node_registry.nodes[0].version, "custom");
        assert_eq!(node_registry.nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.nodes[0].user, get_username());
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode1"))
        );
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode1"))
        );
        assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

        Ok(())
    }

    #[tokio::test]
    async fn add_node_should_use_custom_ports_for_one_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry { nodes: vec![] };
        let latest_version = "0.96.4";
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;
        let safenode_download_path = temp_dir.child(SAFENODE_FILE_NAME);
        safenode_download_path.write_binary(b"fake safenode bin")?;

        let custom_port = 12000;
        let custom_rpc_port = 12001;

        let mut seq = Sequence::new();
        mock_service_control
            .expect_is_port_free()
            .with(eq(custom_port))
            .times(1)
            .returning(|_| true)
            .in_sequence(&mut seq);
        mock_service_control
            .expect_is_port_free()
            .with(eq(custom_rpc_port))
            .times(1)
            .returning(|_| true)
            .in_sequence(&mut seq);

        mock_release_repo
            .expect_get_latest_version()
            .times(1)
            .returning(|_| Ok(latest_version.to_string()))
            .in_sequence(&mut seq);

        mock_release_repo
            .expect_download_release_from_s3()
            .with(
                eq(&ReleaseType::Safenode),
                eq(latest_version),
                always(), // Varies per platform
                eq(&ArchiveType::TarGz),
                always(), // Temporary directory which doesn't really matter
                always(), // Callback for progress bar which also doesn't matter
            )
            .times(1)
            .returning(move |_, _, _, _, _, _| {
                Ok(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                )))
            })
            .in_sequence(&mut seq);

        let safenode_download_path_clone = safenode_download_path.to_path_buf().clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                always(), // We will extract to a temporary directory
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_download_path_clone.clone()))
            .in_sequence(&mut seq);

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceConfig {
                name: "safenode1".to_string(),
                safenode_path: node_data_dir
                    .to_path_buf()
                    .join("safenode1")
                    .join(SAFENODE_FILE_NAME),
                node_port: custom_port,
                rpc_port: custom_rpc_port,
                service_user: get_username(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
                peers: vec![],
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                count: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                peers: vec![],
                port: Some(custom_port),
                rpc_port: Some(custom_rpc_port),
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
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
        assert_eq!(node_registry.nodes[0].port, custom_port);
        assert_eq!(node_registry.nodes[0].rpc_port, custom_rpc_port);
        assert_eq!(
            node_registry.nodes[0].log_dir_path,
            Some(node_logs_dir.to_path_buf().join("safenode1"))
        );
        assert_eq!(
            node_registry.nodes[0].data_dir_path,
            Some(node_data_dir.to_path_buf().join("safenode1"))
        );
        assert_matches!(node_registry.nodes[0].status, NodeStatus::Added);

        Ok(())
    }

    #[tokio::test]
    async fn add_node_should_return_error_if_custom_port_is_in_use() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();

        let mut node_registry = NodeRegistry { nodes: vec![] };
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;

        let custom_port = 12000;
        let custom_rpc_port = 12001;

        mock_service_control
            .expect_is_port_free()
            .with(eq(custom_port))
            .times(1)
            .returning(|_| false);

        let result = add(
            AddServiceOptions {
                count: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                peers: vec![],
                port: Some(custom_port),
                rpc_port: Some(custom_rpc_port),
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(MockSafeReleaseRepository::new()),
        )
        .await;

        match result {
            Ok(_) => panic!("This test should result in an error"),
            Err(e) => {
                assert_eq!(
                    format!("Port {custom_port} is already in use"),
                    e.to_string()
                )
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn add_node_should_return_error_if_custom_rpc_port_is_in_use() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();

        let mut node_registry = NodeRegistry { nodes: vec![] };
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;

        let custom_port = 12000;
        let custom_rpc_port = 12001;

        let mut seq = Sequence::new();
        mock_service_control
            .expect_is_port_free()
            .with(eq(custom_port))
            .times(1)
            .returning(|_| true)
            .in_sequence(&mut seq);
        mock_service_control
            .expect_is_port_free()
            .with(eq(custom_rpc_port))
            .times(1)
            .returning(|_| false)
            .in_sequence(&mut seq);

        let result = add(
            AddServiceOptions {
                count: None,
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                peers: vec![],
                port: Some(custom_port),
                rpc_port: Some(custom_rpc_port),
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(MockSafeReleaseRepository::new()),
        )
        .await;

        match result {
            Ok(_) => panic!("This test should result in an error"),
            Err(e) => {
                assert_eq!(
                    format!("Port {custom_rpc_port} is already in use"),
                    e.to_string()
                )
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn add_node_should_return_error_if_custom_port_is_used_and_more_than_one_service_is_used(
    ) -> Result<()> {
        let mut node_registry = NodeRegistry { nodes: vec![] };
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("data");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;

        let custom_port = 12000;
        let custom_rpc_port = 12001;

        let result = add(
            AddServiceOptions {
                count: Some(3),
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                peers: vec![],
                port: Some(custom_port),
                rpc_port: Some(custom_rpc_port),
                url: None,
                user: get_username(),
                version: None,
            },
            &mut node_registry,
            &MockServiceControl::new(),
            Box::new(MockSafeReleaseRepository::new()),
        )
        .await;

        match result {
            Ok(_) => panic!("This test should result in an error"),
            Err(e) => {
                assert_eq!(
                    format!("Custom ports can only be used when adding a single service"),
                    e.to_string()
                )
            }
        }

        Ok(())
    }
}
