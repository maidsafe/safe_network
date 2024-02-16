// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
use libp2p::Multiaddr;
use semver::Version;
use service_manager::{ServiceInstallCtx, ServiceLabel};
use std::{
    ffi::OsString,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

#[derive(Debug, PartialEq)]
/// Intermediate struct to generate the proper `ServiceInstallCtx` that is used to install safenode services.
pub(super) struct InstallNodeServiceCtxBuilder {
    pub data_dir_path: PathBuf,
    pub genesis: bool,
    pub local: bool,
    pub log_dir_path: PathBuf,
    pub name: String,
    pub node_port: Option<u16>,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub rpc_socket_addr: SocketAddr,
    pub safenode_path: PathBuf,
    pub service_user: String,
    pub env_variables: Option<Vec<(String, String)>>,
}

impl InstallNodeServiceCtxBuilder {
    pub fn execute(self) -> Result<ServiceInstallCtx> {
        let label: ServiceLabel = self.name.parse()?;
        let mut args = vec![
            OsString::from("--rpc"),
            OsString::from(self.rpc_socket_addr.to_string()),
            OsString::from("--root-dir"),
            OsString::from(self.data_dir_path.to_string_lossy().to_string()),
            OsString::from("--log-output-dest"),
            OsString::from(self.log_dir_path.to_string_lossy().to_string()),
        ];

        if self.genesis {
            args.push(OsString::from("--first"));
        }
        if self.local {
            args.push(OsString::from("--local"));
        }
        if let Some(node_port) = self.node_port {
            args.push(OsString::from("--port"));
            args.push(OsString::from(node_port.to_string()));
        }

        if !self.bootstrap_peers.is_empty() {
            let peers_str = self
                .bootstrap_peers
                .iter()
                .map(|peer| peer.to_string())
                .collect::<Vec<_>>()
                .join(",");
            args.push(OsString::from("--peer"));
            args.push(OsString::from(peers_str));
        }

        let mut service_ctx = ServiceInstallCtx {
            label: label.clone(),
            program: self.safenode_path.to_path_buf(),
            args,
            contents: None,
            username: Some(self.service_user.to_string()),
            working_directory: None,
            environment: self.env_variables,
        };
        // Temporary fix to enable the restart cmd to properly restart a running service.
        // 'ServiceInstallCtx::content' will override the other passed in fields.
        #[cfg(target_os = "linux")]
        {
            use std::fmt::Write;
            let mut service = String::new();

            let _ = writeln!(service, "[Unit]");
            let _ = writeln!(
                service,
                "Description={}",
                service_ctx.label.to_script_name()
            );
            let _ = writeln!(service, "[Service]");
            let program = service_ctx.program.to_string_lossy();
            let args = service_ctx
                .args
                .clone()
                .into_iter()
                .map(|a| a.to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            let _ = writeln!(service, "ExecStart={program} {args}");
            if let Some(env_vars) = &service_ctx.environment {
                for (var, val) in env_vars {
                    let _ = writeln!(service, "Environment=\"{}={}\"", var, val);
                }
            }
            let _ = writeln!(service, "Restart=on-failure");
            let _ = writeln!(service, "User={}", self.service_user);
            let _ = writeln!(service, "KillMode=process"); // fixes the restart issue
            let _ = writeln!(service, "[Install]");
            let _ = writeln!(service, "WantedBy=multi-user.target");

            service_ctx.contents = Some(service);
        }
        #[cfg(not(target_os = "linux"))]
        {
            service_ctx.contents = None;
        }
        Ok(service_ctx)
    }
}

/// Set of config that is passed to the service `add()` fn
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

/// Set of config that is passed to the service `upgrade()` fn
pub struct UpgradeOptions {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub env_variables: Option<Vec<(String, String)>>,
    pub force: bool,
    pub start_node: bool,
    pub target_safenode_path: PathBuf,
    pub target_version: Version,
}
