// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use eyre::{eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr};

/// The name of the environment variable that can be used to pass peers to the node.
pub const SAFE_PEERS_ENV: &str = "SAFE_PEERS";

/// Parse strings like `1.2.3.4:1234` and `/ip4/1.2.3.4/tcp/1234` into a (TCP) multiaddr.
pub fn parse_peer_addr(addr: &str) -> Result<Multiaddr> {
    // Parse valid IPv4 socket address, e.g. `1.2.3.4:1234`.
    if let Ok(addr) = addr.parse::<std::net::SocketAddrV4>() {
        // Turn the address into a `/ip4/<ip>/tcp/<port>` multiaddr.
        let multiaddr = Multiaddr::from(*addr.ip()).with(Protocol::Tcp(addr.port()));
        return Ok(multiaddr);
    }

    // Parse any valid multiaddr string, e.g. `/ip4/1.2.3.4/tcp/1234/p2p/<peer_id>`.
    if let Ok(addr) = addr.parse::<Multiaddr>() {
        return Ok(addr);
    }

    Err(eyre!(
        "peer address `{addr}` is not a valid multiaddr or socket address"
    ))
}
