// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::driver::NodeBehaviour;
use libp2p::{
    core::transport::ListenerId, multiaddr::Protocol, Multiaddr, PeerId, StreamProtocol, Swarm,
};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

const MAX_CONCURRENT_RELAY_CONNECTIONS: usize = 3;
const MAX_POTENTIAL_CANDIDATES: usize = 15;

/// To manager relayed connections.
#[derive(Debug)]
pub(crate) struct RelayManager {
    self_peer_id: PeerId,
    // states
    enabled: bool,
    candidates: VecDeque<(PeerId, Multiaddr)>,
    waiting_for_reservation: BTreeMap<PeerId, Multiaddr>,
    connected_relays: BTreeMap<PeerId, Multiaddr>,

    /// Tracker for the relayed listen addresses.
    relayed_listener_id_map: HashMap<ListenerId, PeerId>,
}

impl RelayManager {
    pub(crate) fn new(initial_peers: Vec<Multiaddr>, self_peer_id: PeerId) -> Self {
        let candidates = initial_peers
            .into_iter()
            .filter_map(|addr| {
                for protocol in addr.iter() {
                    if let Protocol::P2p(peer_id) = protocol {
                        let relay_addr = Self::craft_relay_address(&addr, Some(peer_id))?;

                        return Some((peer_id, relay_addr));
                    }
                }
                None
            })
            .collect();
        Self {
            self_peer_id,
            enabled: false,
            connected_relays: Default::default(),
            waiting_for_reservation: Default::default(),
            candidates,
            relayed_listener_id_map: Default::default(),
        }
    }

    pub(crate) fn enable_hole_punching(&mut self, enable: bool) {
        info!("Setting enable hole punching to {enable:?}");
        self.enabled = enable;
    }

    pub(crate) fn keep_alive_peer(&self, peer_id: &PeerId) -> bool {
        self.connected_relays.contains_key(peer_id)
            || self.waiting_for_reservation.contains_key(peer_id)
    }

    /// Add a potential candidate to the list if it satisfies all the identify checks and also supports the relay server
    /// protocol.
    pub(crate) fn add_potential_candidates(
        &mut self,
        peer_id: &PeerId,
        addrs: &HashSet<Multiaddr>,
        stream_protocols: &Vec<StreamProtocol>,
    ) {
        trace!("Trying to add potential relay candidates for {peer_id:?} with addrs: {addrs:?}");
        if self.candidates.len() >= MAX_POTENTIAL_CANDIDATES {
            trace!("Got max relay candidates");
            return;
        }

        if Self::does_it_support_relay_server_protocol(stream_protocols) {
            // todo: collect and manage multiple addrs
            if let Some(addr) = addrs.iter().next() {
                // only consider non relayed peers
                if !addr.iter().any(|p| p == Protocol::P2pCircuit) {
                    if let Some(relay_addr) = Self::craft_relay_address(addr, Some(*peer_id)) {
                        debug!(
                            "Adding {peer_id:?} with {relay_addr:?} as a potential relay candidate"
                        );
                        self.candidates.push_back((*peer_id, relay_addr));
                    } else {
                        trace!("Was not able to craft relay address");
                    }
                } else {
                    trace!("Addr contains P2pCircuit protocol. Not adding as candidate.");
                }
            }
        } else {
            trace!("Peer does not support relay server protocol");
        }
    }

    // todo: how do we know if a reservation has been revoked / if the peer has gone offline?
    /// Try connecting to candidate relays if we are below the threshold connections.
    /// This is run periodically on a loop.
    pub(crate) fn try_connecting_to_relay(&mut self, swarm: &mut Swarm<NodeBehaviour>) {
        if !self.enabled {
            return;
        }

        if self.connected_relays.len() >= MAX_CONCURRENT_RELAY_CONNECTIONS
            || self.candidates.is_empty()
        {
            return;
        }

        let reservations_to_make = MAX_CONCURRENT_RELAY_CONNECTIONS - self.connected_relays.len();
        let mut n_reservations = 0;

        while n_reservations < reservations_to_make {
            // todo: should we remove all our other `listen_addr`? And should we block from adding `add_external_address` if
            // we're behind nat?
            if let Some((peer_id, relay_addr)) = self.candidates.pop_front() {
                if self.connected_relays.contains_key(&peer_id)
                    || self.waiting_for_reservation.contains_key(&peer_id)
                {
                    trace!("We are already using {peer_id:?} as a relay server. Skipping.");
                    continue;
                }

                match swarm.listen_on(relay_addr.clone()) {
                    Ok(id) => {
                        info!("Sending reservation to relay {peer_id:?} on {relay_addr:?}");
                        self.waiting_for_reservation.insert(peer_id, relay_addr);
                        self.relayed_listener_id_map.insert(id, peer_id);
                        n_reservations += 1;
                    }
                    Err(err) => {
                        error!("Error while trying to listen on the relay addr: {err:?} on {relay_addr:?}");
                    }
                }
            } else {
                trace!("No more relay candidates.");
                break;
            }
        }
    }

    /// Update our state after we've successfully made reservation with a relay.
    pub(crate) fn update_on_successful_reservation(
        &mut self,
        peer_id: &PeerId,
        swarm: &mut Swarm<NodeBehaviour>,
    ) {
        match self.waiting_for_reservation.remove(peer_id) {
            Some(addr) => {
                info!("Successfully made reservation with {peer_id:?} on {addr:?}. Adding the addr to external address.");
                swarm.add_external_address(addr.clone());
                self.connected_relays.insert(*peer_id, addr);
            }
            None => {
                debug!("Made a reservation with a peer that we had not requested to");
            }
        }
    }

    /// Update our state if the reservation has been cancelled or if the relay has closed.
    pub(crate) fn update_on_listener_closed(
        &mut self,
        listener_id: &ListenerId,
        swarm: &mut Swarm<NodeBehaviour>,
    ) {
        let Some(peer_id) = self.relayed_listener_id_map.remove(listener_id) else {
            return;
        };

        if let Some(addr) = self.connected_relays.remove(&peer_id) {
            info!("Removing connected relay server as the listener has been closed: {peer_id:?}");
            info!("Removing external addr: {addr:?}");
            swarm.remove_external_address(&addr);

            // Even though we craft and store addrs in this format /ip4/198.51.100.0/tcp/55555/p2p/QmRelay/p2p-circuit/,
            // sometimes our PeerId is added at the end by the swarm?, which we want to remove as well i.e.,
            // /ip4/198.51.100.0/tcp/55555/p2p/QmRelay/p2p-circuit/p2p/QmSelf
            let Ok(addr_with_self_peer_id) = addr.with_p2p(self.self_peer_id) else {
                return;
            };
            info!("Removing external addr: {addr_with_self_peer_id:?}");
            swarm.remove_external_address(&addr_with_self_peer_id);
        } else if let Some(addr) = self.waiting_for_reservation.remove(&peer_id) {
            info!("Removed peer form waiting_for_reservation as the listener has been closed {peer_id:?}: {addr:?}");
        } else {
            warn!("Could not find the listen addr after making reservation to the same");
        }
    }

    fn does_it_support_relay_server_protocol(protocols: &Vec<StreamProtocol>) -> bool {
        for stream_protocol in protocols {
            if *stream_protocol == "/libp2p/circuit/relay/0.2.0/stop" {
                return true;
            }
        }
        false
    }

    /// The listen addr should be something like /ip4/198.51.100.0/tcp/55555/p2p/QmRelay/p2p-circuit/
    fn craft_relay_address(addr: &Multiaddr, peer_id: Option<PeerId>) -> Option<Multiaddr> {
        let mut output_addr = Multiaddr::empty();

        let ip = addr
            .iter()
            .find(|protocol| matches!(protocol, Protocol::Ip4(_)))?;
        output_addr.push(ip);
        let port = addr
            .iter()
            .find(|protocol| matches!(protocol, Protocol::Udp(_)))?;
        output_addr.push(port);
        output_addr.push(Protocol::QuicV1);

        let peer_id = {
            if let Some(peer_id) = peer_id {
                Protocol::P2p(peer_id)
            } else {
                addr.iter()
                    .find(|protocol| matches!(protocol, Protocol::P2p(_)))?
            }
        };
        output_addr.push(peer_id);
        output_addr.push(Protocol::P2pCircuit);

        debug!("Crafted p2p relay address: {output_addr:?}");
        Some(output_addr)
    }
}
