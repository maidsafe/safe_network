// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod error;

use crate::error::{Error, Result};
use clap::Args;
#[cfg(feature = "network-contacts")]
use lazy_static::lazy_static;
use libp2p::{multiaddr::Protocol, Multiaddr};
use rand::{seq::SliceRandom, thread_rng};
#[cfg(feature = "network-contacts")]
use sn_protocol::version::get_network_version;
use tracing::*;
#[cfg(feature = "network-contacts")]
use url::Url;

#[cfg(feature = "network-contacts")]
lazy_static! {
    // URL containing the multi-addresses of the bootstrap nodes.
    pub static ref NETWORK_CONTACTS_URL: String = {
        let version = get_network_version();
        let version_prefix = if !version.is_empty() { format!("{version}-") } else { version.to_string() };
        format!("https://sn-testnet.s3.eu-west-2.amazonaws.com/{version_prefix}network-contacts")
    };
}

#[cfg(feature = "network-contacts")]
// The maximum number of retries to be performed while trying to fetch the network contacts file.
const MAX_NETWORK_CONTACTS_GET_RETRIES: usize = 3;

/// The name of the environment variable that can be used to pass peers to the node.
pub const SAFE_PEERS_ENV: &str = "SAFE_PEERS";

#[derive(Args, Debug, Default, Clone)]
pub struct PeersArgs {
    /// Set to indicate this is the first node in a new network
    ///
    /// If this argument is used, any others will be ignored because they do not apply to the first
    /// node.
    #[clap(long)]
    pub first: bool,
    /// Peer(s) to use for bootstrap, in a 'multiaddr' format containing the peer ID.
    ///
    /// A multiaddr looks like
    /// '/ip4/1.2.3.4/tcp/1200/tcp/p2p/12D3KooWRi6wF7yxWLuPSNskXc6kQ5cJ6eaymeMbCRdTnMesPgFx' where
    /// `1.2.3.4` is the IP, `1200` is the port and the (optional) last part is the peer ID.
    ///
    /// This argument can be provided multiple times to connect to multiple peers.
    ///
    /// Alternatively, the `SAFE_PEERS` environment variable can provide a comma-separated peer
    /// list.
    #[clap(long = "peer", env = "SAFE_PEERS", value_name = "multiaddr", value_delimiter = ',', value_parser = parse_peer_addr, conflicts_with = "first")]
    pub peers: Vec<Multiaddr>,

    /// Specify the URL to fetch the network contacts from.
    ///
    /// This argument will be overridden if the "peers" argument is set or if the `local-discovery`
    /// feature flag is enabled.
    #[cfg(feature = "network-contacts")]
    #[clap(long, conflicts_with = "first")]
    pub network_contacts_url: Option<Url>,
}

/// Gets the peers based on the arguments provided.
///
/// If the `--first` flag is used, no peers will be provided.
///
/// Otherwise, peers are obtained in the following order of precedence:
/// * The `--peer` argument.
/// * The `SAFE_PEERS` environment variable.
/// * Using the `local-discovery` feature, which will return an empty peer list.
/// * Using the `network-contacts` feature, which will download the peer list from a file on S3.
///
/// Note: the current behaviour is that `--peer` and `SAFE_PEERS` will be combined. Some tests
/// currently rely on this. We will change it soon.
pub async fn get_peers_from_args(args: PeersArgs) -> Result<Vec<Multiaddr>> {
    if args.first {
        return Ok(vec![]);
    }

    let mut peers = if !args.peers.is_empty() {
        info!("Using peers supplied with the --peer argument(s) or SAFE_PEERS");
        args.peers
    } else if cfg!(feature = "local-discovery") {
        info!("No peers given");
        info!(
            "The `local-discovery` feature is enabled, so peers will be discovered through mDNS."
        );
        return Ok(vec![]);
    } else if cfg!(feature = "network-contacts") {
        get_network_contacts(&args).await?
    } else {
        vec![]
    };

    if peers.is_empty() {
        error!("Peers not obtained through any available options");
        return Err(Error::PeersNotObtained);
    };

    // Randomly sort peers before we return them to avoid overly hitting any one peer
    let mut rng = thread_rng();
    peers.shuffle(&mut rng);

    Ok(peers)
}

// should not be reachable, but needed for the compiler to be happy.
#[allow(clippy::unused_async)]
#[cfg(not(feature = "network-contacts"))]
async fn get_network_contacts(_args: &PeersArgs) -> Result<Vec<Multiaddr>> {
    Ok(vec![])
}

#[cfg(feature = "network-contacts")]
async fn get_network_contacts(args: &PeersArgs) -> Result<Vec<Multiaddr>> {
    let url = args
        .network_contacts_url
        .clone()
        .unwrap_or(Url::parse(NETWORK_CONTACTS_URL.as_str())?);

    info!("Trying to fetch the bootstrap peers from {url}");
    println!("Trying to fetch the bootstrap peers from {url}");

    get_bootstrap_peers_from_url(url).await
}

/// Parse strings like `1.2.3.4:1234` and `/ip4/1.2.3.4/tcp/1234` into a multiaddr.
pub fn parse_peer_addr(addr: &str) -> Result<Multiaddr> {
    // Parse valid IPv4 socket address, e.g. `1.2.3.4:1234`.
    if let Ok(addr) = addr.parse::<std::net::SocketAddrV4>() {
        let start_addr = Multiaddr::from(*addr.ip());
        // Start with an address into a `/ip4/<ip>/udp/<port>/quic-v1` multiaddr.
        let multiaddr = start_addr
            .with(Protocol::Udp(addr.port()))
            .with(Protocol::QuicV1);

        #[cfg(all(feature = "websockets", feature = "wasm32"))]
        // Turn the address into a `/ip4/<ip>/udp/<port>/websocket-websys-v1` multiaddr.
        let multiaddr = start_addr
            .with(Protocol::Tcp(addr.port()))
            .with(Protocol::Ws("/".into()));

        return Ok(multiaddr);
    }

    // Parse any valid multiaddr string
    if let Ok(addr) = addr.parse::<Multiaddr>() {
        debug!("Parsing a full multiaddr: {:?}", addr);
        return Ok(addr);
    }

    Err(Error::InvalidPeerAddr)
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
                        return Err(Error::NoMultiAddrObtainedFromNetworkContacts(
                            url.to_string(),
                        ));
                    }
                } else {
                    retries += 1;
                    if retries >= MAX_NETWORK_CONTACTS_GET_RETRIES {
                        return Err(Error::NetworkContactsUnretrievable(
                            url.to_string(),
                            MAX_NETWORK_CONTACTS_GET_RETRIES,
                        ));
                    }
                }
            }
            Err(_) => {
                retries += 1;
                if retries >= MAX_NETWORK_CONTACTS_GET_RETRIES {
                    return Err(Error::NetworkContactsUnretrievable(
                        url.to_string(),
                        MAX_NETWORK_CONTACTS_GET_RETRIES,
                    ));
                }
            }
        }
        trace!("Failed to get bootstrap peers from URL, retrying {retries}/{MAX_NETWORK_CONTACTS_GET_RETRIES}");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
