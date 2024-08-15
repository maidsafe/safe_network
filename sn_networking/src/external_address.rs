// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(dead_code)]

use crate::{driver::NodeBehaviour, mulitaddr_get_ip, multiaddr_is_global};
use itertools::Itertools;
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId, Swarm};
use std::{collections::HashMap, net::IpAddr};

const MAX_REPORTS_BEFORE_CONFIRMATION: u8 = 3;
const MAX_CANDIDATES: usize = 50;

/// Manages the external addresses of a Public node. For a relayed node, the RelayManager should deal with
/// adding and removing external addresses. We don't manage "local" addresses here.
// TODO:
// 1. if the max candidate is reached, kick out the oldest candidate sorted by # of reports
#[derive(Debug)]
pub struct ExternalAddressManager {
    /// All the external addresses of the node
    address_states: Vec<ExternalAddressState>,
    current_ip_address: Option<IpAddr>,

    /// The maximum number of reports before an candidate address is confirmed
    pub max_reports_before_confirmation: u8,
    /// The maximum number of candidates to store
    pub max_candidates: usize,

    pub peer_id: PeerId,
}

impl ExternalAddressManager {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            address_states: Vec::new(),
            current_ip_address: None,
            max_reports_before_confirmation: MAX_REPORTS_BEFORE_CONFIRMATION,
            max_candidates: MAX_CANDIDATES,
            peer_id,
        }
    }

    pub fn set_reports_before_confirmation(&mut self, max_reports_before_confirmation: u8) {
        self.max_reports_before_confirmation = max_reports_before_confirmation;
    }

    pub fn set_max_candidates(&mut self, max_candidates: usize) {
        self.max_candidates = max_candidates;
    }

    /// Get the list of candidate addresses
    pub fn get_candidate_addresses(&self) -> Vec<&Multiaddr> {
        self.address_states
            .iter()
            .filter_map(|state| {
                if let ExternalAddressState::Candidate { address, .. } = state {
                    Some(address)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the list of confirmed addresses
    pub fn get_confirmed_addresses(&self) -> Vec<&Multiaddr> {
        self.address_states
            .iter()
            .filter_map(|state| {
                if let ExternalAddressState::Confirmed { address, .. } = state {
                    Some(address)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Add an external address candidate to the manager.
    /// If the address has been reported often enough, it is confirmed and added to the swarm.
    /// If a new IP address has been reported often enough, then we switch to the new IP address and discard the old
    /// external addresses.
    pub fn add_external_address_candidate(
        &mut self,
        address: Multiaddr,
        swarm: &mut Swarm<NodeBehaviour>,
    ) {
        if !multiaddr_is_global(&address) {
            debug!("Address is not global, ignoring: {address:?}");
            return;
        }

        let Some(address) = self.craft_external_address(&address) else {
            debug!("Address is ill formed, not added to manager: {address:?}");
            return;
        };

        if let Some(state) = self
            .address_states
            .iter_mut()
            .find(|state| state.get_multiaddr() == &address)
        {
            state.increment_reports();

            match state {
                ExternalAddressState::Candidate {
                    num_reports,
                    ip_address,
                    ..
                } => {
                    if *num_reports >= self.max_reports_before_confirmation {
                        // if the IP address of our confirmed address is the same as the new address, then add it
                        let confirmed = if let Some(current_ip_address) = self.current_ip_address {
                            current_ip_address == *ip_address
                        } else {
                            true
                        };

                        if confirmed {
                            info!("External address confirmed, adding it to swarm: {address:?}");
                            swarm.add_external_address(address.clone());
                            *state = ExternalAddressState::Confirmed {
                                address: address.clone(),
                                num_reports: *num_reports,
                                ip_address: *ip_address,
                            };

                            // debug
                            let listen_addr = swarm.listeners().collect::<Vec<_>>();
                            debug!("Listen addresses: {listen_addr:?}");
                            let external_addr = swarm.external_addresses().collect::<Vec<_>>();
                            debug!("External addresses: {external_addr:?}");
                            return;
                        } else {
                            debug!(
                                "External address {address:?} is not confirmed due to mismatched IP address. Checking if we can switch to new IP."
                            );
                        }
                    }
                }
                ExternalAddressState::Confirmed { .. } => {
                    debug!("External address: {address:?} is already confirmed. Do nothing");
                    return;
                }
            }
        }
        // check if we need to update to new ip.
        // TODO: incorrect, we've got multiple reports of a new ip with different ports?? Observe then fix.
        let mut new_ip_map = HashMap::new();

        for state in &self.address_states {
            if let ExternalAddressState::Candidate {
                ip_address,
                num_reports,
                ..
            } = state
            {
                if let Some(current_ip_address) = self.current_ip_address {
                    if current_ip_address != *ip_address
                        && *num_reports >= self.max_reports_before_confirmation
                    {
                        *new_ip_map.entry(ip_address).or_insert(0) += 1;
                    }
                }
            }
        }

        if let Some((&&new_ip, count)) = new_ip_map.iter().sorted_by_key(|(_, count)| *count).last()
        {
            debug!("New IP map: {new_ip_map:?}");
            if *count >= 3 {
                self.switch_to_new_ip(new_ip, swarm);
                return;
            }
        }

        if self.get_candidate_addresses().len() >= self.max_candidates {
            debug!("Max candidates reached, not adding new candidate external address {address:?}");
            return;
        }

        if self
            .address_states
            .iter()
            .any(|state| state.get_multiaddr() == &address)
        {
            debug!(
                "External address {address:?} already exists in manager. Report count incremented."
            );
            return;
        }

        let Some(ip_address) = mulitaddr_get_ip(&address) else {
            return;
        };
        debug!("Added external address to manager: {address:?}");
        self.address_states.push(ExternalAddressState::Candidate {
            address,
            num_reports: 0,
            ip_address,
        });
    }

    /// Adds a non-local listen-addr to the swarm and the manager.
    /// If the IP address of the listen-addr is different from the current IP address, then we directly
    /// switch to the new IP address.
    pub fn add_listen_addr_as_external_address(
        &mut self,
        listen_addr: Multiaddr,
        swarm: &mut Swarm<NodeBehaviour>,
    ) {
        // only add our global addresses
        let address = if multiaddr_is_global(&listen_addr) {
            let Some(address) = self.craft_external_address(&listen_addr) else {
                error!("Listen address is ill formed, not added to manager: {listen_addr:?}");
                return;
            };
            address
        } else {
            debug!("Listen address is not global, ignoring: {listen_addr:?}");
            return;
        };
        let Some(ip_address) = mulitaddr_get_ip(&address) else {
            return;
        };

        if let Some(current_ip_address) = self.current_ip_address {
            if current_ip_address != ip_address {
                // add as candidate with MAX_REPORTS to be confirmed inside switch_to_new_ip
                self.address_states.push(ExternalAddressState::Candidate {
                    address: address.clone(),
                    num_reports: MAX_REPORTS_BEFORE_CONFIRMATION,
                    ip_address,
                });
                self.switch_to_new_ip(ip_address, swarm);
                return;
            }
        }

        if let Some(state) = self
            .address_states
            .iter_mut()
            .find(|state| state.get_multiaddr() == &address)
        {
            match state {
                ExternalAddressState::Candidate { ip_address, .. } => {
                    info!("Listen Addr was found as a candidate. Adding it as external to the swarm {address:?}");
                    swarm.add_external_address(address.clone());
                    *state = ExternalAddressState::Confirmed {
                        address: address.clone(),
                        num_reports: MAX_REPORTS_BEFORE_CONFIRMATION,
                        ip_address: *ip_address,
                    };

                    // debug
                    let listen_addr = swarm.listeners().collect::<Vec<_>>();
                    debug!("Listen addresses: {listen_addr:?}");
                    let external_addr = swarm.external_addresses().collect::<Vec<_>>();
                    debug!("External addresses: {external_addr:?}");
                    return;
                }
                ExternalAddressState::Confirmed { .. } => {
                    debug!("Listen address is already confirmed {address:?}. Do nothing");
                    return;
                }
            }
        }

        // if it is a new one, add it as a confirmed address
        info!("Listen Addr was not found in the manager. Adding it as external to the swarm {address:?}");
        self.address_states.push(ExternalAddressState::Confirmed {
            address: address.clone(),
            num_reports: MAX_REPORTS_BEFORE_CONFIRMATION,
            ip_address,
        });
        swarm.add_external_address(address);
    }

    /// Switch to a new IP address. The old external addresses are removed and the new ones are added.
    /// The new IP address is set as the current IP address.
    fn switch_to_new_ip(&mut self, new_ip: IpAddr, swarm: &mut Swarm<NodeBehaviour>) {
        debug!("Switching to new IpAddr: {new_ip}");
        self.current_ip_address = Some(new_ip);

        // remove all the old confirmed addresses with different ip
        let mut removed_addresses = Vec::new();
        for state in &mut self.address_states {
            if let ExternalAddressState::Confirmed {
                address,
                ip_address,
                ..
            } = state
            {
                if *ip_address != new_ip {
                    removed_addresses.push(address.clone());
                    swarm.remove_external_address(address);
                }
            }
        }
        debug!("Removed addresses due to change of IP: {removed_addresses:?}");

        self.address_states
            .retain(|state| !matches!(state, ExternalAddressState::Confirmed { .. }));

        // add the new confirmed addresses with new ip
        for state in &mut self.address_states {
            if let ExternalAddressState::Candidate {
                address,
                num_reports,
                ip_address,
            } = state
            {
                if *ip_address == new_ip && *num_reports >= self.max_reports_before_confirmation {
                    debug!("Switching to new IP, adding confirmed address: {address:?}");
                    swarm.add_external_address(address.clone());
                    *state = ExternalAddressState::Confirmed {
                        address: address.clone(),
                        num_reports: *num_reports,
                        ip_address: *ip_address,
                    };
                }
            }
        }
    }

    /// Craft a proper address to avoid any ill formed addresses
    fn craft_external_address(&self, given_address: &Multiaddr) -> Option<Multiaddr> {
        let mut output_address = Multiaddr::empty();

        let ip = given_address
            .iter()
            .find(|protocol| matches!(protocol, Protocol::Ip4(_)))?;
        output_address.push(ip);
        let port = given_address
            .iter()
            .find(|protocol| matches!(protocol, Protocol::Udp(_)))?;
        output_address.push(port);
        output_address.push(Protocol::QuicV1);

        output_address.push(Protocol::P2p(self.peer_id));
        Some(output_address)
    }
}

#[derive(Debug)]
enum ExternalAddressState {
    Candidate {
        address: Multiaddr,
        num_reports: u8,
        ip_address: IpAddr,
    },
    Confirmed {
        address: Multiaddr,
        num_reports: u8,
        ip_address: IpAddr,
    },
}

impl ExternalAddressState {
    fn get_multiaddr(&self) -> &Multiaddr {
        match self {
            Self::Candidate { address, .. } => address,
            Self::Confirmed { address, .. } => address,
        }
    }

    fn increment_reports(&mut self) {
        debug!(
            "Incrementing reports for address: {}, current reports: {}",
            self.get_multiaddr(),
            self.get_num_reports(),
        );
        match self {
            Self::Candidate { num_reports, .. } => *num_reports = num_reports.saturating_add(1),
            Self::Confirmed { num_reports, .. } => *num_reports = num_reports.saturating_add(1),
        }
    }

    fn get_num_reports(&self) -> u8 {
        match self {
            Self::Candidate { num_reports, .. } => *num_reports,
            Self::Confirmed { num_reports, .. } => *num_reports,
        }
    }

    fn get_ip_address(&self) -> IpAddr {
        match self {
            Self::Candidate { ip_address, .. } => *ip_address,
            Self::Confirmed { ip_address, .. } => *ip_address,
        }
    }
}
