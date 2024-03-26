// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    driver::{truncate_patch_version, PendingGetClosestType, SwarmDriver},
    error::{NetworkError, Result},
    multiaddr_is_global, multiaddr_strip_p2p, sort_peers_by_address, CLOSE_GROUP_SIZE,
    REPLICATE_RANGE,
};
use core::fmt;
use custom_debug::Debug as CustomDebug;
use itertools::Itertools;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    kad::{self, GetClosestPeersError, InboundRequest, QueryResult, Record, RecordKey, K_VALUE},
    multiaddr::Protocol,
    request_response::{self, Message, ResponseChannel as PeerResponseChannel},
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        DialError, SwarmEvent,
    },
    Multiaddr, PeerId, TransportError,
};

use crate::target_arch::Instant;

use sn_protocol::{
    messages::{CmdResponse, Query, Request, Response},
    storage::RecordType,
    NetworkAddress, PrettyPrintRecordKey,
};
use std::{
    collections::{hash_map::Entry, BTreeSet, HashSet},
    fmt::{Debug, Formatter},
};
use tokio::sync::oneshot;
use tokio::time::Duration;
use tracing::{info, warn};

/// Our agent string has as a prefix that we can match against.
const IDENTIFY_AGENT_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));

/// NodeEvent enum
#[derive(CustomDebug)]
pub(super) enum NodeEvent {
    MsgReceived(request_response::Event<Request, Response>),
    Kademlia(kad::Event),
    #[cfg(feature = "local-discovery")]
    Mdns(Box<mdns::Event>),
    Identify(Box<libp2p::identify::Event>),
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
    // Peer has been removed from the Routing Table. And the number of connected peers.
    PeerRemoved(PeerId, usize),
    /// The records bearing these keys are to be fetched from the holder or the network
    KeysToFetchForReplication(Vec<(PeerId, RecordKey)>),
    /// Started listening on a new address
    NewListenAddr(Multiaddr),
    /// Report unverified record
    UnverifiedRecord(Record),
    /// Terminate Node on HDD write erros
    TerminateNode,
    /// List of peer nodes that failed to fetch replication copy from.
    FailedToFetchHolders(BTreeSet<PeerId>),
    /// A peer in RT that supposed to be verified.
    BadNodeVerification {
        peer_id: PeerId,
    },
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
            NetworkEvent::TerminateNode => {
                write!(f, "NetworkEvent::TerminateNode")
            }
            NetworkEvent::FailedToFetchHolders(bad_nodes) => {
                write!(f, "NetworkEvent::FailedToFetchHolders({bad_nodes:?})")
            }
            NetworkEvent::BadNodeVerification { peer_id } => {
                write!(f, "NetworkEvent::BadNodeVerification({peer_id:?})")
            }
        }
    }
}

impl SwarmDriver {
    /// Handle `SwarmEvents`
    pub(super) fn handle_swarm_events(&mut self, event: SwarmEvent<NodeEvent>) -> Result<()> {
        let start = Instant::now();
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
                self.handle_kad_event(kad_event)?;
            }
            // Handle the Identify event from the libp2p swarm.
            SwarmEvent::Behaviour(NodeEvent::Identify(iden)) => {
                event_string = "identify";

                // Match on the Identify event.
                match *iden {
                    // If the event is a Received event, handle the received peer information.
                    libp2p::identify::Event::Received { peer_id, info } => {
                        trace!(%peer_id, ?info, "identify: received info");

                        let has_dialed = self.dialed_peers.contains(&peer_id);
                        let peer_is_agent = info
                            .agent_version
                            .starts_with(truncate_patch_version(IDENTIFY_AGENT_STR));

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

                        // When received an identify from un-dialed peer, try to dial it
                        // The dial shall trigger the same identify to be sent again and confirm
                        // peer is external accessable, hence safe to be added into RT.
                        if !self.local && peer_is_agent && !has_dialed {
                            // Only need to dial back for not fulfilled kbucket
                            let (kbucket_full, ilog2) = if let Some(kbucket) =
                                self.swarm.behaviour_mut().kademlia.kbucket(peer_id)
                            {
                                let ilog2 = kbucket.range().0.ilog2();
                                let num_peers = kbucket.num_entries();
                                let mut is_bucket_full = num_peers >= K_VALUE.into();

                                // If the bucket contains any of a bootstrap node,
                                // consider the bucket is not full and dial back
                                // so that the bootstrap nodes can be replaced.
                                if is_bucket_full {
                                    if let Some(peers) = self.bootstrap_peers.get(&ilog2) {
                                        if kbucket
                                            .iter()
                                            .any(|entry| peers.contains(entry.node.key.preimage()))
                                        {
                                            is_bucket_full = false;
                                        }
                                    }
                                }

                                (is_bucket_full, ilog2)
                            } else {
                                // Function will return `None` if the given key refers to self
                                // hence return true to skip further action.
                                (true, None)
                            };

                            if !kbucket_full {
                                info!(%peer_id, ?addrs, "received identify info from undialed peer for not full kbucket {:?}, dail back to confirm external accesable", ilog2);
                                self.dialed_peers
                                    .push(peer_id)
                                    .map_err(|_| NetworkError::CircularVecPopFrontError)?;
                                if let Err(err) = self.swarm.dial(
                                    DialOpts::peer_id(peer_id)
                                        .condition(PeerCondition::NotDialing)
                                        .addresses(addrs.iter().cloned().collect())
                                        .build(),
                                ) {
                                    warn!(%peer_id, ?addrs, "dialing error: {err:?}");
                                }
                            }

                            trace!(
                                "SwarmEvent handled in {:?}: {event_string:?}",
                                start.elapsed()
                            );
                            return Ok(());
                        }

                        // If we are not local, we care only for peers that we dialed and thus are reachable.
                        if self.local || has_dialed && peer_is_agent {
                            // only trigger the bad_node verification once have enough nodes in RT
                            // currently set the trigger bar at 100
                            let total_peers: usize = self
                                .swarm
                                .behaviour_mut()
                                .kademlia
                                .kbuckets()
                                .map(|kbucket| kbucket.num_entries())
                                .sum();

                            // To reduce the bad_node check resource usage,
                            // during the connection establish process, only check cached black_list
                            // The periodical check, which involves network queries shall filter
                            // out bad_nodes eventually.
                            if total_peers > 100 && self.bad_nodes.get(&peer_id).is_some() {
                                info!("Peer {peer_id:?} is considered as bad, blocking it.");
                            } else {
                                self.remove_bootstrap_from_full(peer_id);

                                trace!(%peer_id, ?addrs, "identify: attempting to add addresses to routing table");

                                // Attempt to add the addresses to the routing table.
                                for multiaddr in &addrs {
                                    let _routing_update = self
                                        .swarm
                                        .behaviour_mut()
                                        .kademlia
                                        .add_address(&peer_id, multiaddr.clone());
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
                // info!("{:?}", self.swarm.network_info());
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
                // info!(%peer_id, ?connection_id, "ConnectionEstablished {:?}", self.swarm.network_info());

                let _ = self.live_connected_peers.insert(
                    connection_id,
                    (peer_id, Instant::now() + Duration::from_secs(60)),
                );

                if endpoint.is_dialer() {
                    self.dialed_peers
                        .push(peer_id)
                        .map_err(|_| NetworkError::CircularVecPopFrontError)?;
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
                // info!(%peer_id, ?connection_id, "ConnectionClosed: {:?}", self.swarm.network_info());
                trace!(%peer_id, ?connection_id, ?cause, num_established, "ConnectionClosed: {}", endpoint_str(&endpoint));
                let _ = self.live_connected_peers.remove(&connection_id);
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(failed_peer_id),
                error,
                connection_id,
            } => {
                event_string = "OutgoingConnErr";
                warn!("OutgoingConnectionError to {failed_peer_id:?} on {connection_id:?} - {error:?}");

                // we need to decide if this was a critical error and the peer should be removed from the routing table
                let should_clean_peer = match error {
                    DialError::Transport(errors) => {
                        // as it's an outgoing error, if it's transport based we can assume it is _our_ fault
                        //
                        // (eg, could not get a port for a tcp connection)
                        // so we default to it not being a real issue
                        // unless there are _specific_ errors (connection refused eg)
                        error!("Dial errors len : {:?}", errors.len());
                        let mut there_is_a_serious_issue = false;
                        for (_addr, err) in errors {
                            error!("OutgoingTransport error : {err:?}");

                            match err {
                                TransportError::MultiaddrNotSupported(addr) => {
                                    warn!("Multiaddr not supported : {addr:?}");
                                    // if we can't dial a peer on a given address, we should remove it from the routing table
                                    there_is_a_serious_issue = true
                                }
                                TransportError::Other(err) => {
                                    let problematic_errors = [
                                        "ConnectionRefused",
                                        "HostUnreachable",
                                        "HandshakeTimedOut",
                                    ];
                                    // It is really difficult to match this error, due to being eg:
                                    // Custom { kind: Other, error: Left(Left(Os { code: 61, kind: ConnectionRefused, message: "Connection refused" })) }
                                    // if we can match that, let's. But meanwhile we'll check the message
                                    let error_msg = format!("{err:?}");
                                    if problematic_errors.iter().any(|err| error_msg.contains(err))
                                    {
                                        warn!("Problematic error encountered: {error_msg}");
                                        there_is_a_serious_issue = true;
                                    }
                                }
                            }
                        }
                        there_is_a_serious_issue
                    }
                    DialError::NoAddresses => {
                        // We provided no address, and while we can't really blame the peer
                        // we also can't connect, so we opt to cleanup...
                        warn!("OutgoingConnectionError: No address provided");
                        true
                    }
                    DialError::Aborted => {
                        // not their fault
                        warn!("OutgoingConnectionError: Aborted");
                        false
                    }
                    DialError::DialPeerConditionFalse(_) => {
                        // we could not dial due to an internal condition, so not their issue
                        warn!("OutgoingConnectionError: DialPeerConditionFalse");
                        false
                    }
                    DialError::LocalPeerId { endpoint, .. } => {
                        // This is actually _us_ So we should remove this from the RT
                        error!(
                            "OutgoingConnectionError: LocalPeerId: {}",
                            endpoint_str(&endpoint)
                        );
                        true
                    }
                    DialError::WrongPeerId { obtained, endpoint } => {
                        // The peer id we attempted to dial was not the one we expected
                        // cleanup
                        error!("OutgoingConnectionError: WrongPeerId: obtained: {obtained:?}, endpoint: {endpoint:?}");
                        true
                    }
                    DialError::Denied { cause } => {
                        // The peer denied our connection
                        // cleanup
                        error!("OutgoingConnectionError: Denied: {cause:?}");
                        true
                    }
                };

                if should_clean_peer {
                    warn!("Cleaning out peer {failed_peer_id:?}");
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
            }
            SwarmEvent::IncomingConnectionError {
                connection_id,
                local_addr,
                send_back_addr,
                error,
            } => {
                // info!("{:?}", self.swarm.network_info());
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
            SwarmEvent::NewExternalAddrCandidate { address } => {
                event_string = "NewExternalAddrCandidate";

                if !self.swarm.external_addresses().any(|addr| addr == &address) && !self.is_client
                {
                    info!(%address, "external address: new candidate");

                    // Identify will let us know when we have a candidate. (Peers will tell us what address they see us as.)
                    // We manually confirm this to be our externally reachable address, though in theory it's possible we
                    // are not actually reachable. (Peers can lie to us.) This is a good enough heuristic for now.
                    // Setting this will also switch kad to server mode if it's not already in it.
                    self.swarm.add_external_address(address);
                }
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                event_string = "ExternalAddrConfirmed";
                info!(%address, "external address: confirmed");
            }
            SwarmEvent::ExternalAddrExpired { address } => {
                event_string = "ExternalAddrExpired";
                info!(%address, "external address: expired");
            }
            other => {
                event_string = "Other";

                trace!("SwarmEvent has been ignored: {other:?}")
            }
        }

        self.remove_outdated_connections();

        self.log_handling(event_string.to_string(), start.elapsed());

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
    ) -> Result<(), NetworkError> {
        match event {
            request_response::Event::Message { message, peer } => match message {
                Message::Request {
                    request,
                    channel,
                    request_id,
                    ..
                } => {
                    trace!("Received request {request_id:?} from peer {peer:?}, req: {request:?}");
                    // if the request is replication, we can handle it and send the OK response here,
                    // as we send that regardless of how we handle the request as its unimportant to the sender.
                    match request {
                        Request::Cmd(sn_protocol::messages::Cmd::Replicate { holder, keys }) => {
                            let response = Response::Cmd(
                                sn_protocol::messages::CmdResponse::Replicate(Ok(())),
                            );
                            self.swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, response)
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?;

                            self.add_keys_to_replication_fetcher(holder, keys);
                        }
                        Request::Query(query) => {
                            self.send_event(NetworkEvent::QueryRequestReceived {
                                query,
                                channel: MsgResponder::FromPeer(channel),
                            })
                        }
                    }
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
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?,
                            None => {
                                if let Response::Cmd(CmdResponse::Replicate(Ok(()))) = response {
                                    // Nothing to do, response was fine
                                    // This only exists to ensure we dont drop the handle and
                                    // exit early, potentially logging false connection woes
                                } else {
                                    // responses that are not awaited at the call site must be handled
                                    // separately
                                    self.send_event(NetworkEvent::ResponseReceived {
                                        res: response,
                                    });
                                }
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
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?;
                        }
                        None => {
                            warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                            return Err(NetworkError::ReceivedResponseDropped(request_id));
                        }
                    }
                } else {
                    warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                    return Err(NetworkError::ReceivedResponseDropped(request_id));
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
        let start = Instant::now();
        let event_string;

        match kad_event {
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetClosestPeers(Ok(ref closest_peers)),
                ref stats,
                ref step,
            } => {
                event_string = "kad_event::get_closest_peers";
                trace!(
                    "Query task {id:?} of key {:?} returned with peers {:?}, {stats:?} - {step:?}",
                    hex::encode(closest_peers.key.clone()),
                    closest_peers.peers,
                );

                if let Entry::Occupied(mut entry) = self.pending_get_closest_peers.entry(id) {
                    let (_, current_closest) = entry.get_mut();

                    // TODO: consider order the result and terminate when reach any of the
                    //       following criteria:
                    //   1, `stats.num_pending()` is 0
                    //   2, `stats.duration()` is longer than a defined period
                    current_closest.extend(closest_peers.peers.clone());
                    if current_closest.len() >= usize::from(K_VALUE) || step.last {
                        let (get_closest_type, current_closest) = entry.remove();
                        match get_closest_type {
                            PendingGetClosestType::NetworkDiscovery => self
                                .network_discovery
                                .handle_get_closest_query(current_closest),
                            PendingGetClosestType::FunctionCall(sender) => {
                                sender
                                    .send(current_closest)
                                    .map_err(|_| NetworkError::InternalMsgChannelDropped)?;
                            }
                        }
                    }
                } else {
                    trace!("Can't locate query task {id:?}, it has likely been completed already.");
                    return Err(NetworkError::ReceivedKademliaEventDropped {
                        query_id: id,
                        event: "GetClosestPeers Ok".to_string(),
                    });
                }
            }
            // Handle GetClosestPeers timeouts
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetClosestPeers(Err(ref err)),
                ref stats,
                ref step,
            } => {
                event_string = "kad_event::get_closest_peers_err";
                error!("GetClosest Query task {id:?} errored with {err:?}, {stats:?} - {step:?}");

                let (get_closest_type, mut current_closest) =
                    self.pending_get_closest_peers.remove(&id).ok_or_else(|| {
                        trace!(
                            "Can't locate query task {id:?}, it has likely been completed already."
                        );
                        NetworkError::ReceivedKademliaEventDropped {
                            query_id: id,
                            event: "Get ClosestPeers error".to_string(),
                        }
                    })?;

                // We have `current_closest` from previous progress,
                // and `peers` from `GetClosestPeersError`.
                // Trust them and leave for the caller to check whether they are enough.
                match err {
                    GetClosestPeersError::Timeout { ref peers, .. } => {
                        current_closest.extend(peers);
                    }
                }

                match get_closest_type {
                    PendingGetClosestType::NetworkDiscovery => self
                        .network_discovery
                        .handle_get_closest_query(current_closest),
                    PendingGetClosestType::FunctionCall(sender) => {
                        sender
                            .send(current_closest)
                            .map_err(|_| NetworkError::InternalMsgChannelDropped)?;
                    }
                }
            }

            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
                stats,
                step,
            } => {
                event_string = "kad_event::get_record::found";
                trace!(
                    "Query task {id:?} returned with record {:?} from peer {:?}, {stats:?} - {step:?}",
                    PrettyPrintRecordKey::from(&peer_record.record.key),
                    peer_record.peer
                );
                self.accumulate_get_record_found(id, peer_record, stats, step)?;
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    QueryResult::GetRecord(Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord {
                        cache_candidates,
                    })),
                stats,
                step,
            } => {
                event_string = "kad_event::get_record::finished_no_additional";
                trace!("Query task {id:?} of get_record completed with {stats:?} - {step:?} - {cache_candidates:?}");
                self.handle_get_record_finished(id, step)?;
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetRecord(Err(get_record_err)),
                stats,
                step,
            } => {
                // log the errors
                match &get_record_err {
                    kad::GetRecordError::NotFound { key, closest_peers } => {
                        event_string = "kad_event::GetRecordError::NotFound";
                        info!("Query task {id:?} NotFound record {:?} among peers {closest_peers:?}, {stats:?} - {step:?}",
                        PrettyPrintRecordKey::from(key));
                    }
                    kad::GetRecordError::QuorumFailed {
                        key,
                        records,
                        quorum,
                    } => {
                        event_string = "kad_event::GetRecordError::QuorumFailed";
                        let pretty_key = PrettyPrintRecordKey::from(key);
                        let peers = records
                            .iter()
                            .map(|peer_record| peer_record.peer)
                            .collect_vec();
                        info!("Query task {id:?} QuorumFailed record {pretty_key:?} among peers {peers:?} with quorum {quorum:?}, {stats:?} - {step:?}");
                    }
                    kad::GetRecordError::Timeout { key } => {
                        event_string = "kad_event::GetRecordError::Timeout";
                        let pretty_key = PrettyPrintRecordKey::from(key);

                        debug!(
                            "Query task {id:?} timed out when looking for record {pretty_key:?}"
                        );
                    }
                }
                self.handle_get_record_error(id, get_record_err, stats, step)?;
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::PutRecord(Err(put_record_err)),
                stats,
                step,
            } => {
                // Currently, only `client` calls `put_record_to` to upload data.
                // The result of such operation is not critical to client in general.
                // However, if client keeps receiving error responses, it may indicating:
                //   1, Client itself is with slow connection
                //   OR
                //   2, The payee node selected could be in trouble
                //
                // TODO: Figure out which payee node the error response is related to,
                //       and may exclude that node from later on payee selection.
                let (key, success, quorum) = match &put_record_err {
                    kad::PutRecordError::QuorumFailed {
                        key,
                        success,
                        quorum,
                    } => {
                        event_string = "kad_event::PutRecordError::QuorumFailed";
                        (key, success, quorum)
                    }
                    kad::PutRecordError::Timeout {
                        key,
                        success,
                        quorum,
                    } => {
                        event_string = "kad_event::PutRecordError::Timeout";
                        (key, success, quorum)
                    }
                };
                error!("Query task {id:?} failed put record {:?} {:?}, required quorum {quorum}, stored on {success:?}, {stats:?} - {step:?}",
                       PrettyPrintRecordKey::from(key), event_string);
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::PutRecord(Ok(put_record_ok)),
                stats,
                step,
            } => {
                event_string = "kad_event::PutRecordOk";
                trace!(
                    "Query task {id:?} put record {:?} ok, {stats:?} - {step:?}",
                    PrettyPrintRecordKey::from(&put_record_ok.key)
                );
            }
            // Shall no longer receive this event
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

                    // This should only happen once
                    if self.bootstrap.notify_new_peer() {
                        info!("Performing the first bootstrap");
                        self.trigger_network_discovery();
                    }
                    self.send_event(NetworkEvent::PeerAdded(peer, self.connected_peers));
                }

                info!("kad_event::RoutingUpdated {:?}: {peer:?}, is_new_peer: {is_new_peer:?} old_peer: {old_peer:?}", self.connected_peers);
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

        self.log_handling(event_string.to_string(), start.elapsed());

        trace!(
            "kad::Event handled in {:?}: {event_string:?}",
            start.elapsed()
        );

        Ok(())
    }

    /// Check for changes in our close group
    ///
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

    // if target bucket is full, remove a bootstrap node if presents.
    fn remove_bootstrap_from_full(&mut self, peer_id: PeerId) {
        let mut shall_removed = None;

        if let Some(kbucket) = self.swarm.behaviour_mut().kademlia.kbucket(peer_id) {
            if kbucket.num_entries() >= K_VALUE.into() {
                if let Some(peers) = self.bootstrap_peers.get(&kbucket.range().0.ilog2()) {
                    for peer_entry in kbucket.iter() {
                        if peers.contains(peer_entry.node.key.preimage()) {
                            shall_removed = Some(*peer_entry.node.key.preimage());
                            break;
                        }
                    }
                }
            }
        }
        if let Some(to_be_removed_bootstrap) = shall_removed {
            trace!("Bootstrap node {to_be_removed_bootstrap:?} to be replaced by peer {peer_id:?}");
            let _entry = self
                .swarm
                .behaviour_mut()
                .kademlia
                .remove_peer(&to_be_removed_bootstrap);
        }
    }

    // Remove outdated connection to a peer if it is not in the RT.
    fn remove_outdated_connections(&mut self) {
        let mut shall_removed = vec![];

        self.live_connected_peers
            .retain(|connection_id, (peer_id, timeout)| {
                let shall_retained = *timeout > Instant::now();
                if !shall_retained {
                    shall_removed.push((*connection_id, *peer_id))
                }
                shall_retained
            });

        if !shall_removed.is_empty() {
            trace!(
                "Current libp2p peers pool stats is {:?}",
                self.swarm.network_info()
            );
            trace!(
                "Removing {} outdated live connections, still have {} left.",
                shall_removed.len(),
                self.live_connected_peers.len()
            );
        }

        // Only remove outdated peer not in the RT
        for (connection_id, peer_id) in shall_removed {
            if let Some(kbucket) = self.swarm.behaviour_mut().kademlia.kbucket(peer_id) {
                if kbucket
                    .iter()
                    .any(|peer_entry| peer_id == *peer_entry.node.key.preimage())
                {
                    // Skip the connection as peer presents in the RT.
                    continue;
                }
            }

            trace!("Removing outdated connection {connection_id:?} to {peer_id:?}");
            let _result = self.swarm.close_connection(connection_id);
        }
    }

    fn add_keys_to_replication_fetcher(
        &mut self,
        sender: NetworkAddress,
        incoming_keys: Vec<(NetworkAddress, RecordType)>,
    ) {
        let holder = if let Some(peer_id) = sender.as_peer_id() {
            peer_id
        } else {
            warn!("Replication list sender is not a peer_id {sender:?}");
            return;
        };

        trace!(
            "Received replication list from {holder:?} of {} keys",
            incoming_keys.len()
        );

        // accept replication requests from the K_VALUE peers away,
        // giving us some margin for replication
        let closest_k_peers = self.get_closest_k_value_local_peers();
        if !closest_k_peers.contains(&holder) || holder == self.self_peer_id {
            trace!("Holder {holder:?} is self or not in replication range.");
            return;
        }

        // Only handle those non-exist and in close range keys
        let keys_to_store =
            self.select_non_existent_records_for_replications(&incoming_keys, &closest_k_peers);
        if keys_to_store.is_empty() {
            debug!("Empty keys to store after adding to");
            return;
        }

        #[allow(clippy::mutable_key_type)]
        let all_keys = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .record_addresses_ref();
        let keys_to_fetch = self
            .replication_fetcher
            .add_keys(holder, keys_to_store, all_keys);
        if !keys_to_fetch.is_empty() {
            self.send_event(NetworkEvent::KeysToFetchForReplication(keys_to_fetch));
        } else {
            trace!("no waiting keys to fetch from the network");
        }
    }

    /// Checks suggested records against what we hold, so we only
    /// enqueue what we do not have
    fn select_non_existent_records_for_replications(
        &mut self,
        incoming_keys: &[(NetworkAddress, RecordType)],
        closest_k_peers: &Vec<PeerId>,
    ) -> Vec<(NetworkAddress, RecordType)> {
        #[allow(clippy::mutable_key_type)]
        let locally_stored_keys = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .record_addresses_ref();
        let non_existent_keys: Vec<_> = incoming_keys
            .iter()
            .filter(|(addr, record_type)| {
                let key = addr.to_record_key();
                let local = locally_stored_keys.get(&key);

                // if we have a local value of matching record_type, we don't need to fetch it
                if let Some((_, local_record_type)) = local {
                    local_record_type != record_type
                } else {
                    true
                }
            })
            .collect();

        non_existent_keys
            .into_iter()
            .filter_map(|(key, record_type)| {
                if self.is_in_close_range(key, closest_k_peers) {
                    Some((key.clone(), record_type.clone()))
                } else {
                    // Reduce the log level as there will always be around 40% records being
                    // out of the close range, as the sender side is using `CLOSE_GROUP_SIZE + 2`
                    // to send our replication list to provide addressing margin.
                    // Given there will normally be 6 nodes sending such list with interval of 5-10s,
                    // this will accumulate to a lot of logs with the increasing records uploaded.
                    trace!("not in close range for key {key:?}");
                    None
                }
            })
            .collect()
    }

    // A close target doesn't falls into the close peers range:
    // For example, a node b11111X has an RT: [(1, b1111), (2, b111), (5, b11), (9, b1), (7, b0)]
    // Then for a target bearing b011111 as prefix, all nodes in (7, b0) are its close_group peers.
    // Then the node b11111X. But b11111X's close_group peers [(1, b1111), (2, b111), (5, b11)]
    // are none among target b011111's close range.
    // Hence, the ilog2 calculation based on close_range cannot cover such case.
    // And have to sort all nodes to figure out whether self is among the close_group to the target.
    fn is_in_close_range(&self, target: &NetworkAddress, all_peers: &Vec<PeerId>) -> bool {
        if all_peers.len() <= REPLICATE_RANGE {
            return true;
        }

        // Margin of 2 to allow our RT being bit lagging.
        match sort_peers_by_address(all_peers, target, REPLICATE_RANGE) {
            Ok(close_group) => close_group.contains(&&self.self_peer_id),
            Err(err) => {
                warn!("Could not get sorted peers for {target:?} with error {err:?}");
                true
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
