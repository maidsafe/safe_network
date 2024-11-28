// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::{Error, Result};
use ant_protocol::version::{get_key_version_str, get_truncate_version_str};
use std::path::{Path, PathBuf};
use url::Url;

const MAX_PEERS: usize = 1500;
// const UPDATE_INTERVAL: Duration = Duration::from_secs(60);

/// Configuration for the bootstrap cache
#[derive(Clone, Debug)]
pub struct BootstrapConfig {
    /// List of bootstrap endpoints to fetch peer information from
    pub endpoints: Vec<Url>,
    /// Maximum number of peers to keep in the cache
    pub max_peers: usize,
    /// Path to the bootstrap cache file
    pub cache_file_path: PathBuf,
    // /// How often to update the cache (in seconds)
    // pub update_interval: Duration,
    /// Flag to disable writing to the cache file
    pub disable_cache_writing: bool,
}

impl BootstrapConfig {
    /// Creates a new BootstrapConfig with default settings
    pub fn default_config() -> Result<Self> {
        Ok(Self {
            endpoints: vec![
                "https://sn-testnet.s3.eu-west-2.amazonaws.com/bootstrap_cache.json"
                    .parse()
                    .expect("Failed to parse URL"),
                "https://sn-testnet.s3.eu-west-2.amazonaws.com/network-contacts"
                    .parse()
                    .expect("Failed to parse URL"),
            ],
            max_peers: MAX_PEERS,
            cache_file_path: default_cache_path()?,
            // update_interval: UPDATE_INTERVAL,
            disable_cache_writing: false,
        })
    }

    /// Creates a new BootstrapConfig with empty settings
    pub fn empty() -> Self {
        Self {
            endpoints: vec![],
            max_peers: MAX_PEERS,
            cache_file_path: PathBuf::new(),
            // update_interval: UPDATE_INTERVAL,
            disable_cache_writing: false,
        }
    }

    /// Update the config with custom endpoints
    pub fn with_endpoints(mut self, endpoints: Vec<Url>) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// Update the config with default endpoints
    pub fn with_default_endpoints(mut self) -> Self {
        self.endpoints = vec![
            "https://sn-testnet.s3.eu-west-2.amazonaws.com/bootstrap_cache.json"
                .parse()
                .expect("Failed to parse URL"),
            "https://sn-testnet.s3.eu-west-2.amazonaws.com/network-contacts"
                .parse()
                .expect("Failed to parse URL"),
        ];
        self
    }

    /// Update the config with a custom cache file path
    pub fn with_cache_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.cache_file_path = path.as_ref().to_path_buf();
        self
    }

    /// Sets the maximum number of peers
    pub fn with_max_peers(mut self, max_peers: usize) -> Self {
        self.max_peers = max_peers;
        self
    }

    // /// Sets the update interval
    // pub fn with_update_interval(mut self, update_interval: Duration) -> Self {
    //     self.update_interval = update_interval;
    //     self
    // }

    /// Sets the flag to disable writing to the cache file
    pub fn with_disable_cache_writing(mut self, disable: bool) -> Self {
        self.disable_cache_writing = disable;
        self
    }
}

/// Returns the default path for the bootstrap cache file
fn default_cache_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| Error::CouldNotObtainDataDir)?
        .join("autonomi")
        .join("bootstrap_cache");

    std::fs::create_dir_all(&dir)?;

    let network_id = format!("{}_{}", get_key_version_str(), get_truncate_version_str());
    let path = dir.join(format!("bootstrap_cache_{}.json", network_id));

    Ok(path)
}
