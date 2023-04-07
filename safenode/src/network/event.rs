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
    NetworkSwarmLoop,
};

use crate::protocol::messages::{Request, Response};
use libp2p::{
    kad::{store::MemoryStore, Kademlia, KademliaEvent, QueryResult},
    mdns,
    multiaddr::Protocol,
    request_response::{self, ResponseChannel},
    swarm::{NetworkBehaviour, SwarmEvent},
};
use tracing::{info, warn};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NodeEvent")]
pub(super) struct NodeBehaviour {
    pub(super) request_response: request_response::Behaviour<MsgCodec>,
    pub(super) kademlia: Kademlia<MemoryStore>,
    pub(super) mdns: mdns::tokio::Behaviour,
}

#[derive(Debug)]
pub(super) enum NodeEvent {
    RequestResponse(request_response::Event<Request, Response>),
    Kademlia(KademliaEvent),
    Mdns(Box<mdns::Event>),
}

impl From<request_response::Event<Request, Response>> for NodeEvent {
    fn from(event: request_response::Event<Request, Response>) -> Self {
        NodeEvent::RequestResponse(event)
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

#[derive(Debug)]
/// Events forwarded by the underlying Network; to be used by the upper layers
pub enum NetworkEvent {
    /// Incoming `Request` from a peer
    RequestReceived {
        /// Request
        req: Request,
        /// The channel to send the `Response` through
        channel: ResponseChannel<Response>,
    },
    /// Emitted when the DHT is updated
    PeerAdded,
}

impl NetworkSwarmLoop {
    // Handle `SwarmEvents`
    pub(super) async fn handle_swarm_events<EventError: std::error::Error>(
        &mut self,
        event: SwarmEvent<NodeEvent, EventError>,
    ) -> Result<()> {
        match event {
            // handle RequestResponse events
            SwarmEvent::Behaviour(NodeEvent::RequestResponse(event)) => {
                if let Err(e) = self.handle_msg(event).await {
                    warn!("RequestResponseError: {e:?}");
                }
            }
            // handle Kademlia events
            SwarmEvent::Behaviour(NodeEvent::Kademlia(event)) => match event {
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result: QueryResult::GetClosestPeers(Ok(closest_peers)),
                    ..
                } => {
                    if let Some(sender) = self.pending_get_closest_peers.remove(&id) {
                        sender
                            .send(closest_peers.peers.into_iter().collect())
                            .map_err(|_| Error::Other("Receiver not to be dropped".to_string()))?;
                    }
                }
                KademliaEvent::RoutingUpdated { .. } => {
                    self.event_sender.send(NetworkEvent::PeerAdded).await?;
                }
                KademliaEvent::InboundRequest { request } => {
                    info!("got inbound request: {request:?}");
                }
                todo => {
                    error!("KademliaEvent has not been implemented: {todo:?}");
                }
            },
            SwarmEvent::Behaviour(NodeEvent::Mdns(mdns_event)) => match *mdns_event {
                mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        info!("Node discovered: {multiaddr:?}");
                        let _routing_update = self
                            .swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, multiaddr);
                    }
                }
                mdns::Event::Expired(_) => {
                    info!("mdns peer expired");
                }
            },
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                info!(
                    "Local node is listening on {:?}",
                    address.with(Protocol::P2p(local_peer_id.into()))
                );
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
            SwarmEvent::ConnectionClosed { .. } => {}
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
