// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    cmd::SwarmCmd, event::NodeEvent, multiaddr_is_global, multiaddr_strip_p2p,
    target_arch::Instant, NetworkEvent, Result, SwarmDriver,
};
use itertools::Itertools;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    kad::K_VALUE,
    multiaddr::Protocol,
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        DialError, SwarmEvent,
    },
    Multiaddr, PeerId, TransportError,
};
use sn_protocol::{
    get_port_from_multiaddr,
    version::{IDENTIFY_NODE_VERSION_STR, IDENTIFY_PROTOCOL_STR},
};
use std::collections::HashSet;
use tokio::time::Duration;

impl SwarmDriver {
    /// Handle `SwarmEvents`
    pub(crate) fn handle_swarm_events(&mut self, event: SwarmEvent<NodeEvent>) -> Result<()> {
        let start = Instant::now();
        let event_string;
        match event {
            SwarmEvent::Behaviour(NodeEvent::MsgReceived(event)) => {
                event_string = "msg_received";
                if let Err(e) = self.handle_req_resp_events(event) {
                    warn!("MsgReceivedError: {e:?}");
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Kademlia(kad_event)) => {
                event_string = "kad_event";
                self.handle_kad_event(kad_event)?;
            }
            SwarmEvent::Behaviour(NodeEvent::Dcutr(event)) => {
                event_string = "dcutr_event";
                info!(
                    "Dcutr with remote peer: {:?} is: {:?}",
                    event.remote_peer_id, event.result
                );
            }
            SwarmEvent::Behaviour(NodeEvent::RelayClient(event)) => {
                event_string = "relay_client_event";

                info!(?event, "relay client event");

                if let libp2p::relay::client::Event::ReservationReqAccepted {
                    relay_peer_id, ..
                } = *event
                {
                    self.relay_manager
                        .on_successful_reservation_by_client(&relay_peer_id, &mut self.swarm);
                }
            }

            SwarmEvent::Behaviour(NodeEvent::RelayServer(event)) => {
                event_string = "relay_server_event";

                info!(?event, "relay server event");

                match *event {
                    libp2p::relay::Event::ReservationReqAccepted {
                        src_peer_id,
                        renewed: _,
                    } => {
                        self.relay_manager
                            .on_successful_reservation_by_server(src_peer_id);
                    }
                    libp2p::relay::Event::ReservationTimedOut { src_peer_id } => {
                        self.relay_manager.on_reservation_timeout(src_peer_id);
                    }
                    _ => {}
                }
            }
            SwarmEvent::Behaviour(NodeEvent::Identify(iden)) => {
                event_string = "identify";

                match *iden {
                    libp2p::identify::Event::Received { peer_id, info } => {
                        trace!(%peer_id, ?info, "identify: received info");

                        if info.protocol_version != IDENTIFY_PROTOCOL_STR.to_string() {
                            warn!(?info.protocol_version, "identify: {peer_id:?} does not have the same protocol. Our IDENTIFY_PROTOCOL_STR: {:?}", IDENTIFY_PROTOCOL_STR.as_str());

                            self.send_event(NetworkEvent::PeerWithUnsupportedProtocol {
                                our_protocol: IDENTIFY_PROTOCOL_STR.to_string(),
                                their_protocol: info.protocol_version,
                            });

                            return Ok(());
                        }

                        // if client, return.
                        if info.agent_version != IDENTIFY_NODE_VERSION_STR.to_string() {
                            return Ok(());
                        }

                        let has_dialed = self.dialed_peers.contains(&peer_id);

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

                        self.relay_manager.add_potential_candidates(
                            &peer_id,
                            &addrs,
                            &info.protocols,
                        );

                        // When received an identify from un-dialed peer, try to dial it
                        // The dial shall trigger the same identify to be sent again and confirm
                        // peer is external accessible, hence safe to be added into RT.
                        if !self.local && !has_dialed {
                            // Only need to dial back for not fulfilled kbucket
                            let (kbucket_full, already_present_in_rt, ilog2) =
                                if let Some(kbucket) =
                                    self.swarm.behaviour_mut().kademlia.kbucket(peer_id)
                                {
                                    let ilog2 = kbucket.range().0.ilog2();
                                    let num_peers = kbucket.num_entries();
                                    let mut is_bucket_full = num_peers >= K_VALUE.into();

                                    // check if peer_id is already a part of RT
                                    let already_present_in_rt = kbucket
                                        .iter()
                                        .any(|entry| entry.node.key.preimage() == &peer_id);

                                    // If the bucket contains any of a bootstrap node,
                                    // consider the bucket is not full and dial back
                                    // so that the bootstrap nodes can be replaced.
                                    if is_bucket_full {
                                        if let Some(peers) = self.bootstrap_peers.get(&ilog2) {
                                            if kbucket.iter().any(|entry| {
                                                peers.contains(entry.node.key.preimage())
                                            }) {
                                                is_bucket_full = false;
                                            }
                                        }
                                    }

                                    (is_bucket_full, already_present_in_rt, ilog2)
                                } else {
                                    return Ok(());
                                };

                            if kbucket_full {
                                trace!("received identify for a full bucket {ilog2:?}, not dialing {peer_id:?} on {addrs:?}");
                                return Ok(());
                            } else if already_present_in_rt {
                                trace!("received identify for {peer_id:?} that is already part of the RT. Not dialing {peer_id:?} on {addrs:?}");
                                return Ok(());
                            }

                            info!(%peer_id, ?addrs, "received identify info from undialed peer for not full kbucket {ilog2:?}, dial back to confirm external accessible");
                            if let Err(err) = self.swarm.dial(
                                DialOpts::peer_id(peer_id)
                                    .condition(PeerCondition::NotDialing)
                                    .addresses(addrs.iter().cloned().collect())
                                    .build(),
                            ) {
                                warn!(%peer_id, ?addrs, "dialing error: {err:?}");
                            }

                            trace!(
                                "SwarmEvent handled in {:?}: {event_string:?}",
                                start.elapsed()
                            );
                            return Ok(());
                        }

                        // If we are not local, we care only for peers that we dialed and thus are reachable.
                        if self.local || has_dialed {
                            // To reduce the bad_node check resource usage,
                            // during the connection establish process, only check cached black_list
                            // The periodical check, which involves network queries shall filter
                            // out bad_nodes eventually.
                            if let Some((_issues, true)) = self.bad_nodes.get(&peer_id) {
                                info!("Peer {peer_id:?} is considered as bad, blocking it.");
                            } else {
                                self.remove_bootstrap_from_full(peer_id);

                                trace!(%peer_id, ?addrs, "identify: attempting to add addresses to routing table");

                                // Attempt to add the addresses to the routing table.
                                for multiaddr in addrs {
                                    let _routing_update = self
                                        .swarm
                                        .behaviour_mut()
                                        .kademlia
                                        .add_address(&peer_id, multiaddr);
                                }
                            }
                        }
                        trace!(
                            "SwarmEvent handled in {:?}: {event_string:?}",
                            start.elapsed()
                        );
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

            SwarmEvent::NewListenAddr {
                address,
                listener_id,
            } => {
                event_string = "new listen addr";

                // update our stored port if it is configured to be 0 or None
                match self.listen_port {
                    Some(0) | None => {
                        if let Some(actual_port) = get_port_from_multiaddr(&address) {
                            info!("Our listen port is configured as 0 or is not set. Setting it to our actual port: {actual_port}");
                            self.listen_port = Some(actual_port);
                        }
                    }
                    _ => {}
                };

                let local_peer_id = *self.swarm.local_peer_id();
                let address = address.with(Protocol::P2p(local_peer_id));

                // Trigger server mode if we're not a client and we should not add our own address if we're behind
                // home network.
                if !self.is_client && !self.is_behind_home_network {
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

                info!("Local node is listening {listener_id:?} on {address:?}");
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                event_string = "listener closed";
                info!("Listener {listener_id:?} with add {addresses:?} has been closed for {reason:?}");
                self.relay_manager
                    .on_listener_closed(&listener_id, &mut self.swarm);
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
                concurrent_dial_errors,
                established_in,
            } => {
                event_string = "ConnectionEstablished";
                trace!(%peer_id, num_established, ?concurrent_dial_errors, "ConnectionEstablished ({connection_id:?}) in {established_in:?}: {}", endpoint_str(&endpoint));

                let _ = self.live_connected_peers.insert(
                    connection_id,
                    (peer_id, Instant::now() + Duration::from_secs(60)),
                );

                if endpoint.is_dialer() {
                    self.dialed_peers.push(peer_id);
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
                let _ = self.live_connected_peers.remove(&connection_id);
            }
            SwarmEvent::OutgoingConnectionError {
                connection_id,
                peer_id: None,
                error,
            } => {
                event_string = "OutgoingConnErr";
                warn!("OutgoingConnectionError to on {connection_id:?} - {error:?}");
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

                                    let is_bootstrap_peer = self
                                        .bootstrap_peers
                                        .iter()
                                        .any(|(_ilog2, peers)| peers.contains(&failed_peer_id));

                                    if is_bootstrap_peer
                                        && self.connected_peers < self.bootstrap_peers.len()
                                    {
                                        warn!("OutgoingConnectionError: On bootstrap peer {failed_peer_id:?}, while still in bootstrap mode, ignoring");
                                        there_is_a_serious_issue = false;
                                    } else {
                                        // It is really difficult to match this error, due to being eg:
                                        // Custom { kind: Other, error: Left(Left(Os { code: 61, kind: ConnectionRefused, message: "Connection refused" })) }
                                        // if we can match that, let's. But meanwhile we'll check the message
                                        let error_msg = format!("{err:?}");
                                        if problematic_errors
                                            .iter()
                                            .any(|err| error_msg.contains(err))
                                        {
                                            warn!("Problematic error encountered: {error_msg}");
                                            there_is_a_serious_issue = true;
                                        }
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
                    warn!("Tracking issue of {failed_peer_id:?}. Clearing it out for now");

                    if let Some(dead_peer) = self
                        .swarm
                        .behaviour_mut()
                        .kademlia
                        .remove_peer(&failed_peer_id)
                    {
                        self.connected_peers = self.connected_peers.saturating_sub(1);

                        self.handle_cmd(SwarmCmd::RecordNodeIssue {
                            peer_id: failed_peer_id,
                            issue: crate::NodeIssue::ConnectionIssue,
                        })?;

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

                if !self.swarm.external_addresses().any(|addr| addr == &address)
                    && !self.is_client
                    // If we are behind a home network, then our IP is returned here. We should be only having
                    // relay server as our external address
                    // todo: can our relay address be reported here? If so, maybe we should add them.
                    && !self.is_behind_home_network
                {
                    debug!(%address, "external address: new candidate");

                    // Identify will let us know when we have a candidate. (Peers will tell us what address they see us as.)
                    // We manually confirm this to be our externally reachable address, though in theory it's possible we
                    // are not actually reachable. This event returns addresses with ports that were not set by the user,
                    // so we must not add those ports as they will not be forwarded.
                    // Setting this will also switch kad to server mode if it's not already in it.
                    if let Some(our_port) = self.listen_port {
                        if let Some(port) = get_port_from_multiaddr(&address) {
                            if port == our_port {
                                info!(%address, "external address: new candidate has the same configured port, adding it.");
                                self.swarm.add_external_address(address);
                            } else {
                                info!(%address, %our_port, "external address: new candidate has a different port, not adding it.");
                            }
                        }
                    } else {
                        trace!("external address: listen port not set. This has to be set if you're running a node");
                    }
                }
                let all_external_addresses = self.swarm.external_addresses().collect_vec();
                let all_listeners = self.swarm.listeners().collect_vec();
                debug!("All our listeners: {all_listeners:?}");
                debug!("All our external addresses: {all_external_addresses:?}");
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

        let timed_out_connections =
            self.live_connected_peers
                .iter()
                .filter_map(|(connection_id, (peer_id, timeout))| {
                    if Instant::now() > *timeout {
                        Some((connection_id, peer_id))
                    } else {
                        None
                    }
                });

        for (connection_id, peer_id) in timed_out_connections {
            // Skip if the peer is present in our RT
            if let Some(kbucket) = self.swarm.behaviour_mut().kademlia.kbucket(*peer_id) {
                if kbucket
                    .iter()
                    .any(|peer_entry| *peer_id == *peer_entry.node.key.preimage())
                {
                    continue;
                }
            }

            // skip if the peer is a relay server that we're connected to
            if self.relay_manager.keep_alive_peer(peer_id) {
                continue;
            }

            shall_removed.push((*connection_id, *peer_id));
        }

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
            trace!(?self.relay_manager);

            for (connection_id, peer_id) in shall_removed {
                let _ = self.live_connected_peers.remove(&connection_id);
                let result = self.swarm.close_connection(connection_id);
                trace!("Removed outdated connection {connection_id:?} to {peer_id:?} with result: {result:?}");
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
