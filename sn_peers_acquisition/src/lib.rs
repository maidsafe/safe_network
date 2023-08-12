// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Args;
use eyre::{eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr};

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
    /// Note: `SAFE_PEERS` env_var only got picked up
    ///       when `--peers` argument is undefined via the safe/safenode executable.
    ///       OR, during the restart of a node, `SAFE_PEERS` contains new peers not presented
    ///       within the original initial_peers passed via `--peers` argument
    ///       BUT, the value of `SAFE_PEERS` env_var is at the time when safenode started,
    ///       i.e. if `SAFE_PEERS` env_var got updated after a safenode got started,
    ///            it will still be the old value got picked up during node restarting.
    #[clap(long = "peer", value_name = "multiaddr", env = SAFE_PEERS_ENV, value_delimiter = ',', value_parser = parse_peer_addr)]
    pub peers: Vec<Multiaddr>,
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
