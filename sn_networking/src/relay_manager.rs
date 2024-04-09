// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::driver::NodeBehaviour;
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId, StreamProtocol, Swarm};
use std::collections::{BTreeMap, HashSet, VecDeque};

const MAX_CONCURRENT_RELAY_CONNECTIONS: usize = 3;
const MAX_POTENTIAL_CANDIDATES: usize = 15;

/// To manager relayed connections.
// todo: try to dial whenever connected_relays drops below threshold. Need to perform this on interval.
pub(crate) struct RelayManager {
    connected_relays: BTreeMap<PeerId, Multiaddr>,
    waiting_for_reservation: BTreeMap<PeerId, Multiaddr>,
    candidates: VecDeque<(PeerId, Multiaddr)>,
}

impl RelayManager {
    pub(crate) fn new(initial_peers: Vec<Multiaddr>) -> Self {
        let candidates = initial_peers
            .into_iter()
            .filter_map(|addr| {
                for protocol in addr.iter() {
                    if let Protocol::P2p(peer_id) = protocol {
                        return Some((peer_id, addr));
                    }
                }
                None
            })
            .collect();
        Self {
            connected_relays: Default::default(),
            waiting_for_reservation: Default::default(),
            candidates,
        }
    }

    /// Add a potential candidate to the list if it satisfies all the identify checks and also supports the relay server
    /// protocol.
    pub(crate) fn add_potential_candidates(
        &mut self,
        peer_id: &PeerId,
        addrs: &HashSet<Multiaddr>,
        stream_protocols: &Vec<StreamProtocol>,
    ) {
        if self.candidates.len() >= MAX_POTENTIAL_CANDIDATES {
            return;
        }

        if Self::does_it_support_relay_server_protocol(stream_protocols) {
            // todo: collect and manage multiple addrs
            if let Some(addr) = addrs.iter().next() {
                // only consider non relayed peers
                if !addr.iter().any(|p| p == Protocol::P2pCircuit) {
                    self.candidates.push_back((*peer_id, addr.clone()));
                }
            }
        }
    }

    // todo: how do we know if a reservation has been revoked / if the peer has gone offline?
    /// Try connecting to candidate relays if we are below the threshold connections.
    /// This is run periodically on a loop.
    pub(crate) fn try_connecting_to_relay(&mut self, swarm: &mut Swarm<NodeBehaviour>) {
        if self.connected_relays.len() >= MAX_CONCURRENT_RELAY_CONNECTIONS {
            return;
        }

        let reservations_to_make = MAX_CONCURRENT_RELAY_CONNECTIONS - self.connected_relays.len();
        let mut n_reservations = 0;

        while n_reservations < reservations_to_make {
            // todo: should we remove all our other `listen_addr`? And should we block from adding `add_external_address` if
            // we're behind nat?
            if let Some((peer_id, addr)) = self.candidates.pop_front() {
                let relay_addr = addr.with(Protocol::P2pCircuit);
                match swarm.listen_on(relay_addr.clone()) {
                    Ok(_) => {
                        info!("Sending reservation to relay {peer_id:?} on {relay_addr:?}");
                        self.waiting_for_reservation.insert(peer_id, relay_addr);
                        n_reservations += 1;
                    }
                    Err(err) => {
                        error!("Error while trying to listen on the relay addr: {err:?} on {relay_addr:?}");
                    }
                }
            } else {
                error!("No more relay candidates");
                break;
            }
        }
    }

    /// Update our state after we've successfully made reservation with a relay.
    pub(crate) fn update_on_successful_reservation(&mut self, peer_id: &PeerId) {
        match self.waiting_for_reservation.remove(peer_id) {
            Some(addr) => {
                info!("Successfully made reservation with {peer_id:?} on {addr:?}");
                self.connected_relays.insert(*peer_id, addr);
            }
            None => {
                debug!("Made a reservation with a peer that we had not requested to");
            }
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
}
