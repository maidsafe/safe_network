// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::helpers::get_bin_version;
use color_eyre::Result;
use colored::Colorize;
use service_manager::{ServiceInstallCtx, ServiceLabel};
use sn_service_management::{
    control::ServiceControl, DaemonServiceData, NodeRegistry, ServiceStatus,
};
use std::{
    ffi::OsString,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

pub const DAEMON_DEFAULT_PORT: u16 = 12500;
const DAEMON_SERVICE_NAME: &str = "safenodemand";

/// Install the daemon as a service.
///
/// This only defines the service; it does not start it.
pub fn add_daemon(
    address: Ipv4Addr,
    port: u16,
    daemon_path: PathBuf,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
) -> Result<()> {
    let service_name: ServiceLabel = DAEMON_SERVICE_NAME.parse()?;

    // try to stop and uninstall if already installed
    if let Err(err) = service_control.stop(DAEMON_SERVICE_NAME) {
        println!("Error while stopping manager daemon. Ignoring the error. {err:?}");
    }
    if let Err(err) = service_control.uninstall(DAEMON_SERVICE_NAME) {
        println!("Error while uninstalling manager daemon. Ignoring the error. {err:?}");
    }

    let install_ctx = ServiceInstallCtx {
        label: service_name.clone(),
        program: daemon_path.clone(),
        args: vec![
            OsString::from("--port"),
            OsString::from(port.to_string()),
            OsString::from("--address"),
            OsString::from(address.to_string()),
        ],
        contents: None,
        username: None,
        working_directory: None,
        environment: None,
    };

    match service_control.install(install_ctx) {
        Ok(()) => {
            let daemon = DaemonServiceData {
                daemon_path: daemon_path.clone(),
                endpoint: Some(SocketAddr::new(IpAddr::V4(address), port)),
                pid: None,
                service_name: DAEMON_SERVICE_NAME.to_string(),
                status: ServiceStatus::Added,
                version: get_bin_version(&daemon_path)?,
            };
            node_registry.daemon = Some(daemon);

            println!("Daemon service added {}", "✓".green());
            println!("[!] Note: the service has not been started");
            node_registry.save()?;
            Ok(())
        }
        Err(e) => {
            println!("Failed to add daemon service: {e}");
            Err(e.into())
        }
    }
}
