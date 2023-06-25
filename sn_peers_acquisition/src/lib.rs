// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Args;
use eyre::{eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};

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
    #[clap(long = "peer", value_name = "multiaddr", env = SAFE_PEERS_ENV, value_delimiter = ',', value_parser = parse_peer_addr)]
    pub peers: Vec<(PeerId, Multiaddr)>,
}

/// Split a `Multiaddr` into the `PeerId` and the rest of the `Multiaddr`.
pub fn parse_peer_addr(addr: &str) -> Result<(PeerId, Multiaddr)> {
    let mut multiaddr = addr
        .parse::<Multiaddr>()
        .map_err(|err| eyre!("address is not a valid multiaddr: {err}"))?;

    let protocol = multiaddr.pop().ok_or_else(|| eyre!("address is empty"))?;

    let peer_id = match protocol {
        Protocol::P2p(hash) => hash,
        _ => return Err(eyre!("address does not end on `/p2p/<PeerId>`")),
    };

    Ok((peer_id, multiaddr))
}
