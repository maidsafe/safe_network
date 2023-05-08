// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    msg::MsgCodec,
    SwarmDriver,
};

use crate::network::IDENTIFY_AGENT_STR;
use crate::protocol::{
    messages::{QueryResponse, Request, Response},
    storage::Chunk,
};

use crate::domain::storage::DiskBackedRecordStore;

use libp2p::{
    kad::{GetRecordOk, Kademlia, KademliaEvent, QueryResult, K_VALUE},
    mdns,
    multiaddr::Protocol,
    request_response::{self, ResponseChannel as PeerResponseChannel},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId,
};
use std::collections::{hash_map, HashSet};
use tokio::sync::oneshot;
use tracing::{info, warn};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NodeEvent")]
pub(super) struct NodeBehaviour {
    pub(super) request_response: request_response::Behaviour<MsgCodec>,
    pub(super) kademlia: Kademlia<DiskBackedRecordStore>,
    pub(super) mdns: mdns::tokio::Behaviour,
    pub(super) identify: libp2p::identify::Behaviour,
}

#[derive(Debug)]
pub(super) enum NodeEvent {
    MsgReceived(request_response::Event<Request, Response>),
    Kademlia(KademliaEvent),
    Mdns(Box<mdns::Event>),
    Identify(Box<libp2p::identify::Event>),
}

impl From<request_response::Event<Request, Response>> for NodeEvent {
    fn from(event: request_response::Event<Request, Response>) -> Self {
        NodeEvent::MsgReceived(event)
    }
}

impl From<KademliaEvent> for NodeEvent {
    fn from(event: KademliaEvent) -> Self {
        NodeEvent::Kademlia(event)
    }
}

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

#[derive(Debug)]
/// Channel to send the `Response` through.
pub enum MsgResponder {
    /// Respond to a request from `self` through a simple one-shot channel.
    FromSelf(oneshot::Sender<Result<Response>>),
    /// Respond to a request from a peer in the network.
    FromPeer(PeerResponseChannel<Response>),
}

#[derive(Debug)]
/// Events forwarded by the underlying Network; to be used by the upper layers
pub enum NetworkEvent {
    /// Incoming `Request` from a peer
    RequestReceived {
        /// Request
        req: Request,
        /// The channel to send the `Response` through
        channel: MsgResponder,
    },
    /// Emitted when the DHT is updated
    PeerAdded(PeerId),
    /// Started listening on a new address
    NewListenAddr(Multiaddr),
}

impl SwarmDriver {
    // Handle `SwarmEvents`
    pub(super) async fn handle_swarm_events<EventError: std::error::Error>(
        &mut self,
        event: SwarmEvent<NodeEvent, EventError>,
    ) -> Result<()> {
        let span = info_span!("Handling a swarm event");
        let _ = span.enter();
        match event {
            SwarmEvent::Behaviour(NodeEvent::MsgReceived(event)) => {
                if let Err(e) = self.handle_msg(event).await {
                    warn!("MsgReceivedError: {e:?}");
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Kademlia(ref event)) => match event {
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result: QueryResult::GetClosestPeers(Ok(closest_peers)),
                    stats,
                    step,
                } => {
                    trace!("Query task {id:?} returned with peers {closest_peers:?}, {stats:?} - {step:?}");

                    let (sender, mut current_closest) =
                        self.pending_get_closest_peers.remove(id).ok_or_else(|| {
                            trace!("Can't locate query task {id:?}, shall be completed already.");
                            Error::ReceivedKademliaEventDropped(event.clone())
                        })?;

                    // TODO: consider order the result and terminate when reach any of the
                    //       following creterias:
                    //   1, `stats.num_pending()` is 0
                    //   2, `stats.duration()` is longer than a defined period
                    let new_peers: HashSet<PeerId> =
                        closest_peers.peers.clone().into_iter().collect();
                    current_closest.extend(new_peers);
                    if current_closest.len() >= usize::from(K_VALUE) || step.last {
                        sender
                            .send(current_closest)
                            .map_err(|_| Error::InternalMsgChannelDropped)?;
                    } else {
                        let _ = self
                            .pending_get_closest_peers
                            .insert(*id, (sender, current_closest));
                    }
                }
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result: QueryResult::GetRecord(result),
                    stats,
                    step,
                } => {
                    trace!("Record query task {id:?} returned with result, {stats:?} - {step:?}");
                    if let Ok(GetRecordOk::FoundRecord(peer_record)) = result {
                        trace!(
                            "Query {id:?} returned with record {:?} from peer {:?}",
                            peer_record.record.key,
                            peer_record.peer
                        );
                        if let Some(sender) = self.pending_query.remove(id) {
                            sender
                                .send(Ok(QueryResponse::GetChunk(Ok(Chunk::new(
                                    peer_record.record.value.clone().into(),
                                )))))
                                .map_err(|_| Error::InternalMsgChannelDropped)?;
                        }
                    } else {
                        warn!("Query {id:?} failed to get record with result {result:?}");
                        if step.last {
                            // To avoid the caller wait forever on a non-existring entry
                            if let Some(sender) = self.pending_query.remove(id) {
                                sender
                                    .send(Err(Error::RecordNotFound))
                                    .map_err(|_| Error::InternalMsgChannelDropped)?;
                            }
                        }
                        // TODO: send an error response back?
                    }
                }
                KademliaEvent::RoutingUpdated {
                    peer, is_new_peer, ..
                } => {
                    if *is_new_peer {
                        self.event_sender
                            .send(NetworkEvent::PeerAdded(*peer))
                            .await?;
                    }
                }
                KademliaEvent::InboundRequest { request } => {
                    info!("got inbound request: {request:?}");
                }
                todo => {
                    error!("KademliaEvent has not been implemented: {todo:?}");
                }
            },
            SwarmEvent::Behaviour(NodeEvent::Identify(iden)) => {
                info!("IdentifyEvent: {iden:?}");
                match *iden {
                    libp2p::identify::Event::Received { peer_id, info } => {
                        info!("Adding peer to routing table, based on received identify info from {peer_id:?}: {info:?}");
                        if info.agent_version.starts_with(IDENTIFY_AGENT_STR) {
                            for multiaddr in info.listen_addrs {
                                let _routing_update = self
                                    .swarm
                                    .behaviour_mut()
                                    .kademlia
                                    .add_address(&peer_id, multiaddr);
                            }
                        }
                    }
                    libp2p::identify::Event::Sent { .. } => {}
                    libp2p::identify::Event::Pushed { .. } => {}
                    libp2p::identify::Event::Error { .. } => {}
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Mdns(mdns_event)) => match *mdns_event {
                mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        info!("Node discovered and dialing!!!: {multiaddr:?}");
                        // TODO: Deduplicate this functionality by calling in on SwarmCmd::Dial
                        if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
                            // TODO: Dropping the receiver immediately might get logged as error later.
                            let (sender, _receiver) = oneshot::channel();
                            match self
                                .swarm
                                .dial(multiaddr.with(Protocol::P2p(peer_id.into())))
                            {
                                Ok(()) => {
                                    let _ = e.insert(sender);
                                }
                                Err(e) => {
                                    let _ = sender.send(Err(e.into()));
                                }
                            }
                        }
                    }
                }
                mdns::Event::Expired(peer) => {
                    info!("mdns peer {peer:?} expired");
                }
            },
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                let address = address.with(Protocol::P2p(local_peer_id.into()));
                self.event_sender
                    .send(NetworkEvent::NewListenAddr(address.clone()))
                    .await?;
                info!("Local node is listening on {address:?}");
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                if endpoint.is_dialer() {
                    info!("Connected with {peer_id:?}");
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                cause,
                ..
            } => {
                info!("Connection closed to Peer {peer_id} - {endpoint:?} - {cause:?}");
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(error.into()));
                    }
                }
            }
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::Dialing(peer_id) => info!("Dialing {peer_id}"),
            todo => error!("SwarmEvent has not been implemented: {todo:?}"),
        }
        Ok(())
    }
}
