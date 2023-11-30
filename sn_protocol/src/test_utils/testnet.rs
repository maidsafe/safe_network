// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::{
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

// The contents of the file stored by sn-testnet-deploy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentInventory {
    pub name: String,
    pub version_info: Option<(String, String)>,
    pub branch_info: Option<(String, String)>,
    pub vm_list: Vec<(String, IpAddr)>,
    pub rpc_endpoints: Vec<SocketAddr>,
    pub node_count: u16,
    pub ssh_user: String,
    pub genesis_multiaddr: String,
    pub peers: Vec<String>,
    pub faucet_address: String,
    pub uploaded_files: Vec<(String, String)>,
}

impl DeploymentInventory {
    pub fn load() -> Result<Self> {
        let path = Self::get_deployment_path()?;
        let inventory_file = std::fs::read(path)?;
        let inventory = serde_json::from_slice(&inventory_file)?;
        Ok(inventory)
    }

    // Read the path from the env variable SN_INVENTORY
    // Else read deployment name from SN_INVENTORY
    fn get_deployment_path() -> Result<PathBuf> {
        let sn_inventory = env::var("SN_INVENTORY")
        .map_err(|_| eyre!("SN_INVENTORY not set. Provide either the deployment name or the direct path to the inventory.json file"))?;
        let path_from_env = PathBuf::from(&sn_inventory);
        if path_from_env.exists() {
            Ok(path_from_env)
        } else {
            let path = dirs_next::data_dir()
                .ok_or_else(|| eyre!("Could not obtain data_dir"))?
                .join("safe")
                .join("testnet-deploy")
                .join(format!("{sn_inventory}-inventory.json"));
            if path.exists() {
                Ok(path)
            } else {
                Err(eyre!(
                    "Could not obtain the deployment path from SN_INVENTORY"
                ))
            }
        }
    }
}
