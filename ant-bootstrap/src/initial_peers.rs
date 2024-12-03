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
use url::Url;

/// The name of the environment variable that can be used to pass peers to the node.
pub const ANT_PEERS_ENV: &str = "ANT_PEERS";

/// Command line arguments for peer configuration
#[derive(Args, Debug, Clone, Default)]
pub struct PeersArgs {
    /// Set to indicate this is the first node in a new network
    ///
    /// If this argument is used, any others will be ignored because they do not apply to the first
    /// node.
    #[clap(long)]
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
        conflicts_with = "first",
        value_parser = parse_multiaddr_str
    )]
    pub addrs: Vec<Multiaddr>,
    /// Specify the URL to fetch the network contacts from.
    ///
    /// The URL can point to a text file containing Multiaddresses separated by newline character, or
    /// a bootstrap cache JSON file.
    #[clap(long, conflicts_with = "first")]
    pub network_contacts_url: Option<Url>,
    /// Set to indicate this is a local network. You could also set the `local` feature flag to set this to true.
    ///
    /// This would use mDNS for peer discovery.
    #[clap(long, conflicts_with = "network_contacts_url")]
    pub local: bool,
    /// Set to indicate this is a testnet.
    ///
    /// This disables fetching peers from the mainnet network contacts.
    #[clap(name = "testnet", long, conflicts_with = "network_contacts_url")]
    pub disable_mainnet_contacts: bool,

    /// Set to not load the bootstrap addresses from the local cache.
    #[clap(long)]
    pub ignore_cache: bool,
}
impl PeersArgs {
    /// Get bootstrap peers
    /// Order of precedence:
    /// 1. Addresses from arguments
    /// 2. Addresses from environment variable SAFE_PEERS
    /// 3. Addresses from cache
    /// 4. Addresses from network contacts URL
    pub async fn get_bootstrap_addr(&self) -> Result<Vec<BootstrapAddr>> {
        self.get_bootstrap_addr_and_initialize_cache(None).await
    }

    pub async fn get_addrs(&self) -> Result<Vec<Multiaddr>> {
        Ok(self
            .get_bootstrap_addr()
            .await?
            .into_iter()
            .map(|addr| addr.addr)
            .collect())
    }

    /// Helper function to fetch bootstrap addresses and initialize cache based on the passed in args.
    pub(crate) async fn get_bootstrap_addr_and_initialize_cache(
        &self,
        mut cache: Option<&mut BootstrapCacheStore>,
    ) -> Result<Vec<BootstrapAddr>> {
        // If this is the first node, return an empty list
        if self.first {
            info!("First node in network, no initial bootstrap peers");
            if let Some(cache) = cache {
                info!("Clearing cache for 'first' node");
                cache.clear_peers_and_save().await?;
            }
            return Ok(vec![]);
        }

        // If local mode is enabled, return empty store (will use mDNS)
        if self.local || cfg!(feature = "local") {
            info!("Local mode enabled, using only local discovery.");
            if let Some(cache) = cache {
                info!("Setting config to not write to cache, as 'local' mode is enabled");
                cache.config.disable_cache_writing = true;
            }
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

        // If we have a network contacts URL, fetch addrs from there.
        if let Some(url) = self.network_contacts_url.clone() {
            info!("Fetching bootstrap address from network contacts URL: {url}",);
            let contacts_fetcher = ContactsFetcher::with_endpoints(vec![url])?;
            let addrs = contacts_fetcher.fetch_bootstrap_addresses().await?;
            bootstrap_addresses.extend(addrs);
        }

        // Return here if we fetched peers from the args
        if !bootstrap_addresses.is_empty() {
            if let Some(cache) = cache.as_mut() {
                info!("Initializing cache with bootstrap addresses from arguments");
                for addr in &bootstrap_addresses {
                    cache.add_addr(addr.addr.clone());
                }
            }
            return Ok(bootstrap_addresses);
        }

        // load from cache if present

        if !self.ignore_cache {
            let cfg = if let Some(cache) = cache.as_ref() {
                Some(cache.config.clone())
            } else {
                BootstrapCacheConfig::default_config().ok()
            };
            if let Some(cfg) = cfg {
                info!("Loading bootstrap addresses from cache");
                if let Ok(data) = BootstrapCacheStore::load_cache_data(&cfg).await {
                    if let Some(cache) = cache.as_mut() {
                        info!("Initializing cache with bootstrap addresses from cache");
                        cache.data = data.clone();
                        cache.old_shared_state = data.clone();
                    }

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
            return Ok(bootstrap_addresses);
        }

        if !self.disable_mainnet_contacts {
            let contacts_fetcher = ContactsFetcher::with_mainnet_endpoints()?;
            let addrs = contacts_fetcher.fetch_bootstrap_addresses().await?;
            if let Some(cache) = cache.as_mut() {
                info!("Initializing cache with bootstrap addresses from mainnet contacts");
                for addr in addrs.iter() {
                    cache.add_addr(addr.addr.clone());
                }
            }
            bootstrap_addresses = addrs;
        }

        if !bootstrap_addresses.is_empty() {
            Ok(bootstrap_addresses)
        } else {
            error!("No initial bootstrap peers found through any means");
            Err(Error::NoBootstrapPeersFound)
        }
    }
}

pub fn parse_multiaddr_str(addr: &str) -> std::result::Result<Multiaddr, libp2p::multiaddr::Error> {
    addr.parse::<Multiaddr>()
}
