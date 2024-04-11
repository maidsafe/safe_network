// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod control;
pub mod daemon;
pub mod error;
pub mod faucet;
pub mod node;
pub mod rpc;

pub mod safenode_manager_proto {
    tonic::include_proto!("safenode_manager_proto");
}

use crate::error::{Error, Result};
use async_trait::async_trait;
use libp2p::Multiaddr;
use semver::Version;
use serde::{Deserialize, Serialize};
use service_manager::ServiceInstallCtx;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub use daemon::{DaemonService, DaemonServiceData};
pub use faucet::{FaucetService, FaucetServiceData};
pub use node::{NodeService, NodeServiceData};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServiceStatus {
    /// The service has been added but not started for the first time
    Added,
    /// Last time we checked the service was running
    Running,
    /// The service has been stopped
    Stopped,
    /// The service has been removed
    Removed,
}

#[derive(Clone, Debug)]
pub enum UpgradeResult {
    Forced(String, String),
    NotRequired,
    Upgraded(String, String),
    UpgradedButNotStarted(String, String, String),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct UpgradeOptions {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub env_variables: Option<Vec<(String, String)>>,
    pub force: bool,
    pub start_service: bool,
    pub target_bin_path: PathBuf,
    pub target_version: Version,
}

#[async_trait]
pub trait ServiceStateActions {
    fn bin_path(&self) -> PathBuf;
    fn build_upgrade_install_context(&self, options: UpgradeOptions) -> Result<ServiceInstallCtx>;
    fn data_dir_path(&self) -> PathBuf;
    fn log_dir_path(&self) -> PathBuf;
    fn name(&self) -> String;
    fn pid(&self) -> Option<u32>;
    fn on_remove(&mut self);
    async fn on_start(&mut self) -> Result<()>;
    async fn on_stop(&mut self) -> Result<()>;
    fn set_version(&mut self, version: &str);
    fn status(&self) -> ServiceStatus;
    fn version(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusSummary {
    pub nodes: Vec<NodeServiceData>,
    pub daemon: Option<DaemonServiceData>,
    pub faucet: Option<FaucetServiceData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeRegistry {
    pub bootstrap_peers: Vec<Multiaddr>,
    pub daemon: Option<DaemonServiceData>,
    pub environment_variables: Option<Vec<(String, String)>>,
    pub faucet: Option<FaucetServiceData>,
    pub nodes: Vec<NodeServiceData>,
    pub save_path: PathBuf,
}

impl NodeRegistry {
    pub fn save(&self) -> Result<()> {
        let path = Path::new(&self.save_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string(self)?;
        let mut file = std::fs::File::create(self.save_path.clone())?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(NodeRegistry {
                bootstrap_peers: vec![],
                daemon: None,
                environment_variables: None,
                faucet: None,
                nodes: vec![],
                save_path: path.to_path_buf(),
            });
        }

        let mut file = std::fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // It's possible for the file to be empty if the user runs a `status` command before any
        // services were added.
        if contents.is_empty() {
            return Ok(NodeRegistry {
                bootstrap_peers: vec![],
                daemon: None,
                environment_variables: None,
                faucet: None,
                nodes: vec![],
                save_path: path.to_path_buf(),
            });
        }

        let registry = serde_json::from_str(&contents)?;
        Ok(registry)
    }

    pub fn to_status_summary(&self) -> StatusSummary {
        StatusSummary {
            nodes: self.nodes.clone(),
            daemon: self.daemon.clone(),
            faucet: self.faucet.clone(),
        }
    }
}

pub fn get_local_node_registry_path() -> Result<PathBuf> {
    let path = dirs_next::data_dir()
        .ok_or_else(|| Error::UserDataDirectoryNotObtainable)?
        .join("safe")
        .join("local_node_registry.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}
