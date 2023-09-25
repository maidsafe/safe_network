// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Args;
#[cfg(feature = "network-contacts")]
use color_eyre::eyre::Context;
use color_eyre::{eyre::eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr};
use tracing::*;
#[cfg(feature = "network-contacts")]
use url::Url;

#[cfg(feature = "network-contacts")]
// URL containing the multi-addresses of the bootstrap nodes.
const NETWORK_CONTACTS_URL: &str = "https://sn-testnet.s3.eu-west-2.amazonaws.com/network-contacts";

#[cfg(feature = "network-contacts")]
// The maximum number of retries to be performed while trying to fetch the network contacts file.
const MAX_NETWORK_CONTACTS_GET_RETRIES: usize = 3;

/// The name of the environment variable that can be used to pass peers to the node.
pub const SAFE_PEERS_ENV: &str = "SAFE_PEERS";

#[derive(Args, Debug)]
pub struct PeersArgs {
    /// Peer(s) to use for bootstrap, in a 'multiaddr' format containing the peer ID.
    ///
    /// A multiaddr looks like '/ip4/1.2.3.4/tcp/1200/tcp/p2p/12D3KooWRi6wF7yxWLuPSNskXc6kQ5cJ6eaymeMbCRdTnMesPgFx'
    /// where `1.2.3.4` is the IP, `1200` is the port and the (optional) last part is the peer ID.
    ///
    /// This argument can be provided multiple times to connect to multiple peers.
    ///
    /// Peers can also be provided by an environment variable (see below), but the
    /// command-line argument (`--peer`) takes precedence. To pass multiple peers with the
    /// environment variable, separate them with commas.
    #[clap(long = "peer", value_name = "multiaddr", env = SAFE_PEERS_ENV, value_delimiter = ',', value_parser = parse_peer_addr)]
    pub peers: Vec<Multiaddr>,

    /// Specify the URL to fetch the network contacts from.
    ///
    /// This argument will be overridden if the "peers" argument is set or if the `local-discovery` feature flag is
    /// enabled.
    #[cfg(feature = "network-contacts")]
    #[clap(long)]
    pub network_contacts_url: Option<Url>,
}

/// Parses PeersArgs
///
/// The order of precedence for the bootstrap peers are `--peer` arg, `SAFE_PEERS` env variable, `local-discovery` flag
/// and `network-contacts` flag respectively. The later ones are ignored if one of the prior option is used.
pub async fn parse_peers_args(args: PeersArgs) -> Result<Vec<Multiaddr>> {
    if !args.peers.is_empty() {
        info!("Using passed peers or SAFE_PEERS env variable");
        Ok(args.peers)
    } else if cfg!(feature = "local-discovery") {
        info!("No peers given. As `local-discovery` feature is enabled, we will be attempt to connect to the network using mDNS.");
        Ok(vec![])
    } else if cfg!(feature = "network-contacts") {
        #[cfg(feature = "network-contacts")]
        let peers = {
            info!("Trying to fetch the bootstrap peers from {NETWORK_CONTACTS_URL}");
            println!("Trying to fetch the bootstrap peers from {NETWORK_CONTACTS_URL}");
            let url = args
                .network_contacts_url
                .unwrap_or(Url::parse(NETWORK_CONTACTS_URL)?);
            let peers = get_bootstrap_peers_from_url(url)
                .await
                .wrap_err("Error while fetching bootstrap peers from Network contacts URL")?;

            if peers.is_empty() {
                return Err(color_eyre::eyre::eyre!(
                    "Could not obtain a single valid multi-addr from URL {NETWORK_CONTACTS_URL}"
                ));
            } else {
                Ok(peers)
            }
        };
        // should not be reachable, but needed for the compiler to be happy.
        #[cfg(not(feature = "network-contacts"))]
        let peers = Ok(vec![]);

        peers
    } else {
        let err_str = "No peers given, 'local-discovery' and 'network-contacts' feature flags are disabled. We cannot connect to the network.";
        error!("{err_str}");
        return Err(color_eyre::eyre::eyre!("{err_str}"));
    }
}

/// Parse strings like `1.2.3.4:1234` and `/ip4/1.2.3.4/tcp/1234` into a (TCP) multiaddr.
pub fn parse_peer_addr(addr: &str) -> Result<Multiaddr> {
    // Parse valid IPv4 socket address, e.g. `1.2.3.4:1234`.
    if let Ok(addr) = addr.parse::<std::net::SocketAddrV4>() {
        #[cfg(not(feature = "quic"))]
        // Turn the address into a `/ip4/<ip>/tcp/<port>` multiaddr.
        let multiaddr = Multiaddr::from(*addr.ip()).with(Protocol::Tcp(addr.port()));
        #[cfg(feature = "quic")]
        // Turn the address into a `/ip4/<ip>/udp/<port>/quic-v1` multiaddr.
        let multiaddr = Multiaddr::from(*addr.ip())
            .with(Protocol::Udp(addr.port()))
            .with(Protocol::QuicV1);
        return Ok(multiaddr);
    }

    // Parse any valid multiaddr string, e.g. `/ip4/1.2.3.4/tcp/1234/p2p/<peer_id>`.
    if let Ok(addr) = addr.parse::<Multiaddr>() {
        return Ok(addr);
    }

    Err(eyre!("invalid multiaddr or socket address"))
}

#[cfg(feature = "network-contacts")]
/// Get bootstrap peers from the Network contacts file stored in the given URL.
///
/// If URL is not provided, the addresses are fetched from the default NETWORK_CONTACTS_URL
async fn get_bootstrap_peers_from_url(url: Url) -> Result<Vec<Multiaddr>> {
    let mut retries = 0;

    loop {
        let response = reqwest::get(url.clone()).await;

        match response {
            Ok(response) => {
                let mut multi_addresses = Vec::new();
                if response.status().is_success() {
                    let text = response.text().await?;
                    trace!("Got bootstrap peers from {url}: {text}");
                    // example of contacts file exists in resources/network-contacts-examples
                    for addr in text.split('\n') {
                        // ignore empty/last lines
                        if addr.is_empty() {
                            continue;
                        }

                        debug!("Attempting to parse {addr}");
                        multi_addresses.push(parse_peer_addr(addr)?);
                    }
                    if !multi_addresses.is_empty() {
                        trace!("Successfully got bootstrap peers from URL {multi_addresses:?}");
                        return Ok(multi_addresses);
                    } else {
                        return Err(color_eyre::eyre::eyre!(
                            "Could not obtain a single valid multi-addr from URL {NETWORK_CONTACTS_URL}"
                        ));
                    }
                } else {
                    retries += 1;
                    if retries >= MAX_NETWORK_CONTACTS_GET_RETRIES {
                        return Err(color_eyre::eyre::eyre!(
                            "Could not GET network contacts from {NETWORK_CONTACTS_URL} after {MAX_NETWORK_CONTACTS_GET_RETRIES} retries",
                        ));
                    }
                }
            }
            Err(err) => {
                retries += 1;
                if retries >= MAX_NETWORK_CONTACTS_GET_RETRIES {
                    return Err(color_eyre::eyre::eyre!(
                        "Failed to perform request to {NETWORK_CONTACTS_URL} after {MAX_NETWORK_CONTACTS_GET_RETRIES} retries due to: {err:?}"
                    ));
                }
            }
        }
        trace!("Failed to get bootstrap peers from URL, retrying {retries}/{MAX_NETWORK_CONTACTS_GET_RETRIES}");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
