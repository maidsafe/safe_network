// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    safe_msg::SafeMsgCodec,
    EventLoop, SafeRequest, SafeResponse,
};
use futures::{channel::oneshot, SinkExt};
use libp2p::{
    kad::{store::MemoryStore, GetProvidersOk, Kademlia, KademliaEvent, QueryResult},
    mdns,
    multiaddr::Protocol,
    request_response::{self, ResponseChannel},
    swarm::{NetworkBehaviour, SwarmEvent},
};
use tracing::{info, warn};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "SafeNodeEvent")]
pub(super) struct SafeNodeBehaviour {
    pub(super) request_response: request_response::Behaviour<SafeMsgCodec>,
    pub(super) kademlia: Kademlia<MemoryStore>,
    pub(super) mdns: mdns::async_io::Behaviour,
}

#[derive(Debug)]
pub(super) enum SafeNodeEvent {
    RequestResponse(request_response::Event<SafeRequest, SafeResponse>),
    Kademlia(KademliaEvent),
    Mdns(Box<mdns::Event>),
}

impl From<request_response::Event<SafeRequest, SafeResponse>> for SafeNodeEvent {
    fn from(event: request_response::Event<SafeRequest, SafeResponse>) -> Self {
        SafeNodeEvent::RequestResponse(event)
    }
}

impl From<KademliaEvent> for SafeNodeEvent {
    fn from(event: KademliaEvent) -> Self {
        SafeNodeEvent::Kademlia(event)
    }
}

impl From<mdns::Event> for SafeNodeEvent {
    fn from(event: mdns::Event) -> Self {
        SafeNodeEvent::Mdns(Box::new(event))
    }
}

#[derive(Debug)]
pub enum NetworkEvent {
    InboundSafeRequest {
        req: SafeRequest,
        channel: ResponseChannel<SafeResponse>,
    },
    // might/might not be successfully added to the DHT; `RoutingUpdate` is private/no debug impl
    PeerDiscoverd,
}

impl EventLoop {
    pub(super) async fn handle_event<EventError: std::error::Error>(
        &mut self,
        event: SwarmEvent<SafeNodeEvent, EventError>,
    ) -> Result<()> {
        match event {
            // handle RequestResponse events
            SwarmEvent::Behaviour(SafeNodeEvent::RequestResponse(event)) => {
                if let Err(e) = self.handle_safe_msg(event).await {
                    warn!("RequestResponseError: {e:?}");
                }
            }
            // handle Kademlia events
            SwarmEvent::Behaviour(SafeNodeEvent::Kademlia(event)) => match event {
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result: QueryResult::StartProviding(_),
                    ..
                } => {
                    let sender: oneshot::Sender<Result<()>> = self
                        .pending_start_providing
                        .remove(&id)
                        .ok_or(Error::Other(
                            "Completed query to be previously pending.".to_string(),
                        ))?;
                    let _ = sender.send(Ok(()));
                }
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result:
                        QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders {
                            providers, ..
                        })),
                    ..
                } => {
                    if let Some(sender) = self.pending_get_providers.remove(&id) {
                        sender
                            .send(providers)
                            .map_err(|_| Error::Other("Receiver not to be dropped".to_string()))?;

                        // Finish the query. We are only interested in the first result.
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .query_mut(&id)
                            .ok_or(Error::Other("Query should be exist".to_string()))?
                            .finish();
                    }
                }
                _ => {}
            },
            SwarmEvent::Behaviour(SafeNodeEvent::Mdns(mdns_event)) => match *mdns_event {
                mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        info!("Node discovered: {multiaddr:?}");
                        let _routing_update = self
                            .swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, multiaddr);
                    }
                    self.event_sender.send(NetworkEvent::PeerDiscoverd).await?;
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
            e => panic!("{e:?}"),
        }
        Ok(())
    }
}
