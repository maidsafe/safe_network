// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{service::ServiceControl, VerbosityLevel};
use color_eyre::Result;
use service_manager::{ServiceInstallCtx, ServiceLabel};
use std::{ffi::OsString, net::Ipv4Addr, path::PathBuf};

pub fn run(
    address: Ipv4Addr,
    port: u16,
    daemon_path: PathBuf,
    service_user: String,
    service_control: &dyn ServiceControl,
    _verbosity: VerbosityLevel,
) -> Result<()> {
    let service_name: ServiceLabel = "safenode-manager-daemon".parse()?;

    let install_ctx = ServiceInstallCtx {
        label: service_name.clone(),
        program: daemon_path,
        args: vec![
            OsString::from("--port"),
            OsString::from(port.to_string()),
            OsString::from("--address"),
            OsString::from(address.to_string()),
        ],
        contents: None,
        username: Some(service_user),
        working_directory: None,
        environment: None,
    };
    service_control.install(install_ctx)?;
    service_control.start(&service_name.to_string())?;

    Ok(())
}
