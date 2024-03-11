// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
use libp2p::Multiaddr;
use service_manager::{ServiceInstallCtx, ServiceLabel};
use std::{
    ffi::OsString,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

#[derive(Debug, PartialEq)]
pub struct InstallNodeServiceCtxBuilder {
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

pub struct AddServiceOptions {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub count: Option<u16>,
    pub env_variables: Option<Vec<(String, String)>>,
    pub genesis: bool,
    pub local: bool,
    pub node_port: Option<u16>,
    pub rpc_address: Option<Ipv4Addr>,
    pub safenode_bin_path: PathBuf,
    pub safenode_dir_path: PathBuf,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub url: Option<String>,
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
    pub faucet_download_bin_path: PathBuf,
    pub faucet_install_bin_path: PathBuf,
    pub local: bool,
    pub service_data_dir_path: PathBuf,
    pub service_log_dir_path: PathBuf,
    pub url: Option<String>,
    pub user: String,
    pub version: String,
}

pub struct AddDaemonServiceOptions {
    pub address: Ipv4Addr,
    pub port: u16,
    pub daemon_download_bin_path: PathBuf,
    pub daemon_install_bin_path: PathBuf,
    pub version: String,
}
