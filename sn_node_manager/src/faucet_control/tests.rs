// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    faucet_control::{add_faucet, config::AddFaucetServiceOptions},
    VerbosityLevel,
};
use assert_fs::prelude::*;
use color_eyre::Result;
use mockall::{mock, predicate::*};
use predicates::prelude::*;
use service_manager::ServiceInstallCtx;
use sn_service_management::control::ServiceControl;
use sn_service_management::error::Result as ServiceControlResult;
use sn_service_management::{NodeRegistry, ServiceStatus};
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

mock! {
    pub ServiceControl {}
    impl ServiceControl for ServiceControl {
        fn create_service_user(&self, username: &str) -> ServiceControlResult<()>;
        fn get_available_port(&self) -> ServiceControlResult<u16>;
        fn install(&self, install_ctx: ServiceInstallCtx) -> ServiceControlResult<()>;
        fn get_process_pid(&self, name: &str) -> ServiceControlResult<u32>;
        fn is_service_process_running(&self, pid: u32) -> bool;
        fn start(&self, service_name: &str) -> ServiceControlResult<()>;
        fn stop(&self, service_name: &str) -> ServiceControlResult<()>;
        fn uninstall(&self, service_name: &str) -> ServiceControlResult<()>;
        fn wait(&self, delay: u64);
    }
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
