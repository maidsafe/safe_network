// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Result as ProtocolResult, Error};
use color_eyre::Result;
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    io::{Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// The node service has been added but not started for the first time
    Added,
    /// Last time we checked the service was running
    Running,
    /// The node service has been stopped
    Stopped,
    /// The node service has been removed
    Removed,
}

fn serialize_peer_id<S>(value: &Option<PeerId>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(peer_id) = value {
        return serializer.serialize_str(&peer_id.to_string());
    }
    serializer.serialize_none()
}

fn deserialize_peer_id<'de, D>(deserializer: D) -> Result<Option<PeerId>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    if let Some(peer_id_str) = s {
        PeerId::from_str(&peer_id_str)
            .map(Some)
            .map_err(DeError::custom)
    } else {
        Ok(None)
    }
}

fn serialize_connected_peers<S>(
    connected_peers: &Option<Vec<PeerId>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match connected_peers {
        Some(peers) => {
            let peer_strs: Vec<String> = peers.iter().map(|p| p.to_string()).collect();
            serializer.serialize_some(&peer_strs)
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_connected_peers<'de, D>(deserializer: D) -> Result<Option<Vec<PeerId>>, D::Error>
where
    D: Deserializer<'de>,
{
    let vec: Option<Vec<String>> = Option::deserialize(deserializer)?;
    match vec {
        Some(peer_strs) => {
            let peers: Result<Vec<PeerId>, _> = peer_strs
                .into_iter()
                .map(|s| PeerId::from_str(&s).map_err(DeError::custom))
                .collect();
            peers.map(Some)
        }
        None => Ok(None),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub genesis: bool,
    pub local: bool,
    pub version: String,
    pub service_name: String,
    pub user: String,
    pub number: u16,
    pub rpc_socket_addr: SocketAddr,
    pub status: NodeStatus,
    pub pid: Option<u32>,
    #[serde(
        serialize_with = "serialize_peer_id",
        deserialize_with = "deserialize_peer_id"
    )]
    pub peer_id: Option<PeerId>,
    pub listen_addr: Option<Vec<Multiaddr>>,
    pub data_dir_path: PathBuf,
    pub log_dir_path: PathBuf,
    pub safenode_path: PathBuf,
    #[serde(
        serialize_with = "serialize_connected_peers",
        deserialize_with = "deserialize_connected_peers"
    )]
    pub connected_peers: Option<Vec<PeerId>>,
}

impl Node {
    /// Returns the UDP port from our node's listen address.
    pub fn get_safenode_port(&self) -> ProtocolResult<u16> {
        // assuming the listening addr contains /ip4/127.0.0.1/udp/56215/quic-v1/p2p/<peer_id>
        if let Some(multi_addrs) = &self.listen_addr {
            for addr in multi_addrs {
                for protocol in addr.iter() {
                    if let Protocol::Udp(port) = protocol {
                        return Ok(port);
                    }
                }
            }
        }
        Err(Error::CouldNotObtainPortFromMultiAddr)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeRegistry {
    pub save_path: PathBuf,
    pub nodes: Vec<Node>,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub environment_variables: Option<Vec<(String, String)>>,
    pub faucet_pid: Option<u32>,
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
                save_path: path.to_path_buf(),
                nodes: vec![],
                bootstrap_peers: vec![],
                environment_variables: None,
                faucet_pid: None,
            });
        }

        let mut file = std::fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // It's possible for the file to be empty if the user runs a `status` command before any
        // services were added.
        if contents.is_empty() {
            return Ok(NodeRegistry {
                save_path: path.to_path_buf(),
                nodes: vec![],
                bootstrap_peers: vec![],
                environment_variables: None,
                faucet_pid: None,
            });
        }

        let registry = serde_json::from_str(&contents)?;
        Ok(registry)
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
