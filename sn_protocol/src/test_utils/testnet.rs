// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::{env, net::IpAddr, path::PathBuf};

// The contents of the file stored by sn-testnet-deploy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentInventory {
    pub name: String,
    pub version_info: Option<(String, String)>,
    pub branch_info: Option<(String, String)>,
    pub vm_list: Vec<(String, IpAddr)>,
    pub node_count: u16,
    pub ssh_user: String,
    pub genesis_multiaddr: String,
    pub peers: Vec<String>,
    pub faucet_address: String,
    pub uploaded_files: Vec<(String, String)>,
}

// Read the path from the env variable SN_INVENTORY
// Else read deployment name from SN_INVENTORY
fn get_deployment_path() -> Result<PathBuf> {
    let sn_inventory = env::var("SN_INVENTORY")?;
    let path = PathBuf::from(sn_inventory);
    if path.exists() {
        Ok(path)
    } else {
        // let path = dirs_next::data_dir()
        //     .ok_or_else(|| Error::CouldNotRetrieveDataDirectory)?
        //     .join("safe")
        //     .join("testnet-deploy");
        // if !path.exists() {
        //     std::fs::create_dir_all(path.clone())?;
        // }
        // read from the data dir
        Ok(path.join(""))
    }
}
