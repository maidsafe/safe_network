// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod kad;
mod request_response;
mod swarm;

use crate::{driver::SwarmDriver, error::Result, CLOSE_GROUP_SIZE};
use core::fmt;
use custom_debug::Debug as CustomDebug;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    kad::{Record, RecordKey},
    request_response::ResponseChannel as PeerResponseChannel,
    Multiaddr, PeerId,
};

use sn_protocol::{
    messages::{Query, Request, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::PaymentQuote;
use std::{
    collections::{BTreeSet, HashSet},
    fmt::{Debug, Formatter},
};
use tokio::sync::oneshot;

/// NodeEvent enum
#[derive(CustomDebug)]
pub(super) enum NodeEvent {
    MsgReceived(libp2p::request_response::Event<Request, Response>),
    Kademlia(libp2p::kad::Event),
    #[cfg(feature = "local-discovery")]
    Mdns(Box<mdns::Event>),
    Identify(Box<libp2p::identify::Event>),
    Dcutr(Box<libp2p::dcutr::Event>),
    RelayClient(Box<libp2p::relay::client::Event>),
    RelayServer(Box<libp2p::relay::Event>),
}

impl From<libp2p::request_response::Event<Request, Response>> for NodeEvent {
    fn from(event: libp2p::request_response::Event<Request, Response>) -> Self {
        NodeEvent::MsgReceived(event)
    }
}

impl From<libp2p::kad::Event> for NodeEvent {
    fn from(event: libp2p::kad::Event) -> Self {
        NodeEvent::Kademlia(event)
    }
}

#[cfg(feature = "local-discovery")]
impl From<mdns::Event> for NodeEvent {
    fn from(event: mdns::Event) -> Self {
        NodeEvent::Mdns(Box::new(event))
    }
}

impl From<libp2p::identify::Event> for NodeEvent {
    fn from(event: libp2p::identify::Event) -> Self {
        NodeEvent::Identify(Box::new(event))
    }
}
impl From<libp2p::dcutr::Event> for NodeEvent {
    fn from(event: libp2p::dcutr::Event) -> Self {
        NodeEvent::Dcutr(Box::new(event))
    }
}
impl From<libp2p::relay::client::Event> for NodeEvent {
    fn from(event: libp2p::relay::client::Event) -> Self {
        NodeEvent::RelayClient(Box::new(event))
    }
}
impl From<libp2p::relay::Event> for NodeEvent {
    fn from(event: libp2p::relay::Event) -> Self {
        NodeEvent::RelayServer(Box::new(event))
    }
}

#[derive(CustomDebug)]
/// Channel to send the `Response` through.
pub enum MsgResponder {
    /// Respond to a request from `self` through a simple one-shot channel.
    FromSelf(Option<oneshot::Sender<Result<Response>>>),
    /// Respond to a request from a peer in the network.
    FromPeer(PeerResponseChannel<Response>),
}

#[allow(clippy::large_enum_variant)]
/// Events forwarded by the underlying Network; to be used by the upper layers
pub enum NetworkEvent {
    /// Incoming `Query` from a peer
    QueryRequestReceived {
        /// Query
        query: Query,
        /// The channel to send the `Response` through
        channel: MsgResponder,
    },
    /// Handles the responses that are not awaited at the call site
    ResponseReceived {
        /// Response
        res: Response,
    },
    /// Peer has been added to the Routing Table. And the number of connected peers.
    PeerAdded(PeerId, usize),
    /// Peer has been removed from the Routing Table. And the number of connected peers.
    PeerRemoved(PeerId, usize),
    /// The peer does not support our protocol
    PeerWithUnsupportedProtocol {
        our_protocol: String,
        their_protocol: String,
    },
    /// The peer is now considered as a bad node, due to the detected bad behaviour
    PeerConsideredAsBad {
        detected_by: PeerId,
        bad_peer: PeerId,
        bad_behaviour: String,
    },
    /// The records bearing these keys are to be fetched from the holder or the network
    KeysToFetchForReplication(Vec<(PeerId, RecordKey)>),
    /// Started listening on a new address
    NewListenAddr(Multiaddr),
    /// Report unverified record
    UnverifiedRecord(Record),
    /// Terminate Node on unrecoverable errors
    TerminateNode { reason: TerminateNodeReason },
    /// List of peer nodes that failed to fetch replication copy from.
    FailedToFetchHolders(BTreeSet<PeerId>),
    /// A peer in RT that supposed to be verified.
    BadNodeVerification { peer_id: PeerId },
    /// Quotes to be verified
    QuoteVerification { quotes: Vec<(PeerId, PaymentQuote)> },
    /// Carry out chunk proof check against the specified record and peer
    ChunkProofVerification {
        peer_id: PeerId,
        keys_to_verify: Vec<NetworkAddress>,
    },
}

/// Terminate node for the following reason
#[derive(Debug, Clone)]
pub enum TerminateNodeReason {
    HardDiskWriteError,
}

// Manually implement Debug as `#[debug(with = "unverified_record_fmt")]` not working as expected.
impl Debug for NetworkEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NetworkEvent::QueryRequestReceived { query, .. } => {
                write!(f, "NetworkEvent::QueryRequestReceived({query:?})")
            }
            NetworkEvent::ResponseReceived { res, .. } => {
                write!(f, "NetworkEvent::ResponseReceived({res:?})")
            }
            NetworkEvent::PeerAdded(peer_id, connected_peers) => {
                write!(f, "NetworkEvent::PeerAdded({peer_id:?}, {connected_peers})")
            }
            NetworkEvent::PeerRemoved(peer_id, connected_peers) => {
                write!(
                    f,
                    "NetworkEvent::PeerRemoved({peer_id:?}, {connected_peers})"
                )
            }
            NetworkEvent::PeerWithUnsupportedProtocol {
                our_protocol,
                their_protocol,
            } => {
                write!(f, "NetworkEvent::PeerWithUnsupportedProtocol({our_protocol:?}, {their_protocol:?})")
            }
            NetworkEvent::PeerConsideredAsBad {
                bad_peer,
                bad_behaviour,
                ..
            } => {
                write!(
                    f,
                    "NetworkEvent::PeerConsideredAsBad({bad_peer:?}, {bad_behaviour:?})"
                )
            }
            NetworkEvent::KeysToFetchForReplication(list) => {
                let keys_len = list.len();
                write!(f, "NetworkEvent::KeysForReplication({keys_len:?})")
            }
            NetworkEvent::NewListenAddr(addr) => {
                write!(f, "NetworkEvent::NewListenAddr({addr:?})")
            }
            NetworkEvent::UnverifiedRecord(record) => {
                let pretty_key = PrettyPrintRecordKey::from(&record.key);
                write!(f, "NetworkEvent::UnverifiedRecord({pretty_key:?})")
            }
            NetworkEvent::TerminateNode { reason } => {
                write!(f, "NetworkEvent::TerminateNode({reason:?})")
            }
            NetworkEvent::FailedToFetchHolders(bad_nodes) => {
                write!(f, "NetworkEvent::FailedToFetchHolders({bad_nodes:?})")
            }
            NetworkEvent::BadNodeVerification { peer_id } => {
                write!(f, "NetworkEvent::BadNodeVerification({peer_id:?})")
            }
            NetworkEvent::QuoteVerification { quotes } => {
                write!(
                    f,
                    "NetworkEvent::QuoteVerification({} quotes)",
                    quotes.len()
                )
            }
            NetworkEvent::ChunkProofVerification {
                peer_id,
                keys_to_verify,
            } => {
                write!(
                    f,
                    "NetworkEvent::ChunkProofVerification({peer_id:?} {keys_to_verify:?})"
                )
            }
        }
    }
}

impl SwarmDriver {
    /// Check for changes in our close group
    pub(crate) fn check_for_change_in_our_close_group(&mut self) -> bool {
        // this includes self
        let closest_k_peers = self.get_closest_k_value_local_peers();

        let new_closest_peers: Vec<_> =
            closest_k_peers.into_iter().take(CLOSE_GROUP_SIZE).collect();

        let old = self.close_group.iter().cloned().collect::<HashSet<_>>();
        let new_members: Vec<_> = new_closest_peers
            .iter()
            .filter(|p| !old.contains(p))
            .collect();
        if !new_members.is_empty() {
            debug!("The close group has been updated. The new members are {new_members:?}");
            debug!("New close group: {new_closest_peers:?}");
            self.close_group = new_closest_peers;
            true
        } else {
            false
        }
    }

    pub(crate) fn log_kbuckets(&mut self, peer: &PeerId) {
        let distance = NetworkAddress::from_peer(self.self_peer_id)
            .distance(&NetworkAddress::from_peer(*peer));
        info!("Peer {peer:?} has a {:?} distance to us", distance.ilog2());
        let mut kbucket_table_stats = vec![];
        let mut index = 0;
        let mut total_peers = 0;
        for kbucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            let range = kbucket.range();
            total_peers += kbucket.num_entries();
            if let Some(distance) = range.0.ilog2() {
                kbucket_table_stats.push((index, kbucket.num_entries(), distance));
            } else {
                // This shall never happen.
                error!("bucket #{index:?} is ourself ???!!!");
            }
            index += 1;
        }
        info!("kBucketTable has {index:?} kbuckets {total_peers:?} peers, {kbucket_table_stats:?}");
    }
}
