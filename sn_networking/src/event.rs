// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    // msg::MsgCodec,
    SwarmDriver,
};
use crate::{multiaddr_is_global, multiaddr_strip_p2p, CLOSE_GROUP_SIZE, IDENTIFY_AGENT_STR};
use itertools::Itertools;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    autonat::{self, NatStatus},
    kad::{GetRecordOk, InboundRequest, Kademlia, KademliaEvent, QueryResult, K_VALUE},
    multiaddr::Protocol,
    request_response::{self, ResponseChannel as PeerResponseChannel},
    swarm::{behaviour::toggle::Toggle, DialError, NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId,
};
use sn_protocol::{
    messages::{Request, Response},
    NetworkAddress,
};
use sn_record_store::DiskBackedRecordStore;
#[cfg(feature = "local-discovery")]
use std::collections::hash_map;
use std::collections::HashSet;
use tokio::sync::oneshot;
use tracing::{info, warn};

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NodeEvent")]
pub(super) struct NodeBehaviour {
    pub(super) request_response: request_response::cbor::Behaviour<Request, Response>,
    pub(super) kademlia: Kademlia<DiskBackedRecordStore>,
    #[cfg(feature = "local-discovery")]
    pub(super) mdns: mdns::tokio::Behaviour,
    pub(super) identify: libp2p::identify::Behaviour,
    pub(super) autonat: Toggle<autonat::Behaviour>,
}

#[derive(Debug)]
pub(super) enum NodeEvent {
    MsgReceived(request_response::Event<Request, Response>),
    Kademlia(KademliaEvent),
    #[cfg(feature = "local-discovery")]
    Mdns(Box<mdns::Event>),
    Identify(Box<libp2p::identify::Event>),
    Autonat(autonat::Event),
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

impl From<autonat::Event> for NodeEvent {
    fn from(event: autonat::Event) -> Self {
        NodeEvent::Autonat(event)
    }
}

#[derive(Debug)]
/// Channel to send the `Response` through.
pub enum MsgResponder {
    /// Respond to a request from `self` through a simple one-shot channel.
    FromSelf(Option<oneshot::Sender<Result<Response>>>),
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
    /// Handles the responses that are not awaited at the call site
    ResponseReceived {
        /// Response
        res: Response,
    },
    /// Peer has been added to the Routing Table
    PeerAdded(PeerId),
    // Peer has been removed from the Routing Table
    PeerRemoved(PeerId),
    /// Started listening on a new address
    NewListenAddr(Multiaddr),
    /// AutoNAT status changed
    NatStatusChanged(NatStatus),
    /// Report peers that lost a record
    LostRecordDetected(Vec<PeerId>),
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
            SwarmEvent::Behaviour(NodeEvent::Kademlia(kad_event)) => {
                self.handle_kad_event(kad_event).await?;
            }
            SwarmEvent::Behaviour(NodeEvent::Identify(iden)) => {
                match *iden {
                    libp2p::identify::Event::Received { peer_id, info } => {
                        debug!(%peer_id, ?info, "identify: received info");

                        // If we are not local, we care only for peers that we dialed and thus are reachable.
                        if (self.local || self.dialed_peers.contains(&peer_id))
                            && info.agent_version.starts_with(IDENTIFY_AGENT_STR)
                        {
                            let addrs = match self.local {
                                true => info.listen_addrs,
                                // If we're not in local mode, only add globally reachable addresses
                                false => info
                                    .listen_addrs
                                    .into_iter()
                                    .filter(multiaddr_is_global)
                                    .collect(),
                            };
                            // Strip the `/p2p/...` part of the multiaddresses
                            let addrs: Vec<_> = addrs
                                .into_iter()
                                .map(|addr| multiaddr_strip_p2p(&addr))
                                // And deduplicate the list
                                .unique()
                                .collect();

                            debug!(%peer_id, ?addrs, "identify: adding addresses to routing table");
                            for multiaddr in addrs.clone() {
                                let _routing_update = self
                                    .swarm
                                    .behaviour_mut()
                                    .kademlia
                                    .add_address(&peer_id, multiaddr);
                            }

                            // If the peer supports AutoNAT, add it as server
                            if info.protocols.iter().any(|protocol| {
                                protocol.to_string().starts_with("/libp2p/autonat/")
                            }) {
                                let a = &mut self.swarm.behaviour_mut().autonat;
                                // It could be that we are on a local network and have AutoNAT disabled.
                                if let Some(autonat) = a.as_mut() {
                                    for multiaddr in addrs {
                                        autonat.add_server(peer_id, Some(multiaddr));
                                    }
                                }
                            }
                        }
                    }
                    libp2p::identify::Event::Sent { .. } => trace!("identify: {iden:?}"),
                    libp2p::identify::Event::Pushed { .. } => trace!("identify: {iden:?}"),
                    libp2p::identify::Event::Error { .. } => trace!("identify: {iden:?}"),
                }
            }
            #[cfg(feature = "local-discovery")]
            SwarmEvent::Behaviour(NodeEvent::Mdns(mdns_event)) => match *mdns_event {
                mdns::Event::Discovered(list) => {
                    if self.local {
                        for (peer_id, multiaddr) in list {
                            info!("Node discovered and dialing: {multiaddr:?}");

                            let mut dial_failed = None;
                            // TODO: Deduplicate this functionality by calling in on SwarmCmd::Dial
                            if let hash_map::Entry::Vacant(dial_entry) =
                                self.pending_dial.entry(peer_id)
                            {
                                let (sender, _receiver) = oneshot::channel();
                                let _ = dial_entry.insert(sender);
                                // TODO: Dropping the receiver immediately might get logged as error later.
                                if let Err(error) =
                                    self.swarm.dial(multiaddr.with(Protocol::P2p(peer_id)))
                                {
                                    dial_failed = Some(error);
                                }
                            }

                            // if we error'd out, send the error back
                            if let Some(error) = dial_failed {
                                if let Some(sender) = self.pending_dial.remove(&peer_id) {
                                    let _ = sender.send(Err(error.into()));
                                }
                            }
                        }
                    }
                }
                mdns::Event::Expired(peer) => {
                    debug!("mdns peer {peer:?} expired");
                }
            },
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                let address = address.with(Protocol::P2p(local_peer_id));
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
                    debug!("Connected with {peer_id:?}");

                    self.dialed_peers.push(peer_id);

                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                cause,
                num_established,
                connection_id,
            } => {
                debug!("Connection closed to Peer {peer_id}({num_established:?} / {connection_id:?}) - {endpoint:?} - {cause:?}");
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                trace!("OutgoingConnectionError to {peer_id:?} - {error:?}");
                if let Some(peer_id) = peer_id {
                    // Related errors are: WrongPeerId, ConnectionRefused(TCP), HandshakeTimedOut(QUIC)
                    let err_string = format!("{error:?}");
                    let is_wrong_id = err_string.contains("WrongPeerId");
                    let is_all_connection_failed = if let DialError::Transport(ref errors) = error {
                        errors.iter().all(|(_, error)| {
                            let err_string = format!("{error:?}");
                            err_string.contains("ConnectionRefused")
                        }) || errors.iter().all(|(_, error)| {
                            let err_string = format!("{error:?}");
                            err_string.contains("HandshakeTimedOut")
                        })
                    } else {
                        false
                    };
                    if is_wrong_id || is_all_connection_failed {
                        info!("Detected dead peer {peer_id:?}");
                        let _ = self
                            .event_sender
                            .send(NetworkEvent::PeerRemoved(peer_id))
                            .await;
                        let _ = self.swarm.behaviour_mut().kademlia.remove_peer(&peer_id);
                        self.log_kbuckets(&peer_id);
                    }

                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(error.into()));
                    } else {
                        debug!("OutgoingConnectionError is due to non pending_dial to {peer_id}");
                    }
                }
            }
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::Dialing {
                peer_id,
                connection_id,
            } => trace!("Dialing {peer_id:?} on {connection_id:?}"),

            SwarmEvent::Behaviour(NodeEvent::Autonat(event)) => match event {
                autonat::Event::InboundProbe(e) => trace!("AutoNAT inbound probe: {e:?}"),
                autonat::Event::OutboundProbe(e) => trace!("AutoNAT outbound probe: {e:?}"),
                autonat::Event::StatusChanged { old, new } => {
                    info!("AutoNAT status changed: {old:?} -> {new:?}");
                    self.event_sender
                        .send(NetworkEvent::NatStatusChanged(new.clone()))
                        .await?;

                    match new {
                        NatStatus::Public(_addr) => {
                            // In theory, we could actively push our address to our peers now. But, which peers? All of them?
                            // Or, should we just wait and let Identify do it on its own? But, what if we are not connected
                            // to any peers anymore? (E.g., our connections timed out etc)
                            // let all_peers: Vec<_> = self.swarm.connected_peers().cloned().collect();
                            // self.swarm.behaviour_mut().identify.push(all_peers);
                        }
                        NatStatus::Private => {
                            // We could just straight out error here. In the future we might try to activate a relay mechanism.
                        }
                        NatStatus::Unknown => {}
                    };
                }
            },
            other => debug!("SwarmEvent has been ignored: {other:?}"),
        }
        Ok(())
    }

    async fn handle_kad_event(&mut self, kad_event: KademliaEvent) -> Result<()> {
        match kad_event {
            ref event @ KademliaEvent::OutboundQueryProgressed {
                id,
                result: QueryResult::GetClosestPeers(Ok(ref closest_peers)),
                ref stats,
                ref step,
            } => {
                trace!(
                    "Query task {id:?} returned with peers {closest_peers:?}, {stats:?} - {step:?}"
                );

                let (sender, mut current_closest) =
                    self.pending_get_closest_peers.remove(&id).ok_or_else(|| {
                        trace!("Can't locate query task {id:?}, shall be completed already.");
                        Error::ReceivedKademliaEventDropped(event.clone())
                    })?;

                // TODO: consider order the result and terminate when reach any of the
                //       following criteria:
                //   1, `stats.num_pending()` is 0
                //   2, `stats.duration()` is longer than a defined period
                let new_peers: HashSet<PeerId> = closest_peers.peers.clone().into_iter().collect();
                current_closest.extend(new_peers);
                if current_closest.len() >= usize::from(K_VALUE) || step.last {
                    sender
                        .send(current_closest)
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                } else {
                    let _ = self
                        .pending_get_closest_peers
                        .insert(id, (sender, current_closest));
                }
            }
            KademliaEvent::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Ok(GetRecordOk::FoundRecord(peer_record))),
                stats,
                step,
            } => {
                trace!(
                    "Query task {id:?} returned with record {:?} from peer {:?}, {stats:?} - {step:?}",
                    peer_record.record.key,
                    peer_record.peer
                );
                if let Some(sender) = self.pending_query.remove(&id) {
                    sender
                        .send(Ok(peer_record.record))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                }
            }
            KademliaEvent::OutboundQueryProgressed {
                result:
                    QueryResult::GetRecord(Ok(GetRecordOk::FinishedWithNoAdditionalRecord {
                        cache_candidates,
                    })),
                ..
            } => {
                // The candidates are nodes supposed to hold a copy however failed to do so.
                // In that case, we can try to trigger a replication to it.
                let peer_ids: Vec<PeerId> = cache_candidates.values().copied().collect();
                trace!("Candidates {peer_ids:?} failed to respond a record query request.");
                self.event_sender
                    .send(NetworkEvent::LostRecordDetected(peer_ids))
                    .await?;
            }
            KademliaEvent::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Err(err)),
                stats,
                step,
            } => {
                warn!("Query task {id:?} failed to get record with error: {err:?}, {stats:?} - {step:?}");
                if step.last {
                    // To avoid the caller wait forever on a non-existing entry
                    if let Some(sender) = self.pending_query.remove(&id) {
                        sender
                            .send(Err(Error::RecordNotFound))
                            .map_err(|_| Error::InternalMsgChannelDropped)?;
                    }
                }
                // TODO: send an error response back?
            }
            KademliaEvent::RoutingUpdated {
                peer, is_new_peer, ..
            } => {
                if is_new_peer {
                    self.log_kbuckets(&peer);
                    self.event_sender
                        .send(NetworkEvent::PeerAdded(peer))
                        .await?;
                }
            }
            KademliaEvent::InboundRequest {
                request: InboundRequest::PutRecord { source, record, .. },
            } => {
                if record.is_some() {
                    // Currently we do not perform `kad.put_record()` or use `kad's replication` in our codebase,
                    // hence we should not receive any inbound PutRecord.
                    warn!("Kad's PutRecord handling is not implemented yet. {source:?} has triggerd kad.put_record or has enabled kad's replication flow");
                } else {
                    // If the Record filtering is not enabled at the kad cfg, a malicious node
                    // can just call `kad.put_record()` which would store that record at the
                    // closest nodes without any validations
                    //
                    // Enable it to instead get the above `PutRequest` event which is then
                    // handled separately
                    warn!("The PutRecord KademliaEvent should include a Record. Enable record filtering via the kad config")
                }
            }
            KademliaEvent::InboundRequest {
                request:
                    InboundRequest::GetRecord {
                        num_closer_peers,
                        present_locally,
                    },
            } => {
                if !present_locally && num_closer_peers < CLOSE_GROUP_SIZE {
                    trace!("InboundRequest::GetRecord doesn't have local record, with {num_closer_peers:?} closer_peers");
                }
            }
            other => {
                trace!("KademliaEvent ignored: {other:?}");
            }
        }

        Ok(())
    }

    fn log_kbuckets(&mut self, peer: &PeerId) {
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
