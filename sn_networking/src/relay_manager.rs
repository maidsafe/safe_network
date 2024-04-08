// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::driver::NodeBehaviour;
use libp2p::{
    multiaddr::Protocol,
    swarm::dial_opts::{DialOpts, PeerCondition},
    Multiaddr, PeerId, StreamProtocol, Swarm,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const MAX_CONCURRENT_RELAY_CONNECTIONS: usize = 3;
const MAX_POTENTIAL_CANDIDATES: usize = 15;

/// To manager relayed connections.
// todo: try to dial whenever connected_relays drops below threshold. Need to perform this on interval.
pub(crate) struct RelayManager {
    connected_relays: BTreeMap<PeerId, Multiaddr>,
    dialing_relays: BTreeSet<PeerId>,
    candidates: BTreeMap<PeerId, Multiaddr>,
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
            dialing_relays: Default::default(),
            candidates,
        }
    }

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
                    self.candidates.insert(*peer_id, addr.clone());
                }
            }
        }
    }

    /// Dials candidate relays.
    pub(crate) fn dial_relays(&mut self, swarm: &mut Swarm<NodeBehaviour>) {
        let mut n_dialed = self.connected_relays.len();

        if n_dialed >= MAX_CONCURRENT_RELAY_CONNECTIONS {
            return;
        }

        for (candidate_id, candidate_addr) in self.candidates.iter() {
            match swarm.dial(
                DialOpts::peer_id(candidate_id.clone())
                    .condition(PeerCondition::NotDialing)
                    // todo: should we add P2pCircuit here?
                    // Just perform a direct connection and if the peer supports circuit protocol, the listen_on it.
                    // the `listen_on(adrr.with P2pP)` will create the persistent relay connection right?
                    .addresses(vec![candidate_addr.clone().with(Protocol::P2pCircuit)])
                    .build(),
            ) {
                Ok(_) => {
                    info!("Dialing Relay: {candidate_id:?} succeeded.");
                    self.dialing_relays.insert(*candidate_id);
                    n_dialed += 1;
                    if n_dialed >= MAX_CONCURRENT_RELAY_CONNECTIONS {
                        return;
                    }
                }
                Err(err) => {
                    error!("Error while dialing relay: {candidate_id:?} {err:?}",);
                }
            }
        }
    }

    // todo: should we remove all our other `listen_addr`? Any should we block from adding `add_external_address` if
    // we're behind nat?
    pub(crate) fn try_update_on_connection_success(
        &mut self,
        peer_id: &PeerId,
        stream_protocols: &Vec<StreamProtocol>,
        swarm: &mut Swarm<NodeBehaviour>,
    ) {
        if !self.dialing_relays.contains(peer_id) {
            return;
        }

        let _ = self.dialing_relays.remove(peer_id);

        // this can happen if the initial bootstrap peers does not support the protocol
        if !Self::does_it_support_relay_server_protocol(stream_protocols) {
            let _ = self.candidates.remove(peer_id);
            error!("A dialed relay candidate does not support relay server protocol: {peer_id:?}");
            return;
        }

        // todo: when should we clear out our previous non-relayed listen_addrs?
        if let Some(addr) = self.candidates.remove(peer_id) {
            // if we have less than threshold relay connections, listen on this relayed connection
            if self.connected_relays.len() < MAX_CONCURRENT_RELAY_CONNECTIONS {
                let relay_addr = addr.with(Protocol::P2pCircuit);
                match swarm.listen_on(relay_addr.clone()) {
                    Ok(_) => {
                        info!("Relay connection established with {peer_id:?} on {relay_addr:?}");
                        self.connected_relays.insert(*peer_id, relay_addr);
                    }
                    Err(err) => {
                        error!("Error while trying to listen on relayed connection: {err:?} on {relay_addr:?}");
                    }
                }
            }
        } else {
            error!("Could not find relay candidate after successful connection: {peer_id:?}");
        }
    }

    pub(crate) fn try_update_on_connection_failure(&mut self, peer_id: &PeerId) {
        if !self.connected_relays.contains_key(peer_id)
            && !self.dialing_relays.contains(peer_id)
            && !self.candidates.contains_key(peer_id)
        {
            return;
        }

        if let Some(addr) = self.connected_relays.remove(peer_id) {
            debug!("Removing connected relay from {peer_id:?}: {addr:?} as we had a connection failure");
        }

        if self.dialing_relays.remove(peer_id) {
            debug!("Removing dialing candidate {peer_id:?} as we had a connection failure");
        }

        if let Some(addr) = self.candidates.remove(peer_id) {
            debug!("Removing relay candidate {peer_id:?}: {addr:?} as we had a connection failure");
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
