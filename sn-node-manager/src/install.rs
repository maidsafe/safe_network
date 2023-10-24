use crate::service::ServiceControl;
use color_eyre::Result;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use sn_releases::{get_running_platform, ArchiveType, ReleaseType, SafeReleaseRepositoryInterface};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledNode {
    pub version: String,
    pub service_name: String,
    pub user: String,
    pub number: u16,
    pub port: u16,
    pub rpc_port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeRegistry {
    pub installed_nodes: Vec<InstalledNode>,
}

impl NodeRegistry {
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string(self)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let registry = serde_json::from_str(&contents)?;
        Ok(registry)
    }
}

pub async fn install(
    count: Option<u16>,
    user: Option<String>,
    version: Option<String>,
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

    let version = if let Some(version) = version {
        version
    } else {
        release_repo
            .get_latest_version(&ReleaseType::Safenode)
            .await?
    };

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

    let install_dir_path = get_safenode_install_path();
    let safenode_path = release_repo.extract_release_archive(&archive_path, &install_dir_path)?;

    let service_user = user.unwrap_or("safe".to_string());
    service_control.create_service_user(&service_user)?;

    let current_node_count = node_registry.installed_nodes.len() as u16;
    let target_node_count = current_node_count + count.unwrap_or(1);
    let mut node_number = current_node_count + 1;
    while node_number <= target_node_count {
        let safenode_port = service_control.get_available_port()?;
        let rpc_port = service_control.get_available_port()?;

        let service_name = format!("safenode{node_number}");
        service_control.install(
            &service_name,
            &safenode_path,
            safenode_port,
            rpc_port,
            &service_user.clone(),
        )?;

        node_registry.installed_nodes.push(InstalledNode {
            service_name,
            user: service_user.clone(),
            number: node_number,
            port: safenode_port,
            rpc_port,
            version: version.clone(),
        });

        node_number += 1;
    }

    Ok(())
}

#[cfg(unix)]
pub fn get_safenode_install_path() -> PathBuf {
    PathBuf::from("/usr/local/bin")
}

#[cfg(windows)]
pub fn get_safenode_install_path() -> PathBuf {
    PathBuf::from("C:\\Program Files\\safenode-manager")
}

#[cfg(unix)]
pub fn get_node_registry_path() -> Result<PathBuf> {
    // This needs to be a system-wide location rather than a user directory because the `install`
    // command will run as the root user. However, it should be readable by non-root users, because
    // other commands, e.g., requesting status, shouldn't require root.
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let path = Path::new("/var/safenode-manager/");
    if !path.exists() {
        fs::create_dir_all(path)?;
        let mut perm = fs::metadata(path)?.permissions();
        perm.set_mode(0o755); // set permissions to rwxr-xr-x
        fs::set_permissions(path, perm)?;
    }

    Ok(path.join("node_registry.json"))
}

#[cfg(windows)]
pub fn get_node_registry_path() -> Result<PathBuf> {
    let path = Path::new("C:\\ProgramData\\safenode-manager");
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    Ok(path.join("node_registry.json"))
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
    use crate::install::get_safenode_install_path;
    use crate::service::MockServiceControl;
    use async_trait::async_trait;
    use mockall::mock;
    use mockall::predicate::*;
    use mockall::Sequence;
    use sn_releases::{
        ArchiveType, Platform, ProgressCallback, ReleaseType, Result as SnReleaseResult,
        SafeReleaseRepositoryInterface,
    };

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
    async fn install_first_node_should_use_latest_version_and_install_one_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry {
            installed_nodes: vec![],
        };
        let latest_version = "0.96.4";

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

        let safenode_install_dir_path = get_safenode_install_path();
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
            .expect_create_service_user()
            .with(eq("safe"))
            .times(1)
            .returning(|_| Ok(()))
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
            .with(
                eq("safenode1"),
                eq(safenode_install_path_clone),
                eq(8080),
                eq(8081),
                eq("safe"),
            )
            .returning(|_, _, _, _, _| Ok(()))
            .in_sequence(&mut seq);

        install(
            None,
            None,
            None,
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.installed_nodes.len(), 1);
        assert_eq!(node_registry.installed_nodes[0].version, latest_version);
        assert_eq!(node_registry.installed_nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.installed_nodes[0].user, "safe");
        assert_eq!(node_registry.installed_nodes[0].number, 1);
        assert_eq!(node_registry.installed_nodes[0].port, 8080);
        assert_eq!(node_registry.installed_nodes[0].rpc_port, 8081);

        Ok(())
    }

    #[tokio::test]
    async fn install_first_node_should_use_latest_version_and_install_three_services() -> Result<()>
    {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry {
            installed_nodes: vec![],
        };

        let latest_version = "0.96.4";

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

        let safenode_install_dir_path = get_safenode_install_path();
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
            .expect_create_service_user()
            .with(eq("safe"))
            .times(1)
            .returning(|_| Ok(()))
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
            .with(
                eq("safenode1"),
                eq(safenode_install_path_clone.clone()),
                eq(8080),
                eq(8081),
                eq("safe"),
            )
            .returning(|_, _, _, _, _| Ok(()))
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
            .with(
                eq("safenode2"),
                eq(safenode_install_path_clone.clone()),
                eq(8082),
                eq(8083),
                eq("safe"),
            )
            .returning(|_, _, _, _, _| Ok(()))
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
            .with(
                eq("safenode3"),
                eq(safenode_install_path_clone),
                eq(8084),
                eq(8085),
                eq("safe"),
            )
            .returning(|_, _, _, _, _| Ok(()))
            .in_sequence(&mut seq);

        install(
            Some(3),
            None,
            None,
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.installed_nodes.len(), 3);
        assert_eq!(node_registry.installed_nodes[0].version, latest_version);
        assert_eq!(node_registry.installed_nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.installed_nodes[0].user, "safe");
        assert_eq!(node_registry.installed_nodes[0].number, 1);
        assert_eq!(node_registry.installed_nodes[0].port, 8080);
        assert_eq!(node_registry.installed_nodes[0].rpc_port, 8081);
        assert_eq!(node_registry.installed_nodes[1].version, latest_version);
        assert_eq!(node_registry.installed_nodes[1].service_name, "safenode2");
        assert_eq!(node_registry.installed_nodes[1].user, "safe");
        assert_eq!(node_registry.installed_nodes[1].number, 2);
        assert_eq!(node_registry.installed_nodes[1].port, 8082);
        assert_eq!(node_registry.installed_nodes[1].rpc_port, 8083);
        assert_eq!(node_registry.installed_nodes[2].version, latest_version);
        assert_eq!(node_registry.installed_nodes[2].service_name, "safenode3");
        assert_eq!(node_registry.installed_nodes[2].user, "safe");
        assert_eq!(node_registry.installed_nodes[2].number, 3);
        assert_eq!(node_registry.installed_nodes[2].port, 8084);
        assert_eq!(node_registry.installed_nodes[2].rpc_port, 8085);

        Ok(())
    }

    #[tokio::test]
    async fn install_first_node_should_use_specific_version_and_install_one_service() -> Result<()>
    {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry {
            installed_nodes: vec![],
        };

        let specific_version = "0.95.0";

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

        let safenode_install_dir_path = get_safenode_install_path();
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
            .expect_create_service_user()
            .with(eq("safe"))
            .times(1)
            .returning(|_| Ok(()))
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
            .with(
                eq("safenode1"),
                eq(safenode_install_path_clone),
                eq(8080),
                eq(8081),
                eq("safe"),
            )
            .returning(|_, _, _, _, _| Ok(()))
            .in_sequence(&mut seq);

        install(
            None,
            None,
            Some(specific_version.to_string()),
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.installed_nodes.len(), 1);
        assert_eq!(node_registry.installed_nodes[0].version, specific_version);
        assert_eq!(node_registry.installed_nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.installed_nodes[0].user, "safe");
        assert_eq!(node_registry.installed_nodes[0].number, 1);
        assert_eq!(node_registry.installed_nodes[0].port, 8080);
        assert_eq!(node_registry.installed_nodes[0].rpc_port, 8081);

        Ok(())
    }

    #[tokio::test]
    async fn install_first_node_should_use_specific_user_and_install_one_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let mut node_registry = NodeRegistry {
            installed_nodes: vec![],
        };

        let latest_version = "0.96.4";

        let mut seq = Sequence::new();
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

        let safenode_install_dir_path = get_safenode_install_path();
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
            .expect_create_service_user()
            .with(eq("safe2"))
            .times(1)
            .returning(|_| Ok(()))
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
            .with(
                eq("safenode1"),
                eq(safenode_install_path_clone),
                eq(8080),
                eq(8081),
                eq("safe2"),
            )
            .returning(|_, _, _, _, _| Ok(()))
            .in_sequence(&mut seq);

        install(
            None,
            Some("safe2".to_string()),
            Some(latest_version.to_string()),
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.installed_nodes.len(), 1);
        assert_eq!(node_registry.installed_nodes[0].version, latest_version);
        assert_eq!(node_registry.installed_nodes[0].service_name, "safenode1");
        assert_eq!(node_registry.installed_nodes[0].user, "safe2");
        assert_eq!(node_registry.installed_nodes[0].number, 1);
        assert_eq!(node_registry.installed_nodes[0].port, 8080);
        assert_eq!(node_registry.installed_nodes[0].rpc_port, 8081);

        Ok(())
    }

    #[tokio::test]
    async fn install_new_node_should_add_another_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_release_repo = MockSafeReleaseRepository::new();

        let latest_version = "0.96.4";
        let mut node_registry = NodeRegistry {
            installed_nodes: vec![InstalledNode {
                service_name: "safenode1".to_string(),
                user: "safe".to_string(),
                number: 1,
                port: 8080,
                rpc_port: 8081,
                version: latest_version.to_string(),
            }],
        };

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

        let safenode_install_dir_path = get_safenode_install_path();
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
            .expect_create_service_user()
            .with(eq("safe"))
            .times(1)
            .returning(|_| Ok(()))
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
            .with(
                eq("safenode2"),
                eq(safenode_install_path_clone),
                eq(8082),
                eq(8083),
                eq("safe"),
            )
            .returning(|_, _, _, _, _| Ok(()))
            .in_sequence(&mut seq);

        install(
            None,
            None,
            None,
            &mut node_registry,
            &mock_service_control,
            Box::new(mock_release_repo),
        )
        .await?;

        assert_eq!(node_registry.installed_nodes.len(), 2);
        assert_eq!(node_registry.installed_nodes[1].version, latest_version);
        assert_eq!(node_registry.installed_nodes[1].service_name, "safenode2");
        assert_eq!(node_registry.installed_nodes[1].user, "safe");
        assert_eq!(node_registry.installed_nodes[1].number, 2);
        assert_eq!(node_registry.installed_nodes[1].port, 8082);
        assert_eq!(node_registry.installed_nodes[1].rpc_port, 8083);

        Ok(())
    }
}
