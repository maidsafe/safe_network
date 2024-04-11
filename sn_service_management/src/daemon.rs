// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    control::ServiceControl,
    error::{Error, Result},
    ServiceStateActions, ServiceStatus, UpgradeOptions,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use service_manager::ServiceInstallCtx;
use std::{ffi::OsString, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DaemonServiceData {
    pub daemon_path: PathBuf,
    pub endpoint: Option<SocketAddr>,
    pub pid: Option<u32>,
    pub service_name: String,
    pub status: ServiceStatus,
    pub version: String,
}

pub struct DaemonService<'a> {
    pub service_data: &'a mut DaemonServiceData,
    pub service_control: Box<dyn ServiceControl + Send>,
}

impl<'a> DaemonService<'a> {
    pub fn new(
        service_data: &'a mut DaemonServiceData,
        service_control: Box<dyn ServiceControl + Send>,
    ) -> DaemonService<'a> {
        DaemonService {
            service_data,
            service_control,
        }
    }
}

#[async_trait]
impl<'a> ServiceStateActions for DaemonService<'a> {
    fn bin_path(&self) -> PathBuf {
        self.service_data.daemon_path.clone()
    }

    fn build_upgrade_install_context(&self, _options: UpgradeOptions) -> Result<ServiceInstallCtx> {
        let (address, port) = self
            .service_data
            .endpoint
            .ok_or_else(|| Error::DaemonEndpointNotSet)
            .map(|e| (e.ip().to_string(), e.port().to_string()))?;
        let install_ctx = ServiceInstallCtx {
            label: self.service_data.service_name.parse()?,
            program: self.service_data.daemon_path.clone(),
            args: vec![
                OsString::from("--port"),
                OsString::from(port),
                OsString::from("--address"),
                OsString::from(address),
            ],
            contents: None,
            username: None,
            working_directory: None,
            environment: None,
        };
        Ok(install_ctx)
    }

    fn data_dir_path(&self) -> PathBuf {
        PathBuf::new()
    }

    fn log_dir_path(&self) -> PathBuf {
        PathBuf::new()
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

    async fn on_start(&mut self) -> Result<()> {
        // get_process_pid causes errors for the daemon. Maybe because it is being run as root?
        if let Ok(pid) = self
            .service_control
            .get_process_pid(&self.service_data.daemon_path)
        {
            self.service_data.pid = Some(pid);
        }

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
