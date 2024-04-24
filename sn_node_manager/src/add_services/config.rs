// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::{eyre::eyre, Result};
use libp2p::Multiaddr;
use service_manager::{ServiceInstallCtx, ServiceLabel};
use std::{
    ffi::OsString,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

#[derive(Clone, Debug)]
pub enum PortRange {
    Single(u16),
    Range(u16, u16),
}

pub fn parse_port_range(s: &str) -> Result<PortRange> {
    if let Ok(port) = u16::from_str(s) {
        Ok(PortRange::Single(port))
    } else {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(eyre!("Port range must be in the format 'start-end'"));
        }
        let start = parts[0].parse::<u16>()?;
        let end = parts[1].parse::<u16>()?;
        if start >= end {
            return Err(eyre!("End port must be greater than start port"));
        }
        Ok(PortRange::Range(start, end))
    }
}

#[derive(Debug, PartialEq)]
pub struct InstallNodeServiceCtxBuilder {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub data_dir_path: PathBuf,
    pub env_variables: Option<Vec<(String, String)>>,
    pub genesis: bool,
    pub local: bool,
    pub log_dir_path: PathBuf,
    pub name: String,
    pub metrics_port: Option<u16>,
    pub node_port: Option<u16>,
    pub rpc_socket_addr: SocketAddr,
    pub safenode_path: PathBuf,
    pub service_user: String,
}

impl InstallNodeServiceCtxBuilder {
    pub fn build(self) -> Result<ServiceInstallCtx> {
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
        if let Some(metrics_port) = self.metrics_port {
            args.push(OsString::from("--metrics-server-port"));
            args.push(OsString::from(metrics_port.to_string()));
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

        Ok(ServiceInstallCtx {
            label: label.clone(),
            program: self.safenode_path.to_path_buf(),
            args,
            contents: None,
            username: Some(self.service_user.to_string()),
            working_directory: None,
            environment: self.env_variables,
        })
    }
}

pub struct AddNodeServiceOptions {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub count: Option<u16>,
    pub delete_safenode_src: bool,
    pub env_variables: Option<Vec<(String, String)>>,
    pub genesis: bool,
    pub local: bool,
    pub metrics_port: Option<PortRange>,
    pub node_port: Option<PortRange>,
    pub rpc_address: Option<Ipv4Addr>,
    pub rpc_port: Option<PortRange>,
    pub safenode_src_path: PathBuf,
    pub safenode_dir_path: PathBuf,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub user: String,
    pub version: String,
}

#[derive(Debug, PartialEq)]
pub struct InstallFaucetServiceCtxBuilder {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub env_variables: Option<Vec<(String, String)>>,
    pub faucet_path: PathBuf,
    pub local: bool,
    pub log_dir_path: PathBuf,
    pub name: String,
    pub service_user: String,
}

impl InstallFaucetServiceCtxBuilder {
    pub fn build(self) -> Result<ServiceInstallCtx> {
        let mut args = vec![
            OsString::from("--log-output-dest"),
            OsString::from(self.log_dir_path.to_string_lossy().to_string()),
        ];

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

        args.push(OsString::from("server"));

        Ok(ServiceInstallCtx {
            label: self.name.parse()?,
            program: self.faucet_path.to_path_buf(),
            args,
            contents: None,
            username: Some(self.service_user.to_string()),
            working_directory: None,
            environment: self.env_variables,
        })
    }
}

pub struct AddFaucetServiceOptions {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub env_variables: Option<Vec<(String, String)>>,
    pub faucet_install_bin_path: PathBuf,
    pub faucet_src_bin_path: PathBuf,
    pub local: bool,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub user: String,
    pub version: String,
}

pub struct AddDaemonServiceOptions {
    pub address: Ipv4Addr,
    pub env_variables: Option<Vec<(String, String)>>,
    pub daemon_install_bin_path: PathBuf,
    pub daemon_src_bin_path: PathBuf,
    pub port: u16,
    pub user: String,
    pub version: String,
}
