// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    craft_valid_multiaddr, craft_valid_multiaddr_from_str,
    error::{Error, Result},
    BootstrapAddr, BootstrapCacheConfig, BootstrapCacheStore, ContactsFetcher,
};
use clap::Args;
use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};
use url::Url;

/// The name of the environment variable that can be used to pass peers to the node.
pub const ANT_PEERS_ENV: &str = "ANT_PEERS";

/// Command line arguments for peer configuration
#[derive(Args, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PeersArgs {
    /// Set to indicate this is the first node in a new network
    ///
    /// If this argument is used, any others will be ignored because they do not apply to the first
    /// node.
    #[clap(long, default_value = "false")]
    pub first: bool,
    /// Addr(s) to use for bootstrap, in a 'multiaddr' format containing the peer ID.
    ///
    /// A multiaddr looks like
    /// '/ip4/1.2.3.4/tcp/1200/tcp/p2p/12D3KooWRi6wF7yxWLuPSNskXc6kQ5cJ6eaymeMbCRdTnMesPgFx' where
    /// `1.2.3.4` is the IP, `1200` is the port and the (optional) last part is the peer ID.
    ///
    /// This argument can be provided multiple times to connect to multiple peers.
    ///
    /// Alternatively, the `ANT_PEERS` environment variable can provide a comma-separated peer
    /// list.
    #[clap(
        long = "peer",
        value_name = "multiaddr",
        value_delimiter = ',',
        conflicts_with = "first"
    )]
    pub addrs: Vec<Multiaddr>,
    /// Specify the URL to fetch the network contacts from.
    ///
    /// The URL can point to a text file containing Multiaddresses separated by newline character, or
    /// a bootstrap cache JSON file.
    #[clap(long, conflicts_with = "first", value_delimiter = ',')]
    pub network_contacts_url: Vec<String>,
    /// Set to indicate this is a local network. You could also set the `local` feature flag to set this to true.
    ///
    /// This would use mDNS for peer discovery.
    #[clap(long, conflicts_with = "network_contacts_url", default_value = "false")]
    pub local: bool,
    /// Set to indicate this is a testnet.
    ///
    /// This disables fetching peers from the mainnet network contacts.
    #[clap(name = "testnet", long)]
    pub disable_mainnet_contacts: bool,

    /// Set to not load the bootstrap addresses from the local cache.
    #[clap(long, default_value = "false")]
    pub ignore_cache: bool,
}
impl PeersArgs {
    /// Get bootstrap peers
    /// Order of precedence:
    /// 1. Addresses from arguments
    /// 2. Addresses from environment variable SAFE_PEERS
    /// 3. Addresses from cache
    /// 4. Addresses from network contacts URL
    pub async fn get_addrs(&self, config: Option<BootstrapCacheConfig>) -> Result<Vec<Multiaddr>> {
        Ok(self
            .get_bootstrap_addr(config)
            .await?
            .into_iter()
            .map(|addr| addr.addr)
            .collect())
    }

    /// Get bootstrap peers
    /// Order of precedence:
    /// 1. Addresses from arguments
    /// 2. Addresses from environment variable SAFE_PEERS
    /// 3. Addresses from cache
    /// 4. Addresses from network contacts URL
    pub async fn get_bootstrap_addr(
        &self,
        config: Option<BootstrapCacheConfig>,
    ) -> Result<Vec<BootstrapAddr>> {
        // If this is the first node, return an empty list
        if self.first {
            info!("First node in network, no initial bootstrap peers");
            return Ok(vec![]);
        }

        // If local mode is enabled, return empty store (will use mDNS)
        if self.local || cfg!(feature = "local") {
            info!("Local mode enabled, using only local discovery.");
            return Ok(vec![]);
        }

        let mut bootstrap_addresses = vec![];

        // Add addrs from arguments if present
        for addr in &self.addrs {
            if let Some(addr) = craft_valid_multiaddr(addr, false) {
                info!("Adding addr from arguments: {addr}");
                bootstrap_addresses.push(BootstrapAddr::new(addr));
            } else {
                warn!("Invalid multiaddress format from arguments: {addr}");
            }
        }
        // Read from ANT_PEERS environment variable if present
        bootstrap_addresses.extend(Self::read_bootstrap_addr_from_env());

        // If we have a network contacts URL, fetch addrs from there.
        if !self.network_contacts_url.is_empty() {
            info!(
                "Fetching bootstrap address from network contacts URLs: {:?}",
                self.network_contacts_url
            );
            let addrs = self
                .network_contacts_url
                .iter()
                .map(|url| url.parse::<Url>().map_err(|_| Error::FailedToParseUrl))
                .collect::<Result<Vec<Url>>>()?;
            let contacts_fetcher = ContactsFetcher::with_endpoints(addrs)?;
            let addrs = contacts_fetcher.fetch_bootstrap_addresses().await?;
            bootstrap_addresses.extend(addrs);
        }

        // Return here if we fetched peers from the args
        if !bootstrap_addresses.is_empty() {
            bootstrap_addresses.sort_by_key(|addr| addr.failure_rate() as u64);
            return Ok(bootstrap_addresses);
        }

        // load from cache if present
        if !self.ignore_cache {
            let cfg = if let Some(config) = config {
                Some(config)
            } else {
                BootstrapCacheConfig::default_config().ok()
            };
            if let Some(cfg) = cfg {
                info!("Loading bootstrap addresses from cache");
                if let Ok(data) = BootstrapCacheStore::load_cache_data(&cfg) {
                    bootstrap_addresses = data
                        .peers
                        .into_iter()
                        .filter_map(|(_, addrs)| {
                            addrs
                                .0
                                .into_iter()
                                .min_by_key(|addr| addr.failure_rate() as u64)
                        })
                        .collect();
                }
            }
        }

        if !bootstrap_addresses.is_empty() {
            bootstrap_addresses.sort_by_key(|addr| addr.failure_rate() as u64);
            return Ok(bootstrap_addresses);
        }

        if !self.disable_mainnet_contacts {
            let contacts_fetcher = ContactsFetcher::with_mainnet_endpoints()?;
            let addrs = contacts_fetcher.fetch_bootstrap_addresses().await?;
            bootstrap_addresses = addrs;
        }

        if !bootstrap_addresses.is_empty() {
            bootstrap_addresses.sort_by_key(|addr| addr.failure_rate() as u64);
            Ok(bootstrap_addresses)
        } else {
            error!("No initial bootstrap peers found through any means");
            Err(Error::NoBootstrapPeersFound)
        }
    }

    pub fn read_addr_from_env() -> Vec<Multiaddr> {
        Self::read_bootstrap_addr_from_env()
            .into_iter()
            .map(|addr| addr.addr)
            .collect()
    }

    pub fn read_bootstrap_addr_from_env() -> Vec<BootstrapAddr> {
        let mut bootstrap_addresses = Vec::new();
        // Read from ANT_PEERS environment variable if present
        if let Ok(addrs) = std::env::var(ANT_PEERS_ENV) {
            for addr_str in addrs.split(',') {
                if let Some(addr) = craft_valid_multiaddr_from_str(addr_str, false) {
                    info!("Adding addr from environment variable: {addr}");
                    bootstrap_addresses.push(BootstrapAddr::new(addr));
                } else {
                    warn!("Invalid multiaddress format from environment variable: {addr_str}");
                }
            }
        }
        bootstrap_addresses
    }
}
