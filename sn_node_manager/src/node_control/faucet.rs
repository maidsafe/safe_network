// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    config::create_owned_dir,
    node_control::config::{AddFaucetServiceOptions, InstallFaucetServiceCtxBuilder},
    service::ServiceControl,
    VerbosityLevel,
};
use color_eyre::Result;
use colored::Colorize;
use sn_protocol::node_registry::{Faucet, NodeRegistry, NodeStatus};

/// Install the faucet as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub async fn add_faucet(
    install_options: AddFaucetServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    verbosity: VerbosityLevel,
) -> Result<()> {
    create_owned_dir(
        install_options.service_log_dir_path.clone(),
        &install_options.user,
    )?;

    std::fs::copy(
        install_options.faucet_download_bin_path.clone(),
        install_options.faucet_install_bin_path.clone(),
    )?;

    let install_ctx = InstallFaucetServiceCtxBuilder {
        bootstrap_peers: install_options.bootstrap_peers.clone(),
        env_variables: install_options.env_variables.clone(),
        faucet_path: install_options.faucet_install_bin_path.clone(),
        local: install_options.local,
        log_dir_path: install_options.service_log_dir_path.clone(),
        name: "faucet".to_string(),
        service_user: install_options.user.clone(),
    };

    match service_control.install(install_ctx.execute()?) {
        Ok(()) => {
            node_registry.faucet = Some(Faucet {
                faucet_path: install_options.faucet_install_bin_path.clone(),
                local: false,
                log_dir_path: install_options.service_log_dir_path.clone(),
                pid: None,
                service_name: "faucet".to_string(),
                status: NodeStatus::Added,
                user: install_options.user.clone(),
                version: install_options.version,
            });
            println!("Faucet service added {}", "✓".green());
            if verbosity != VerbosityLevel::Minimal {
                println!(
                    "  - Bin path: {}",
                    install_options.faucet_install_bin_path.to_string_lossy()
                );
                println!(
                    "  - Data path: {}",
                    install_options.service_data_dir_path.to_string_lossy()
                );
                println!(
                    "  - Log path: {}",
                    install_options.service_log_dir_path.to_string_lossy()
                );
            }
            println!("[!] Note: the service has not been started");
            std::fs::remove_file(install_options.faucet_download_bin_path)?;
            node_registry.save()?;
            Ok(())
        }
        Err(e) => {
            println!("Failed to add faucet service: {e}");
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::MockServiceControl;
    use assert_fs::prelude::*;
    use mockall::predicate::*;
    use predicates::prelude::*;
    use service_manager::ServiceInstallCtx;
    use std::ffi::OsString;

    #[cfg(not(target_os = "windows"))]
    const FAUCET_FILE_NAME: &str = "faucet";
    #[cfg(target_os = "windows")]
    const FAUCET_FILE_NAME: &str = "faucet.exe";

    #[cfg(target_os = "windows")]
    fn get_username() -> String {
        std::env::var("USERNAME").expect("Failed to get username")
    }

    #[cfg(not(target_os = "windows"))]
    fn get_username() -> String {
        std::env::var("USER").expect("Failed to get username")
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
            faucet: None,
            faucet_pid: None,
            environment_variables: None,
            nodes: vec![],
            save_path: node_reg_path.to_path_buf(),
        };

        let mut mock_service_control = MockServiceControl::new();

        mock_service_control
            .expect_install()
            .times(1)
            .with(eq(ServiceInstallCtx {
                label: "faucet".parse()?,
                program: faucet_install_path.to_path_buf(),
                args: vec![
                    OsString::from("--log-output-dest"),
                    OsString::from(faucet_logs_dir.to_path_buf().as_os_str()),
                ],
                environment: Some(vec![("SN_LOG".to_string(), "all".to_string())]),
                contents: None,
                working_directory: None,
                username: Some(get_username()),
            }))
            .returning(|_| Ok(()));

        add_faucet(
            AddFaucetServiceOptions {
                bootstrap_peers: vec![],
                env_variables: Some(vec![("SN_LOG".to_string(), "all".to_string())]),
                faucet_download_bin_path: faucet_download_path.to_path_buf(),
                faucet_install_bin_path: faucet_install_path.to_path_buf(),
                local: false,
                service_data_dir_path: faucet_data_dir.to_path_buf(),
                service_log_dir_path: faucet_logs_dir.to_path_buf(),
                url: None,
                user: get_username(),
                version: latest_version.to_string(),
            },
            &mut node_registry,
            &mock_service_control,
            VerbosityLevel::Normal,
        )
        .await?;

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
        assert_eq!(saved_faucet.status, NodeStatus::Added);
        assert_eq!(saved_faucet.user, get_username());
        assert_eq!(saved_faucet.version, latest_version);

        Ok(())
    }
}
