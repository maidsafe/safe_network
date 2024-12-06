// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    control::ServiceControl, error::Result, ServiceStateActions, ServiceStatus, UpgradeOptions,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use service_manager::ServiceInstallCtx;
use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaucetServiceData {
    pub faucet_path: PathBuf,
    pub local: bool,
    pub log_dir_path: PathBuf,
    pub pid: Option<u32>,
    pub service_name: String,
    pub status: ServiceStatus,
    pub user: String,
    pub version: String,
}

pub struct FaucetService<'a> {
    pub service_data: &'a mut FaucetServiceData,
    pub service_control: Box<dyn ServiceControl + Send>,
}

impl<'a> FaucetService<'a> {
    pub fn new(
        service_data: &'a mut FaucetServiceData,
        service_control: Box<dyn ServiceControl + Send>,
    ) -> FaucetService<'a> {
        FaucetService {
            service_data,
            service_control,
        }
    }
}

#[async_trait]
impl ServiceStateActions for FaucetService<'_> {
    fn bin_path(&self) -> PathBuf {
        self.service_data.faucet_path.clone()
    }

    fn build_upgrade_install_context(&self, options: UpgradeOptions) -> Result<ServiceInstallCtx> {
        let mut args = vec![
            OsString::from("--log-output-dest"),
            OsString::from(self.service_data.log_dir_path.to_string_lossy().to_string()),
        ];

        args.push(OsString::from("server"));

        Ok(ServiceInstallCtx {
            args,
            autostart: true,
            contents: None,
            environment: options.env_variables,
            label: self.service_data.service_name.parse()?,
            program: self.service_data.faucet_path.to_path_buf(),
            username: Some(self.service_data.user.to_string()),
            working_directory: None,
        })
    }

    fn data_dir_path(&self) -> PathBuf {
        PathBuf::new()
    }

    fn is_user_mode(&self) -> bool {
        // The faucet service should never run in user mode.
        false
    }

    fn log_dir_path(&self) -> PathBuf {
        self.service_data.log_dir_path.clone()
    }

    fn name(&self) -> String {
        self.service_data.service_name.clone()
    }

    fn pid(&self) -> Option<u32> {
        self.service_data.pid
    }

    fn on_remove(&mut self) {
        self.service_data.status = ServiceStatus::Removed;
    }

    async fn on_start(&mut self, pid: Option<u32>, _full_refresh: bool) -> Result<()> {
        self.service_data.pid = pid;
        self.service_data.status = ServiceStatus::Running;
        Ok(())
    }

    async fn on_stop(&mut self) -> Result<()> {
        self.service_data.pid = None;
        self.service_data.status = ServiceStatus::Stopped;
        Ok(())
    }

    fn set_version(&mut self, version: &str) {
        self.service_data.version = version.to_string();
    }

    fn status(&self) -> ServiceStatus {
        self.service_data.status.clone()
    }

    fn version(&self) -> String {
        self.service_data.version.clone()
    }
}
