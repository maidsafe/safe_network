// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    close_group_majority,
    driver::SwarmDriver,
    error::{Error, Result},
    multiaddr_is_global, multiaddr_strip_p2p, sort_peers_by_address, CLOSE_GROUP_SIZE,
};
use core::fmt;
use custom_debug::Debug as CustomDebug;
use itertools::Itertools;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    autonat::{self, NatStatus},
    kad::{
        GetRecordError, GetRecordOk, InboundRequest, KademliaEvent, PeerRecord, QueryId,
        QueryResult, Record, RecordKey, K_VALUE,
    },
    multiaddr::Protocol,
    request_response::{self, Message, ResponseChannel as PeerResponseChannel},
    swarm::{dial_opts::DialOpts, SwarmEvent},
    Multiaddr, PeerId,
};
#[cfg(feature = "open-metrics")]
use libp2p_metrics::Recorder;
use sn_protocol::{
    messages::{Request, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{Debug, Formatter},
    num::NonZeroUsize,
};
use tokio::sync::oneshot;
use tracing::{info, warn};
use xor_name::XorName;

/// Our agent string has as a prefix that we can match against.
const IDENTIFY_AGENT_STR: &str = "safe/node/";

/// Using XorName to differentiate different record content under the same key.
pub(super) type GetRecordResultMap = HashMap<XorName, (Record, HashSet<PeerId>)>;

/// NodeEvent enum
#[derive(CustomDebug)]
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
    /// Peer has been added to the Routing Table
    PeerAdded(PeerId),
    // Peer has been removed from the Routing Table
    PeerRemoved(PeerId),
    /// The Records for the these keys are to be fetched from the network
    KeysForReplication(Vec<RecordKey>),
    /// Started listening on a new address
    NewListenAddr(Multiaddr),
    /// AutoNAT status changed
    NatStatusChanged(NatStatus),
    /// Report unverified record
    UnverifiedRecord(Record),
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
            NetworkEvent::PeerAdded(peer_id) => {
                write!(f, "NetworkEvent::PeerAdded({peer_id:?})")
            }
            NetworkEvent::PeerRemoved(peer_id) => {
                write!(f, "NetworkEvent::PeerRemoved({peer_id:?})")
            }
            NetworkEvent::KeysForReplication(list) => {
                let pretty_list: Vec<_> = list
                    .iter()
                    .map(|key| PrettyPrintRecordKey::from(key.clone()))
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
                let pretty_key = PrettyPrintRecordKey::from(record.key.clone());
                write!(f, "NetworkEvent::UnverifiedRecord({pretty_key:?})")
            }
        }
    }
}

impl SwarmDriver {
    // Handle `SwarmEvents`
    pub(super) fn handle_swarm_events<EventError: std::error::Error>(
        &mut self,
        event: SwarmEvent<NodeEvent, EventError>,
    ) -> Result<()> {
        // This does not record all the events. `SwarmEvent::Behaviour(_)` are skipped. Hence `.record()` has to be
        // called individually on each behaviour.
        #[cfg(feature = "open-metrics")]
        self.network_metrics.record(&event);
        match event {
            SwarmEvent::Behaviour(NodeEvent::MsgReceived(event)) => {
                if let Err(e) = self.handle_msg(event) {
                    warn!("MsgReceivedError: {e:?}");
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Kademlia(kad_event)) => {
                self.handle_kad_event(kad_event)?;
            }
            SwarmEvent::Behaviour(NodeEvent::Identify(iden)) => {
                #[cfg(feature = "open-metrics")]
                self.network_metrics.record(&(*iden));
                match *iden {
                    libp2p::identify::Event::Received { peer_id, info } => {
                        trace!(%peer_id, ?info, "identify: received info");

                        // If we are not local, we care only for peers that we dialed and thus are reachable.
                        if (self.local
                            || self.dialed_peers.contains(&peer_id)
                            || self.unroutable_peers.contains(&peer_id))
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
                                // If the peer was unroutable, we dial it.
                                if !self.dialed_peers.contains(&peer_id)
                                    && self.unroutable_peers.contains(&peer_id)
                                {
                                    debug!("identify: dialing unroutable peer by its announced listen address");
                                    if let Err(err) = self.dial_with_opts(
                                        DialOpts::peer_id(peer_id)
                                            // By default the condition is 'Disconnected'. But we still want to establish an outbound connection,
                                            // even if there already is an inbound connection
                                            .condition(
                                                libp2p::swarm::dial_opts::PeerCondition::NotDialing,
                                            )
                                            .build(),
                                    ) {
                                        match err {
                                            // If we are already dialing the peer, that's fine, otherwise report error.
                                            libp2p::swarm::DialError::DialPeerConditionFalse(
                                                libp2p::swarm::dial_opts::PeerCondition::NotDialing,
                                            ) => {}
                                            _ => {
                                                error!(%peer_id, "identify: dial attempt error: {err:?}");
                                            }
                                        }
                                    }
                                } else {
                                    let _routing_update = self
                                        .swarm
                                        .behaviour_mut()
                                        .kademlia
                                        .add_address(&peer_id, multiaddr);
                                }
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
            },
            SwarmEvent::Behaviour(NodeEvent::Autonat(event)) => match event {
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
            },
            SwarmEvent::NewListenAddr { address, .. } => {
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
                trace!("IncomingConnection ({connection_id:?}) with local_addr: {local_addr:?} send_back_addr: {send_back_addr:?}");
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                connection_id,
                ..
            } => {
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
                trace!(%peer_id, ?connection_id, ?cause, num_established, "ConnectionClosed: {}", endpoint_str(&endpoint));
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(failed_peer_id),
                error,
                connection_id,
            } => {
                error!("OutgoingConnectionError to {failed_peer_id:?} on {connection_id:?} - {error:?}");
                if let Some(dead_peer) = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .remove_peer(&failed_peer_id)
                {
                    self.send_event(NetworkEvent::PeerRemoved(*dead_peer.node.key.preimage()));
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
                error!("IncomingConnectionError from local_addr:?{local_addr:?}, send_back_addr {send_back_addr:?} on {connection_id:?} with error {error:?}");
            }
            SwarmEvent::Dialing {
                peer_id,
                connection_id,
            } => trace!("Dialing {peer_id:?} on {connection_id:?}"),
            other => trace!("SwarmEvent has been ignored: {other:?}"),
        }
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

    fn handle_kad_event(&mut self, kad_event: KademliaEvent) -> Result<()> {
        #[cfg(feature = "open-metricss")]
        self.network_metrics.record(&kad_event);
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
            // For `get_record` returning behaviour:
            //   1, targeting a non-existing entry
            //     there will only be one event of `KademliaEvent::OutboundQueryProgressed`
            //     with `ProgressStep::last` to be `true`
            //          `QueryStats::requests` to be 20 (K-Value)
            //          `QueryStats::success` to be over majority of the requests
            //          `err::NotFound::closest_peers` contains a list of CLOSE_GROUP_SIZE peers
            //   2, targeting an existing entry
            //     there will a sequence of (at least CLOSE_GROUP_SIZE) events of
            //     `KademliaEvent::OutboundQueryProgressed` to be received
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
            KademliaEvent::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Ok(GetRecordOk::FoundRecord(peer_record))),
                stats,
                step,
            } => {
                let content_hash = XorName::from_content(&peer_record.record.value);
                trace!(
                    "Query task {id:?} returned with record {:?}(content {content_hash:?}) from peer {:?}, {stats:?} - {step:?}",
                    PrettyPrintRecordKey::from(peer_record.record.key.clone()),
                    peer_record.peer
                );
                self.accumulate_get_record_ok(id, peer_record, step.count);
            }
            KademliaEvent::OutboundQueryProgressed {
                id,
                result:
                    QueryResult::GetRecord(Ok(GetRecordOk::FinishedWithNoAdditionalRecord { .. })),
                stats,
                step,
            } => {
                trace!("Query task {id:?} of get_record completed with {stats:?} - {step:?}");
                if let Some((sender, result_map)) = self.pending_get_record.remove(&id) {
                    if let Some((record, _)) = result_map.values().next() {
                        debug!(
                            "Getting record {:?} early completed with {:?} copies received",
                            PrettyPrintRecordKey::from(record.key.clone()),
                            usize::from(step.count) - 1
                        );
                        // Consider any early completion as Putting in progress or split.
                        // Just send back the first record (for put verification only),
                        // and not to update self
                        sender
                            .send(Err(Error::RecordNotEnoughCopies(record.clone())))
                            .map_err(|_| Error::InternalMsgChannelDropped)?;
                    } else {
                        sender
                            .send(Err(Error::RecordNotFound))
                            .map_err(|_| Error::InternalMsgChannelDropped)?;
                    }
                }
            }
            KademliaEvent::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Err(err)),
                stats,
                step,
            } => {
                match err {
                    GetRecordError::NotFound { key, closest_peers } => {
                        info!("Query task {id:?} NotFound record {:?} among peers {closest_peers:?}, {stats:?} - {step:?}",
                            PrettyPrintRecordKey::from(key.clone()));
                    }
                    GetRecordError::QuorumFailed {
                        key,
                        records,
                        quorum,
                    } => {
                        let peers = records
                            .iter()
                            .map(|peer_record| peer_record.peer)
                            .collect_vec();
                        info!("Query task {id:?} QuorumFailed record {:?} among peers {peers:?} with quorum {quorum:?}, {stats:?} - {step:?}",
                            PrettyPrintRecordKey::from(key.clone()));
                    }
                    GetRecordError::Timeout { key } => {
                        info!(
                            "Query task {id:?} timed out when looking for record {:?}",
                            PrettyPrintRecordKey::from(key.clone())
                        );
                    }
                }

                if let Some((sender, _)) = self.pending_get_record.remove(&id) {
                    sender
                        .send(Err(Error::RecordNotFound))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                }
            }
            KademliaEvent::OutboundQueryProgressed {
                id,
                result: QueryResult::Bootstrap(bootstrap_result),
                step,
                ..
            } => {
                // here BootstrapOk::num_remaining refers to the remaining random peer IDs to query, one per
                // bucket that still needs refreshing.
                trace!("Kademlia Bootstrap with {id:?} progressed with {bootstrap_result:?} and step {step:?}");
                // set to false to enable another bootstrap step to be started if required.
                if step.last {
                    self.bootstrap_ongoing = false;
                }
            }
            KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer,
                old_peer,
                ..
            } => {
                if is_new_peer {
                    self.log_kbuckets(&peer);
                    self.send_event(NetworkEvent::PeerAdded(peer));
                    let connected_peers = self.swarm.connected_peers().count();

                    info!("Connected peers: {connected_peers}");
                    // kad bootstrap process needs at least one peer in the RT be carried out.
                    // Carry out bootstrap until we have at least CLOSE_GROUP_SIZE peers
                    if connected_peers <= CLOSE_GROUP_SIZE && !self.bootstrap_ongoing {
                        debug!("Trying to initiate bootstrap as we have less than {CLOSE_GROUP_SIZE} peers");
                        match self.swarm.behaviour_mut().kademlia.bootstrap() {
                            Ok(query_id) => {
                                debug!(
                                    "Initiated kad bootstrap process with query id {query_id:?}"
                                );
                                self.bootstrap_ongoing = true;
                            }
                            Err(err) => {
                                error!("Failed to initiate kad bootstrap with error: {err:?}")
                            }
                        };
                    }
                }

                if old_peer.is_some() {
                    info!("Evicted old peer on new peer join: {old_peer:?}");
                    self.send_event(NetworkEvent::PeerRemoved(peer));
                    self.log_kbuckets(&peer);
                }
                let _ = self.check_for_change_in_our_close_group();
            }
            KademliaEvent::InboundRequest {
                request: InboundRequest::PutRecord { .. },
            } => {
                // Ignored to reduce logging. When `Record filtering` is enabled,
                // the `record` variable will contain the content for further validation before put.
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
            KademliaEvent::UnroutablePeer { peer } => {
                trace!(peer_id = %peer, "KademliaEvent: UnroutablePeer");
                let _ = self.unroutable_peers.push(peer);
            }
            other => {
                trace!("KademliaEvent ignored: {other:?}");
            }
        }

        Ok(())
    }

    // Check for changes in our close group
    fn check_for_change_in_our_close_group(&mut self) -> Option<Vec<PeerId>> {
        let new_closest_peers = {
            let all_peers = self.get_all_local_peers();
            sort_peers_by_address(
                all_peers,
                &NetworkAddress::from_peer(self.self_peer_id),
                CLOSE_GROUP_SIZE,
            )
            .ok()?
        };

        let old = self.close_group.iter().cloned().collect::<HashSet<_>>();
        let new_members = new_closest_peers
            .iter()
            .filter(|p| !old.contains(p))
            .cloned()
            .collect::<Vec<_>>();
        if !new_members.is_empty() {
            debug!("The close group has been updated. The new members are {new_members:?}");
            debug!("New close group: {new_closest_peers:?}");
            self.close_group = new_closest_peers;
            let _ = self.update_record_distance_range();
            Some(new_members)
        } else {
            None
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
        let peer_id = if let Some(peer_id) = peer_record.peer {
            peer_id
        } else {
            self.self_peer_id
        };
        let record_content_hash = XorName::from_content(&peer_record.record.value);

        if let Some((sender, mut result_map)) = self.pending_get_record.remove(&query_id) {
            let peer_list =
                if let Some((_, mut peer_list)) = result_map.remove(&record_content_hash) {
                    let _ = peer_list.insert(peer_id);
                    peer_list
                } else {
                    let mut peer_list = HashSet::new();
                    let _ = peer_list.insert(peer_id);
                    peer_list
                };

            let result = if peer_list.len() >= close_group_majority() {
                Some(Ok(peer_record.record.clone()))
            } else if usize::from(count) >= CLOSE_GROUP_SIZE {
                Some(Err(Error::RecordNotFound))
            } else {
                None
            };

            let _ = result_map.insert(record_content_hash, (peer_record.record, peer_list));

            if let Some(result) = result {
                let _ = sender.send(result);
                self.try_update_self_for_split_record(result_map);
            } else {
                let _ = self
                    .pending_get_record
                    .insert(query_id, (sender, result_map));
            }
        }
    }

    // Split resolvement policy:
    // 1, Always choose the copy having the highest votes
    // 2, If multiple having same votes, chose the lowest XorName one
    //
    // Only update self when is among the `non-majority list`.
    // Trying to update other peers is un-necessary and may introduce extra holes.
    fn try_update_self_for_split_record(&mut self, result_map: GetRecordResultMap) {
        if result_map.len() == 1 {
            // Do nothing as there is no split votes
            return;
        }

        let mut highest_count = 0;
        let mut highest_records = BTreeMap::new();
        for (xor_name, (record, peer_list)) in &result_map {
            if peer_list.len() > highest_count {
                // Cleanup whenever there is a record got more votes
                highest_records = BTreeMap::new();
            }
            if peer_list.len() >= highest_count {
                highest_count = peer_list.len();
                let _ = highest_records.insert(xor_name, (record, peer_list));
            }
        }

        if let Some((_, (record, peer_list))) = highest_records.pop_first() {
            if !peer_list.contains(&self.self_peer_id) {
                warn!("Update self regarding a split record {:?}", record.key);
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .put_verified(record.clone());
            }
        }
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
