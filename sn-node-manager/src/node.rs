use color_eyre::Result;
use libp2p_identity::PeerId;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::io::{Read, Write};
use std::path::Path;
use std::str::FromStr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeStatus {
    /// The node service has been installed but not started for the first time
    Installed,
    /// Last time we checked the service was running
    Running,
    /// The node service has been stopped
    Stopped,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledNode {
    pub version: String,
    pub service_name: String,
    pub user: String,
    pub number: u16,
    pub port: u16,
    pub rpc_port: u16,
    pub status: NodeStatus,
    pub pid: Option<u32>,
    #[serde(
        serialize_with = "serialize_peer_id",
        deserialize_with = "deserialize_peer_id"
    )]
    pub peer_id: Option<PeerId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeRegistry {
    pub installed_nodes: Vec<InstalledNode>,
}

impl NodeRegistry {
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string(self)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(NodeRegistry {
                installed_nodes: vec![],
            });
        }
        let mut file = std::fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let registry = serde_json::from_str(&contents)?;
        Ok(registry)
    }
}
