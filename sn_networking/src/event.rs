// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    close_group_majority,
    driver::{truncate_patch_version, SwarmDriver},
    error::{Error, Result},
    multiaddr_is_global, multiaddr_strip_p2p, sort_peers_by_address, GetQuorum, CLOSE_GROUP_SIZE,
};
use bytes::Bytes;
use core::fmt;
use custom_debug::Debug as CustomDebug;
use itertools::Itertools;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
#[cfg(feature = "open-metrics")]
use libp2p::metrics::Recorder;
use libp2p::{
    autonat::{self, NatStatus},
    kad::{
        self, GetClosestPeersError, GetRecordError, GetRecordOk, InboundRequest, PeerRecord,
        QueryId, QueryResult, Record, RecordKey, K_VALUE,
    },
    multiaddr::Protocol,
    request_response::{self, Message, ResponseChannel as PeerResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId,
};

use sn_protocol::{
    messages::{Request, Response},
    storage::RecordHeader,
    NetworkAddress, PrettyPrintRecordKey,
};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    fmt::{Debug, Formatter},
    num::NonZeroUsize,
};
use tokio::sync::oneshot;
use tracing::{info, warn};
use xor_name::XorName;

/// Our agent string has as a prefix that we can match against.
const IDENTIFY_AGENT_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));

/// Using XorName to differentiate different record content under the same key.
pub(super) type GetRecordResultMap = HashMap<XorName, (Record, HashSet<PeerId>)>;

/// NodeEvent enum
#[derive(CustomDebug)]
pub(super) enum NodeEvent {
    MsgReceived(request_response::Event<Request, Response>),
    Kademlia(kad::Event),
    #[cfg(feature = "local-discovery")]
    Mdns(Box<mdns::Event>),
    Identify(Box<libp2p::identify::Event>),
    Autonat(autonat::Event),
    Gossipsub(libp2p::gossipsub::Event),
}

impl From<request_response::Event<Request, Response>> for NodeEvent {
    fn from(event: request_response::Event<Request, Response>) -> Self {
        NodeEvent::MsgReceived(event)
    }
}

impl From<kad::Event> for NodeEvent {
    fn from(event: kad::Event) -> Self {
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

impl From<libp2p::gossipsub::Event> for NodeEvent {
    fn from(event: libp2p::gossipsub::Event) -> Self {
        NodeEvent::Gossipsub(event)
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
    /// Peer has been added to the Routing Table. And the number of connected peers.
    PeerAdded(PeerId, usize),
    // Peer has been removed from the Routing Table. And the number of connected peers.
    PeerRemoved(PeerId, usize),
    /// The records bearing these keys are to be fetched from the holder or the network
    KeysForReplication(Vec<(PeerId, RecordKey)>),
    /// Started listening on a new address
    NewListenAddr(Multiaddr),
    /// AutoNAT status changed
    NatStatusChanged(NatStatus),
    /// Report unverified record
    UnverifiedRecord(Record),
    /// Report failed write to cleanup record store
    FailedToWrite(RecordKey),
    /// Gossipsub message received
    GossipsubMsgReceived {
        /// Topic the message was published on
        topic: String,
        /// The raw bytes of the received message
        msg: Bytes,
    },
    /// The Gossipsub message that we published
    GossipsubMsgPublished {
        /// Topic the message was published on
        topic: String,
        /// The raw bytes of the sent message
        msg: Bytes,
    },
}

// Manually implement Debug as `#[debug(with = "unverified_record_fmt")]` not working as expected.
impl Debug for NetworkEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NetworkEvent::RequestReceived { req, .. } => {
                write!(f, "NetworkEvent::RequestReceived({req:?})")
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
            NetworkEvent::KeysForReplication(list) => {
                let pretty_list: Vec<_> = list
                    .iter()
                    .map(|(holder, key)| (*holder, PrettyPrintRecordKey::from(key)))
                    .collect();
                write!(f, "NetworkEvent::KeysForReplication({pretty_list:?})")
            }
            NetworkEvent::NewListenAddr(addr) => {
                write!(f, "NetworkEvent::NewListenAddr({addr:?})")
            }
            NetworkEvent::NatStatusChanged(nat_status) => {
                write!(f, "NetworkEvent::NatStatusChanged({nat_status:?})")
            }
            NetworkEvent::UnverifiedRecord(record) => {
                let pretty_key = PrettyPrintRecordKey::from(&record.key);
                write!(f, "NetworkEvent::UnverifiedRecord({pretty_key:?})")
            }
            NetworkEvent::FailedToWrite(record_key) => {
                let pretty_key = PrettyPrintRecordKey::from(record_key);
                write!(f, "NetworkEvent::FailedToWrite({pretty_key:?})")
            }
            NetworkEvent::GossipsubMsgReceived { topic, .. } => {
                write!(f, "NetworkEvent::GossipsubMsgReceived({topic})")
            }
            NetworkEvent::GossipsubMsgPublished { topic, .. } => {
                write!(f, "NetworkEvent::GossipsubMsgPublished({topic})")
            }
        }
    }
}

impl SwarmDriver {
    /// Handle `SwarmEvents`
    pub(super) fn handle_swarm_events(&mut self, event: SwarmEvent<NodeEvent>) -> Result<()> {
        // This does not record all the events. `SwarmEvent::Behaviour(_)` are skipped. Hence `.record()` has to be
        // called individually on each behaviour.
        #[cfg(feature = "open-metrics")]
        self.network_metrics.record(&event);
        let start = std::time::Instant::now();
        let event_string;
        match event {
            SwarmEvent::Behaviour(NodeEvent::MsgReceived(event)) => {
                event_string = "msg_received";
                if let Err(e) = self.handle_msg(event) {
                    warn!("MsgReceivedError: {e:?}");
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Kademlia(kad_event)) => {
                event_string = "kad_event";
                #[cfg(feature = "open-metrics")]
                self.network_metrics.record(&(kad_event));
                self.handle_kad_event(kad_event)?;
            }
            // Handle the Identify event from the libp2p swarm.
            SwarmEvent::Behaviour(NodeEvent::Identify(iden)) => {
                event_string = "identify";
                // Record the Identify event for metrics if the feature is enabled.
                #[cfg(feature = "open-metrics")]
                self.network_metrics.record(&(*iden));
                // Match on the Identify event.
                match *iden {
                    // If the event is a Received event, handle the received peer information.
                    libp2p::identify::Event::Received { peer_id, info } => {
                        trace!(%peer_id, ?info, "identify: received info");

                        // If we are not local, we care only for peers that we dialed and thus are reachable.
                        if self.local
                            || self.dialed_peers.contains(&peer_id)
                                && info
                                    .agent_version
                                    .starts_with(truncate_patch_version(IDENTIFY_AGENT_STR))
                        {
                            // If we're not in local mode, only add globally reachable addresses.
                            // Strip the `/p2p/...` part of the multiaddresses.
                            // Collect into a HashSet directly to avoid multiple allocations and handle deduplication.
                            let addrs: HashSet<Multiaddr> = match self.local {
                                true => info
                                    .listen_addrs
                                    .into_iter()
                                    .map(|addr| multiaddr_strip_p2p(&addr))
                                    .collect(),
                                false => info
                                    .listen_addrs
                                    .into_iter()
                                    .filter(multiaddr_is_global)
                                    .map(|addr| multiaddr_strip_p2p(&addr))
                                    .collect(),
                            };

                            // Attempt to add the addresses to the routing table.
                            for multiaddr in &addrs {
                                trace!(%peer_id, ?addrs, "identify: attempting to add addresses to routing table");

                                let _routing_update = self
                                    .swarm
                                    .behaviour_mut()
                                    .kademlia
                                    .add_address(&peer_id, multiaddr.clone());
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
                    // Log the other Identify events.
                    libp2p::identify::Event::Sent { .. } => trace!("identify: {iden:?}"),
                    libp2p::identify::Event::Pushed { .. } => trace!("identify: {iden:?}"),
                    libp2p::identify::Event::Error { .. } => trace!("identify: {iden:?}"),
                }
            }
            #[cfg(feature = "local-discovery")]
            SwarmEvent::Behaviour(NodeEvent::Mdns(mdns_event)) => {
                event_string = "mdns";
                match *mdns_event {
                    mdns::Event::Discovered(list) => {
                        if self.local {
                            for (peer_id, addr) in list {
                                // The multiaddr does not contain the peer ID, so add it.
                                let addr = addr.with(Protocol::P2p(peer_id));

                                info!(%addr, "mDNS node discovered and dialing");

                                if let Err(err) = self.dial(addr.clone()) {
                                    warn!(%addr, "mDNS node dial error: {err:?}");
                                }
                            }
                        }
                    }
                    mdns::Event::Expired(peer) => {
                        trace!("mdns peer {peer:?} expired");
                    }
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Autonat(event)) => {
                event_string = "autonat";
                match event {
                    autonat::Event::InboundProbe(e) => trace!("AutoNAT inbound probe: {e:?}"),
                    autonat::Event::OutboundProbe(e) => trace!("AutoNAT outbound probe: {e:?}"),
                    autonat::Event::StatusChanged { old, new } => {
                        info!("AutoNAT status changed: {old:?} -> {new:?}");
                        self.send_event(NetworkEvent::NatStatusChanged(new.clone()));

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
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Gossipsub(event)) => {
                event_string = "gossip";

                #[cfg(feature = "open-metrics")]
                self.network_metrics.record(&event);
                if self.is_gossip_listener {
                    match event {
                        libp2p::gossipsub::Event::Message {
                            message,
                            message_id,
                            ..
                        } => {
                            info!("Gossipsub message received, id: {message_id:?}");
                            let topic = message.topic.into_string();
                            let msg = Bytes::from(message.data);
                            self.send_event(NetworkEvent::GossipsubMsgReceived { topic, msg });
                        }
                        other => trace!("Gossipsub Event has been ignored: {other:?}"),
                    }
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                event_string = "new listen addr";

                let local_peer_id = *self.swarm.local_peer_id();
                let address = address.with(Protocol::P2p(local_peer_id));

                // Trigger server mode if we're not a client
                if !self.is_client {
                    if self.local {
                        // all addresses are effectively external here...
                        // this is needed for Kad Mode::Server
                        self.swarm.add_external_address(address.clone());
                    } else {
                        // only add our global addresses
                        if multiaddr_is_global(&address) {
                            self.swarm.add_external_address(address.clone());
                        }
                    }
                }

                self.send_event(NetworkEvent::NewListenAddr(address.clone()));

                info!("Local node is listening on {address:?}");
            }
            SwarmEvent::IncomingConnection {
                connection_id,
                local_addr,
                send_back_addr,
            } => {
                event_string = "incoming";

                trace!("IncomingConnection ({connection_id:?}) with local_addr: {local_addr:?} send_back_addr: {send_back_addr:?}");
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                connection_id,
                ..
            } => {
                event_string = "ConnectionEstablished";
                trace!(%peer_id, num_established, "ConnectionEstablished ({connection_id:?}): {}", endpoint_str(&endpoint));

                if endpoint.is_dialer() {
                    self.dialed_peers
                        .push(peer_id)
                        .map_err(|_| Error::CircularVecPopFrontError)?;
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                cause,
                num_established,
                connection_id,
            } => {
                event_string = "ConnectionClosed";
                trace!(%peer_id, ?connection_id, ?cause, num_established, "ConnectionClosed: {}", endpoint_str(&endpoint));
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(failed_peer_id),
                error,
                connection_id,
            } => {
                event_string = "Outgoing`ConnErr";
                error!("OutgoingConnectionError to {failed_peer_id:?} on {connection_id:?} - {error:?}");
                if let Some(dead_peer) = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .remove_peer(&failed_peer_id)
                {
                    self.connected_peers = self.connected_peers.saturating_sub(1);
                    self.send_event(NetworkEvent::PeerRemoved(
                        *dead_peer.node.key.preimage(),
                        self.connected_peers,
                    ));
                    self.log_kbuckets(&failed_peer_id);
                    let _ = self.check_for_change_in_our_close_group();
                }
            }
            SwarmEvent::IncomingConnectionError {
                connection_id,
                local_addr,
                send_back_addr,
                error,
            } => {
                event_string = "Incoming ConnErr";
                error!("IncomingConnectionError from local_addr:?{local_addr:?}, send_back_addr {send_back_addr:?} on {connection_id:?} with error {error:?}");
            }
            SwarmEvent::Dialing {
                peer_id,
                connection_id,
            } => {
                event_string = "Dialing";
                trace!("Dialing {peer_id:?} on {connection_id:?}");
            }
            other => {
                event_string = "Other";

                trace!("SwarmEvent has been ignored: {other:?}")
            }
        }
        trace!(
            "SwarmEvent handled in {:?}: {event_string:?}",
            start.elapsed()
        );
        Ok(())
    }

    /// Forwards `Request` to the upper layers using `Sender<NetworkEvent>`. Sends `Response` to the peers
    pub fn handle_msg(
        &mut self,
        event: request_response::Event<Request, Response>,
    ) -> Result<(), Error> {
        match event {
            request_response::Event::Message { message, peer } => match message {
                Message::Request {
                    request,
                    channel,
                    request_id,
                    ..
                } => {
                    trace!("Received request {request_id:?} from peer {peer:?}, req: {request:?}");
                    self.send_event(NetworkEvent::RequestReceived {
                        req: request,
                        channel: MsgResponder::FromPeer(channel),
                    })
                }
                Message::Response {
                    request_id,
                    response,
                } => {
                    trace!("Got response {request_id:?} from peer {peer:?}, res: {response}.");
                    if let Some(sender) = self.pending_requests.remove(&request_id) {
                        // The sender will be provided if the caller (Requester) is awaiting for a response
                        // at the call site.
                        // Else the Request was just sent to the peer and the Response was
                        // meant to be handled in another way and is not awaited.
                        match sender {
                            Some(sender) => sender
                                .send(Ok(response))
                                .map_err(|_| Error::InternalMsgChannelDropped)?,
                            None => {
                                // responses that are not awaited at the call site must be handled
                                // separately
                                self.send_event(NetworkEvent::ResponseReceived { res: response });
                            }
                        }
                    } else {
                        warn!("Tried to remove a RequestId from pending_requests which was not inserted in the first place.
                            Use Cmd::SendRequest with sender:None if you want the Response to be fed into the common handle_response function");
                    }
                }
            },
            request_response::Event::OutboundFailure {
                request_id,
                error,
                peer,
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    match sender {
                        Some(sender) => {
                            sender
                                .send(Err(error.into()))
                                .map_err(|_| Error::InternalMsgChannelDropped)?;
                        }
                        None => {
                            warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                            return Err(Error::ReceivedResponseDropped(request_id));
                        }
                    }
                } else {
                    warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                    return Err(Error::ReceivedResponseDropped(request_id));
                }
            }
            request_response::Event::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                warn!("RequestResponse: InboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
            }
            request_response::Event::ResponseSent { peer, request_id } => {
                trace!("ResponseSent for request_id: {request_id:?} and peer: {peer:?}");
            }
        }
        Ok(())
    }

    fn handle_kad_event(&mut self, kad_event: kad::Event) -> Result<()> {
        #[cfg(feature = "open-metricss")]
        self.network_metrics.record(&kad_event);
        let start = std::time::Instant::now();
        let event_string;

        match kad_event {
            ref event @ kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetClosestPeers(Ok(ref closest_peers)),
                ref stats,
                ref step,
            } => {
                event_string = "kad_event::get_closest_peers";
                trace!(
                    "Query task {id:?} returned with peers {closest_peers:?}, {stats:?} - {step:?}"
                );

                let (sender, mut current_closest) =
                    self.pending_get_closest_peers.remove(&id).ok_or_else(|| {
                        trace!(
                            "Can't locate query task {id:?}, it has likely been completed already."
                        );
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
            // Handle GetClosestPeers timeouts
            ref event @ kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetClosestPeers(Err(ref err)),
                ref stats,
                ref step,
            } => {
                event_string = "kad_event::get_closest_peers_err";
                error!("GetClosest Query task {id:?} errored with {err:?}, {stats:?} - {step:?}");

                let (sender, mut current_closest) =
                    self.pending_get_closest_peers.remove(&id).ok_or_else(|| {
                        trace!(
                            "Can't locate query task {id:?}, it has likely been completed already."
                        );
                        Error::ReceivedKademliaEventDropped(event.clone())
                    })?;

                // We have `current_closest` from previous progress,
                // and `peers` from `GetClosestPeersError`.
                // Trust them and leave for the caller to check whether they are enough.
                match err {
                    GetClosestPeersError::Timeout { ref peers, .. } => {
                        current_closest.extend(peers);
                    }
                }

                sender
                    .send(current_closest)
                    .map_err(|_| Error::InternalMsgChannelDropped)?;
            }

            // For `get_record` returning behaviour:
            //   1, targeting a non-existing entry
            //     there will only be one event of `kad::Event::OutboundQueryProgressed`
            //     with `ProgressStep::last` to be `true`
            //          `QueryStats::requests` to be 20 (K-Value)
            //          `QueryStats::success` to be over majority of the requests
            //          `err::NotFound::closest_peers` contains a list of CLOSE_GROUP_SIZE peers
            //   2, targeting an existing entry
            //     there will a sequence of (at least CLOSE_GROUP_SIZE) events of
            //     `kad::Event::OutboundQueryProgressed` to be received
            //     with `QueryStats::end` always being `None`
            //          `ProgressStep::last` all to be `false`
            //          `ProgressStep::count` to be increased with step of 1
            //             capped and stopped at CLOSE_GROUP_SIZE, may have duplicated counts
            //          `PeerRecord::peer` could be None to indicate from self
            //             in which case it always use a duplicated `ProgressStep::count`
            //     the sequence will be completed with `FinishedWithNoAdditionalRecord`
            //     where: `cache_candidates`: being the peers supposed to hold the record but not
            //            `ProgressStep::count`: to be `number of received copies plus one`
            //            `ProgressStep::last` to be `true`
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Ok(GetRecordOk::FoundRecord(peer_record))),
                stats,
                step,
            } => {
                event_string = "kad_event::get_record::found";
                trace!(
                    "Query task {id:?} returned with record {:?} from peer {:?}, {stats:?} - {step:?}",
                    PrettyPrintRecordKey::from(&peer_record.record.key),
                    peer_record.peer
                );
                self.accumulate_get_record_ok(id, peer_record, step.count);
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    QueryResult::GetRecord(Ok(GetRecordOk::FinishedWithNoAdditionalRecord {
                        cache_candidates,
                    })),
                stats,
                step,
            } => {
                event_string = "kad_event::get_record::finished_no_additional";
                trace!("Query task {id:?} of get_record completed with {stats:?} - {step:?} - {cache_candidates:?}");
                if let Some((sender, result_map, _quorum, expected_holders)) =
                    self.pending_get_record.remove(&id)
                {
                    let num_of_versions = result_map.len();
                    let (result, log_string) = if let Some((record, _)) = result_map.values().next()
                    {
                        let result = if num_of_versions == 1 {
                            Err(Error::RecordNotEnoughCopies(record.clone()))
                        } else {
                            Err(Error::SplitRecord {
                                result_map: result_map.clone(),
                            })
                        };

                        (result, format!(
                            "Getting record {:?} completed with only {:?} copies received, and {num_of_versions} versions.",
                            PrettyPrintRecordKey::from(&record.key),
                            usize::from(step.count) - 1
                        ))
                    } else {
                        (Err(Error::RecordNotFound),
                        format!(
                            "Getting record task {id:?} completed with step count {:?}, but no copy found.",
                            step.count
                        ))
                    };

                    if expected_holders.is_empty() {
                        debug!("{log_string}");
                    } else {
                        debug!(
                            "{log_string}, and {expected_holders:?} expected holders not responded"
                        );
                    }

                    sender
                        .send(result)
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                }
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Err(err)),
                stats,
                step,
            } => {
                match err.clone() {
                    GetRecordError::NotFound { key, closest_peers } => {
                        event_string = "kad_event::GetRecordError::NotFound";
                        info!("Query task {id:?} NotFound record {:?} among peers {closest_peers:?}, {stats:?} - {step:?}",
                        PrettyPrintRecordKey::from(&key));
                    }
                    GetRecordError::QuorumFailed {
                        key,
                        records,
                        quorum,
                    } => {
                        event_string = "kad_event::GetRecordError::QuorumFailed";
                        let pretty_key = PrettyPrintRecordKey::from(&key);
                        let peers = records
                            .iter()
                            .map(|peer_record| peer_record.peer)
                            .collect_vec();
                        info!("Query task {id:?} QuorumFailed record {pretty_key:?} among peers {peers:?} with quorum {quorum:?}, {stats:?} - {step:?}");
                    }
                    GetRecordError::Timeout { key } => {
                        event_string = "kad_event::GetRecordError::Timeout";
                        let pretty_key = PrettyPrintRecordKey::from(&key);

                        debug!(
                            "Query task {id:?} timed out when looking for record {pretty_key:?}"
                        );

                        let (sender, result_map, quorum, expected_holders) =
                            self.pending_get_record.remove(&id).ok_or_else(|| {
                                trace!(
                                    "Can't locate query task {id:?} for {pretty_key:?}, it has likely been completed already."
                                );
                                Error::ReceivedKademliaEventDropped( kad::Event::OutboundQueryProgressed {
                                    id,
                                    result: QueryResult::GetRecord(Err(err.clone())),
                                    stats,
                                    step,
                                })
                            })?;

                        let required_response_count = match quorum {
                            GetQuorum::Majority => close_group_majority(),
                            GetQuorum::All => CLOSE_GROUP_SIZE,
                            GetQuorum::N(v) => v.into(),
                            GetQuorum::One => 1,
                        };

                        // if we've a split over the result xorname, then we don't attempt to resolve this here.
                        // Retry and resolve through normal flows without a timeout.
                        if result_map.len() > 1 {
                            warn!("Get record task {id:?} for {pretty_key:?} timed out with split result map");
                            sender
                                .send(Err(Error::QueryTimeout))
                                .map_err(|_| Error::InternalMsgChannelDropped)?;
                            debug!(
                                "KadEvent {event_string:?} completed after {:?}",
                                start.elapsed()
                            );

                            return Ok(());
                        }

                        // if we have enough responses here, we can return the record
                        if let Some((record, peers)) = result_map.values().next() {
                            if peers.len() >= required_response_count {
                                sender
                                    .send(Ok(record.clone()))
                                    .map_err(|_| Error::InternalMsgChannelDropped)?;

                                debug!(
                                    "KadEvent {event_string:?} completed after {:?}",
                                    start.elapsed()
                                );

                                return Ok(());
                            }
                        }

                        warn!("Get record task {id:?} for {pretty_key:?} returned insufficient responses. {expected_holders:?} did not return record");
                        // Otherwise report the timeout
                        sender
                            .send(Err(Error::QueryTimeout))
                            .map_err(|_| Error::InternalMsgChannelDropped)?;

                        debug!(
                            "KadEvent {event_string:?} completed after {:?}",
                            start.elapsed()
                        );
                        return Ok(());
                    }
                }

                if let Some((sender, _, _, expected_holders)) = self.pending_get_record.remove(&id)
                {
                    if expected_holders.is_empty() {
                        info!("Get record task {id:?} failed with error {err:?}");
                    } else {
                        debug!("Get record task {id:?} failed with {expected_holders:?} expected holders not responded, error {err:?}");
                    }
                    sender
                        .send(Err(Error::RecordNotFound))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                }
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::Bootstrap(bootstrap_result),
                step,
                ..
            } => {
                event_string = "kad_event::OutboundQueryProgressed::Bootstrap";
                // here BootstrapOk::num_remaining refers to the remaining random peer IDs to query, one per
                // bucket that still needs refreshing.
                trace!("Kademlia Bootstrap with {id:?} progressed with {bootstrap_result:?} and step {step:?}");
                if step.last {
                    // inform the bootstrap process about the completion.
                    self.bootstrap.completed();
                }
            }
            kad::Event::RoutingUpdated {
                peer,
                is_new_peer,
                old_peer,
                ..
            } => {
                event_string = "kad_event::RoutingUpdated";
                if is_new_peer {
                    self.connected_peers = self.connected_peers.saturating_add(1);

                    info!("New peer added to routing table: {peer:?}, now we have #{} connected peers", self.connected_peers);
                    self.log_kbuckets(&peer);

                    if self.bootstrap.notify_new_peer() {
                        info!("Performing the first bootstrap");
                        self.initiate_bootstrap();
                    }
                    self.send_event(NetworkEvent::PeerAdded(peer, self.connected_peers));
                }

                if old_peer.is_some() {
                    self.connected_peers = self.connected_peers.saturating_sub(1);

                    info!("Evicted old peer on new peer join: {old_peer:?}");
                    self.send_event(NetworkEvent::PeerRemoved(peer, self.connected_peers));
                    self.log_kbuckets(&peer);
                }
                let _ = self.check_for_change_in_our_close_group();
            }
            kad::Event::InboundRequest {
                request: InboundRequest::PutRecord { .. },
            } => {
                event_string = "kad_event::InboundRequest::PutRecord";
                // Ignored to reduce logging. When `Record filtering` is enabled,
                // the `record` variable will contain the content for further validation before put.
            }
            kad::Event::InboundRequest {
                request: InboundRequest::FindNode { .. },
            } => {
                event_string = "kad_event::InboundRequest::FindNode";
                // Ignored to reduce logging. With continuous bootstrap, this is triggered often.
            }
            kad::Event::InboundRequest {
                request:
                    InboundRequest::GetRecord {
                        num_closer_peers,
                        present_locally,
                    },
            } => {
                event_string = "kad_event::InboundRequest::GetRecord";
                if !present_locally && num_closer_peers < CLOSE_GROUP_SIZE {
                    trace!("InboundRequest::GetRecord doesn't have local record, with {num_closer_peers:?} closer_peers");
                }
            }
            kad::Event::UnroutablePeer { peer } => {
                event_string = "kad_event::UnroutablePeer";
                trace!(peer_id = %peer, "kad::Event: UnroutablePeer");
            }
            other => {
                event_string = "kad_event::Other";
                trace!("kad::Event ignored: {other:?}");
            }
        }

        trace!(
            "kad::Event handled in {:?}: {event_string:?}",
            start.elapsed()
        );

        Ok(())
    }

    /// Check for changes in our close group
    ///
    fn check_for_change_in_our_close_group(&mut self) -> bool {
        let closest_k_peers = self.get_closest_k_value_local_peers();

        let new_closest_peers = {
            match sort_peers_by_address(
                &closest_k_peers,
                &NetworkAddress::from_peer(self.self_peer_id),
                CLOSE_GROUP_SIZE,
            ) {
                Err(error) => {
                    error!("Failed to sort peers by address: {error:?}");
                    return false;
                }
                Ok(closest_k_peers) => closest_k_peers,
            }
        };

        let old = self.close_group.iter().cloned().collect::<HashSet<_>>();
        let new_members: Vec<_> = new_closest_peers
            .iter()
            .filter(|p| !old.contains(p))
            .collect();
        if !new_members.is_empty() {
            debug!("The close group has been updated. The new members are {new_members:?}");
            debug!("New close group: {new_closest_peers:?}");
            self.close_group = new_closest_peers.into_iter().cloned().collect();
            let _ = self.update_record_distance_range();
            true
        } else {
            false
        }
    }

    /// Set the acceptable range of record entry. A record is removed from the storage if the
    /// distance between the record and the node is greater than the `distance_range`
    fn update_record_distance_range(&mut self) -> Option<()> {
        let our_address = NetworkAddress::from_peer(self.self_peer_id);
        let distance_range = self
            .close_group
            .last()
            .map(|peer| NetworkAddress::from_peer(*peer).distance(&our_address))?;

        self.swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .set_distance_range(distance_range);
        debug!("set distance_range successfully to {distance_range:?}");
        Some(())
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

    // Completes when any of the following condition reaches first:
    // 1, Return whenever reached majority of CLOSE_GROUP_SIZE
    // 2, In case of split, return with NotFound,
    //    whenever `ProgressStep::count` hits CLOSE_GROUP_SIZE
    fn accumulate_get_record_ok(
        &mut self,
        query_id: QueryId,
        peer_record: PeerRecord,
        count: NonZeroUsize,
    ) {
        if self.try_early_completion_for_chunk(&query_id, &peer_record) {
            return;
        }

        let peer_id = if let Some(peer_id) = peer_record.peer {
            peer_id
        } else {
            self.self_peer_id
        };

        if let Entry::Occupied(mut entry) = self.pending_get_record.entry(query_id) {
            let (_sender, result_map, quorum, expected_holders) = entry.get_mut();

            let pretty_key = PrettyPrintRecordKey::from(&peer_record.record.key).into_owned();

            if !expected_holders.is_empty() {
                if expected_holders.remove(&peer_id) {
                    debug!("For record {pretty_key:?} task {query_id:?}, received a copy from an expected holder {peer_id:?}");
                } else {
                    debug!("For record {pretty_key:?} task {query_id:?}, received a copy from an unexpected holder {peer_id:?}");
                }
            }

            let record_content_hash = XorName::from_content(&peer_record.record.value);
            let peer_list =
                if let Some((_, mut peer_list)) = result_map.remove(&record_content_hash) {
                    let _ = peer_list.insert(peer_id);
                    peer_list
                } else {
                    let mut peer_list = HashSet::new();
                    let _ = peer_list.insert(peer_id);
                    peer_list
                };

            let expected_answers = match quorum {
                GetQuorum::Majority => close_group_majority(),
                GetQuorum::All => CLOSE_GROUP_SIZE,
                GetQuorum::N(v) => v.get(),
                GetQuorum::One => 1,
            };

            let responded_peers = peer_list.len();
            trace!("Expecting {expected_answers:?} answers for record {pretty_key:?} task {query_id:?}, received {responded_peers} so far");

            let _ = result_map.insert(record_content_hash, (peer_record.record.clone(), peer_list));

            if responded_peers >= expected_answers {
                if !expected_holders.is_empty() {
                    debug!("For record {pretty_key:?} task {query_id:?}, fetch completed with non-responded expected holders {expected_holders:?}");
                }

                // Remove the query task and consume the variables.
                let (sender, result_map, _, _) = entry.remove();

                if result_map.len() == 1 {
                    let _ = sender.send(Ok(peer_record.record));
                } else {
                    debug!("For record {pretty_key:?} task {query_id:?}, fetch completed with split record");
                    let _ = sender.send(Err(Error::SplitRecord { result_map }));
                }

                // Stop the query; possibly stops more nodes from being queried.
                if let Some(mut query) = self.swarm.behaviour_mut().kademlia.query_mut(&query_id) {
                    query.finish();
                }
            } else if usize::from(count) >= CLOSE_GROUP_SIZE {
                debug!("For record {pretty_key:?} task {query_id:?}, got {count:?} with {} versions so far.",
                    result_map.len());
            }
        }
    }

    // For chunk record which can be self-verifiable,
    // complete the flow with the first copy that fetched.
    // Return `true` if early completed, otherwise return `false`.
    // Situations that can be early completed:
    // 1, Not finding an entry within pending_get_record, i.e. no more further action required
    // 2, For a `Chunk` that not required to verify expected holders,
    //    whenever fetched a first copy that passed the self-verification.
    fn try_early_completion_for_chunk(
        &mut self,
        query_id: &QueryId,
        peer_record: &PeerRecord,
    ) -> bool {
        if let Entry::Occupied(mut entry) = self.pending_get_record.entry(*query_id) {
            let (_, _, quorum, expected_holders) = entry.get_mut();

            if expected_holders.is_empty() &&
               RecordHeader::is_record_of_type_chunk(&peer_record.record).unwrap_or(false) &&
               // Ensure that we only exit early if quorum is indeed for only one match
               matches!(quorum, GetQuorum::One)
            {
                // Stop the query; possibly stops more nodes from being queried.
                if let Some(mut query) = self.swarm.behaviour_mut().kademlia.query_mut(query_id) {
                    query.finish();
                }

                // Stop tracking the query task by removing the entry and consume the sender.
                let (sender, ..) = entry.remove();
                // A claimed Chunk type record can be trusted.
                // Punishment of peer that sending corrupted Chunk type record
                // maybe carried out by other verification mechanism.
                let _ = sender.send(Ok(peer_record.record.clone()));
                return true;
            }
        } else {
            // A non-existing pending entry does not need to undertake any further action.
            return true;
        }

        false
    }
}

/// Helper function to print formatted connection role info.
fn endpoint_str(endpoint: &libp2p::core::ConnectedPoint) -> String {
    match endpoint {
        libp2p::core::ConnectedPoint::Dialer { address, .. } => {
            format!("outgoing ({address})")
        }
        libp2p::core::ConnectedPoint::Listener { send_back_addr, .. } => {
            format!("incoming ({send_back_addr})")
        }
    }
}
