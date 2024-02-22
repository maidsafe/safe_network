// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::eyre::{eyre, Result};
use libp2p::PeerId;
use serde::{de, Deserialize, Deserializer};
use std::{
    collections::BTreeMap,
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

fn deserialize_peer_socket_map<'de, D>(
    deserializer: D,
) -> std::result::Result<BTreeMap<PeerId, SocketAddr>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: BTreeMap<String, SocketAddr> = BTreeMap::deserialize(deserializer)?;
    let s = s
        .into_iter()
        .map(|(peer_id, socket_addr)| {
            PeerId::from_str(&peer_id)
                .map_err(de::Error::custom)
                .map(|peer_id| (peer_id, socket_addr))
        })
        .collect::<std::result::Result<Vec<_>, D::Error>>()?;
    Ok(s.into_iter().collect())
}

// The contents of the file stored by sn-testnet-deploy.
#[derive(Clone, Debug, Deserialize)]
pub struct DeploymentInventory {
    pub name: String,
    pub version_info: Option<(String, String)>,
    pub branch_info: Option<(String, String)>,
    pub vm_list: Vec<(String, IpAddr)>,
    // The PeerIds are stored as strings in the inventory file. We convert that directly into PeerId for convenience.
    #[serde(deserialize_with = "deserialize_peer_socket_map")]
    pub rpc_endpoints: BTreeMap<PeerId, SocketAddr>,
    #[serde(deserialize_with = "deserialize_peer_socket_map")]
    pub manager_daemon_endpoints: BTreeMap<PeerId, SocketAddr>,
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
        println!("SN_INVENTORY var set to {path:?}");
        let inventory_file = std::fs::read(path)?;
        let inventory = serde_json::from_slice(&inventory_file)?;
        println!("Read DeploymentInventory");
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
