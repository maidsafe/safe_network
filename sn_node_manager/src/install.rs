// Copyright (C) 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::node::{Node, NodeRegistry, NodeStatus};
use crate::service::{ServiceConfig, ServiceControl};
use color_eyre::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use sn_releases::{get_running_platform, ArchiveType, ReleaseType, SafeReleaseRepositoryInterface};
use std::path::PathBuf;
use std::sync::Arc;

pub struct AddServiceOptions {
    pub safenode_dir_path: PathBuf,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub user: String,
    pub count: Option<u16>,
    pub version: Option<String>,
}

/// Install safenode as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
///
/// For the directory paths used in the install options, they should be created before this
/// function is called, as they may require root or administrative access to write to.
pub async fn add(
    install_options: AddServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    release_repo: Box<dyn SafeReleaseRepositoryInterface>,
) -> Result<()> {
    let pb = Arc::new(ProgressBar::new(0));
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("#>-"));
    let pb_clone = pb.clone();
    let callback: Box<dyn Fn(u64, u64) + Send + Sync> = Box::new(move |downloaded, total| {
        pb_clone.set_length(total);
        pb_clone.set_position(downloaded);
    });

    let version = if let Some(version) = install_options.version {
        version
    } else {
        println!("Retrieving latest version for safenode...");
        release_repo
            .get_latest_version(&ReleaseType::Safenode)
            .await?
    };

    println!("Downloading safenode version {version}...");

    let temp_dir_path = create_temp_dir()?;
    let archive_path = release_repo
        .download_release_from_s3(
            &ReleaseType::Safenode,
            &version,
            &get_running_platform()?,
            &ArchiveType::TarGz,
            &temp_dir_path,
            &callback,
        )
        .await?;
    pb.finish_with_message("Download complete");
    let safenode_path =
        release_repo.extract_release_archive(&archive_path, &install_options.safenode_dir_path)?;

    let mut added_service_data = vec![];
    let current_node_count = node_registry.nodes.len() as u16;
    let target_node_count = current_node_count + install_options.count.unwrap_or(1);
    let mut node_number = current_node_count + 1;
    while node_number <= target_node_count {
        let node_port = service_control.get_available_port()?;
        let rpc_port = service_control.get_available_port()?;

        let service_name = format!("safenode{node_number}");
        let service_data_dir_path = install_options
            .service_data_dir_path
            .join(service_name.clone());
        let service_log_dir_path = install_options
            .service_log_dir_path
            .join(service_name.clone());
        service_control.install(ServiceConfig {
            name: service_name.clone(),
            safenode_path: safenode_path.clone(),
            node_port,
            rpc_port,
            service_user: install_options.user.clone(),
            log_dir_path: service_log_dir_path.clone(),
            data_dir_path: service_data_dir_path.clone(),
        })?;

        added_service_data.push((
            service_name.clone(),
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
            log_dir_path: service_log_dir_path.clone(),
            data_dir_path: service_data_dir_path.clone(),
        });

        node_number += 1;
    }

    println!("Services Added:");
    for install in added_service_data.iter() {
        println!(" {} {}", "✓".green(), install.0);
        println!("    - Data path: {}", install.1);
        println!("    - Log path: {}", install.2);
        println!("    - Service port: {}", install.3);
        println!("    - RPC port: {}", install.4);
    }

    println!("[!] Note: newly added services have not been started");

    Ok(())
}

/// There is a `tempdir` crate that provides the same kind of functionality, but it was flagged for
/// a security vulnerability.
fn create_temp_dir() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    let unique_dir_name = uuid::Uuid::new_v4().to_string();
    let new_temp_dir = temp_dir.join(unique_dir_name);
    std::fs::create_dir_all(&new_temp_dir)?;
    Ok(new_temp_dir)
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
    use sn_releases::{
        ArchiveType, Platform, ProgressCallback, ReleaseType, Result as SnReleaseResult,
        SafeReleaseRepositoryInterface,
    };
    use std::path::Path;

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
            fn extract_release_archive(&self, archive_path: &Path, extract_dir: &Path) -> SnReleaseResult<PathBuf>;
        }
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

        let safenode_install_dir_path = temp_dir.to_path_buf();
        let safenode_install_path = safenode_install_dir_path.join("safenode");
        let safenode_install_path_clone = safenode_install_path.clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                eq(safenode_install_dir_path),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_install_path.clone()))
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
                safenode_path: safenode_install_path_clone,
                node_port: 8080,
                rpc_port: 8081,
                service_user: "safe".to_string(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                user: "safe".to_string(),
                count: None,
                version: None,
            },
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.nodes.len(), 1);
        assert_eq!(node_registry.nodes[0].version, latest_version);
        assert_eq!(node_registry.nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.nodes[0].user, "safe");
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
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

        let safenode_install_dir_path = temp_dir.to_path_buf();
        let safenode_install_path = safenode_install_dir_path.join("safenode");
        let safenode_install_path_clone = safenode_install_path.clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                eq(safenode_install_dir_path),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_install_path.clone()))
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
                safenode_path: safenode_install_path_clone.clone(),
                node_port: 8080,
                rpc_port: 8081,
                service_user: "safe".to_string(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
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
                safenode_path: safenode_install_path_clone.clone(),
                node_port: 8082,
                rpc_port: 8083,
                service_user: "safe".to_string(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
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
                safenode_path: safenode_install_path_clone,
                node_port: 8084,
                rpc_port: 8085,
                service_user: "safe".to_string(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode3"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode3"),
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                user: "safe".to_string(),
                count: Some(3),
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
        assert_eq!(node_registry.nodes[0].user, "safe");
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
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
        assert_eq!(node_registry.nodes[1].user, "safe");
        assert_eq!(node_registry.nodes[1].number, 2);
        assert_eq!(node_registry.nodes[1].port, 8082);
        assert_eq!(node_registry.nodes[1].rpc_port, 8083);
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
        assert_eq!(node_registry.nodes[2].user, "safe");
        assert_eq!(node_registry.nodes[2].number, 3);
        assert_eq!(node_registry.nodes[2].port, 8084);
        assert_eq!(node_registry.nodes[2].rpc_port, 8085);
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

        let safenode_install_dir_path = temp_dir.to_path_buf();
        let safenode_install_path = safenode_install_dir_path.join("safenode");
        let safenode_install_path_clone = safenode_install_path.clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    specific_version
                ))),
                eq(safenode_install_dir_path),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_install_path.clone()))
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
                safenode_path: safenode_install_path_clone,
                node_port: 8080,
                rpc_port: 8081,
                service_user: "safe".to_string(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode1"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode1"),
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                user: "safe".to_string(),
                count: None,
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
        assert_eq!(node_registry.nodes[0].user, "safe");
        assert_eq!(node_registry.nodes[0].number, 1);
        assert_eq!(node_registry.nodes[0].port, 8080);
        assert_eq!(node_registry.nodes[0].rpc_port, 8081);
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
                log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
                data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            }],
        };
        let temp_dir = assert_fs::TempDir::new()?;
        let node_data_dir = temp_dir.child("safenode1");
        node_data_dir.create_dir_all()?;
        let node_logs_dir = temp_dir.child("logs");
        node_logs_dir.create_dir_all()?;

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

        let safenode_install_dir_path = temp_dir.to_path_buf();
        let safenode_install_path = safenode_install_dir_path.join("safenode");
        let safenode_install_path_clone = safenode_install_path.clone();
        mock_release_repo
            .expect_extract_release_archive()
            .with(
                eq(PathBuf::from(format!(
                    "/tmp/safenode-{}-x86_64-unknown-linux-musl.tar.gz",
                    latest_version
                ))),
                eq(safenode_install_dir_path),
            )
            .times(1)
            .returning(move |_, _| Ok(safenode_install_path.clone()))
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
                safenode_path: safenode_install_path_clone,
                node_port: 8082,
                rpc_port: 8083,
                service_user: "safe".to_string(),
                log_dir_path: node_logs_dir.to_path_buf().join("safenode2"),
                data_dir_path: node_data_dir.to_path_buf().join("safenode2"),
            }))
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        add(
            AddServiceOptions {
                safenode_dir_path: temp_dir.to_path_buf(),
                service_data_dir_path: node_data_dir.to_path_buf(),
                service_log_dir_path: node_logs_dir.to_path_buf(),
                user: "safe".to_string(),
                count: None,
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
        assert_eq!(node_registry.nodes[1].user, "safe");
        assert_eq!(node_registry.nodes[1].number, 2);
        assert_eq!(node_registry.nodes[1].port, 8082);
        assert_eq!(node_registry.nodes[1].rpc_port, 8083);
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
}
