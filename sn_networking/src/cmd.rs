// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Error, MsgResponder, NetworkEvent, SwarmDriver};
use crate::{error::Result, multiaddr_pop_p2p, sort_peers_by_address, CLOSE_GROUP_SIZE};
use libp2p::{
    kad::{store::RecordStore, Quorum, Record, RecordKey},
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        DialError,
    },
    Multiaddr, PeerId,
};
use sn_dbc::Token;
use sn_protocol::{
    messages::{Request, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
use std::collections::HashSet;
use tokio::sync::oneshot;

/// Commands to send to the Swarm
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum SwarmCmd {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Dial {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    // Get closest peers from the network
    GetClosestPeers {
        key: NetworkAddress,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    // Get closest peers from the local RoutingTable
    GetClosestLocalPeers {
        key: NetworkAddress,
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    // Returns all the peers from all the k-buckets from the local Routing Table.
    // This includes our PeerId as well.
    GetAllLocalPeers {
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    // Returns the peers that are closet to our PeerId.
    GetOurCloseGroup {
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    GetSwarmLocalState(oneshot::Sender<SwarmLocalState>),
    // Send Request to the PeerId.
    SendRequest {
        req: Request,
        peer: PeerId,

        // If a `sender` is provided, the requesting node will await for a `Response` from the
        // Peer. The result is then returned at the call site.
        //
        // If a `sender` is not provided, the requesting node will not wait for the Peer's
        // response. Instead we trigger a `NetworkEvent::ResponseReceived` which calls the common
        // `response_handler`
        sender: Option<oneshot::Sender<Result<Response>>>,
    },
    SendResponse {
        resp: Response,
        channel: MsgResponder,
    },
    /// Check if the local RecordStore contains the provided key
    RecordStoreHasKey {
        key: RecordKey,
        sender: oneshot::Sender<bool>,
    },
    /// Get the Addresses of all the Records held locally
    GetAllLocalRecordAddresses {
        sender: oneshot::Sender<HashSet<NetworkAddress>>,
    },
    /// Get Record from the Kad network
    GetNetworkRecord {
        key: RecordKey,
        sender: oneshot::Sender<Result<Record>>,
    },
    /// GetLocalStoreCost for this node
    GetLocalStoreCost {
        sender: oneshot::Sender<Token>,
    },
    /// Get data from the local RecordStore
    GetLocalRecord {
        key: RecordKey,
        sender: oneshot::Sender<Option<Record>>,
    },
    /// Put record to network
    PutRecord {
        record: Record,
        sender: oneshot::Sender<Result<()>>,
    },
    /// Put record to the local RecordStore
    PutLocalRecord {
        record: Record,
    },
    /// The keys added to the replication fetcher are later used to fetch the Record from the peer/network
    AddKeysToReplicationFetcher {
        peer: PeerId,
        keys: Vec<NetworkAddress>,
    },
}

/// Snapshot of information kept in the Swarm's local state
#[derive(Debug, Clone)]
pub struct SwarmLocalState {
    /// List of currently connected peers
    pub connected_peers: Vec<PeerId>,
    /// List of addresses the node is currently listening on
    pub listeners: Vec<Multiaddr>,
}

impl SwarmDriver {
    pub(crate) fn handle_cmd(&mut self, cmd: SwarmCmd) -> Result<(), Error> {
        let drives_forward_replication = matches!(
            cmd,
            SwarmCmd::PutLocalRecord { .. } | SwarmCmd::AddKeysToReplicationFetcher { .. }
        );

        match cmd {
            SwarmCmd::AddKeysToReplicationFetcher { peer, keys } => {
                // Only store record from Replication that close enough to us.
                let all_peers = self.get_all_local_peers();
                let keys_to_store = keys
                    .iter()
                    .filter(|key| self.is_in_close_range(key, all_peers.clone()))
                    .cloned()
                    .collect();
                #[allow(clippy::mutable_key_type)]
                let all_keys = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .record_addresses_ref();
                let keys_to_fetch =
                    self.replication_fetcher
                        .add_keys(peer, keys_to_store, all_keys);
                if !keys_to_fetch.is_empty() {
                    self.send_event(NetworkEvent::KeysForReplication(keys_to_fetch));
                }
            }
            SwarmCmd::GetNetworkRecord { key, sender } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_record(key);
                if self
                    .pending_get_record
                    .insert(query_id, (sender, Default::default()))
                    .is_some()
                {
                    warn!("An existing get_record task {query_id:?} got replaced");
                }
            }
            SwarmCmd::GetLocalStoreCost { sender } => {
                let cost = self.swarm.behaviour_mut().kademlia.store_mut().store_cost();

                let _res = sender.send(cost);
            }
            SwarmCmd::GetLocalRecord { key, sender } => {
                let record = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .get(&key)
                    .map(|rec| rec.into_owned());
                let _ = sender.send(record);
            }
            SwarmCmd::PutRecord { record, sender } => {
                let record_key = PrettyPrintRecordKey::from(record.key.clone());
                trace!(
                    "Putting record sized: {:?} to network {:?}",
                    record.value.len(),
                    record_key
                );
                let res = match self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, Quorum::All)
                {
                    Ok(request_id) => {
                        trace!("Sent record {record_key:?} to network. Request id: {request_id:?} to network");
                        Ok(())
                    }
                    Err(error) => {
                        error!("Error sending record {record_key:?} to network");
                        Err(Error::from(error))
                    }
                };

                if let Err(err) = sender.send(res) {
                    error!("Could not send response to PutRecord cmd: {:?}", err);
                }
            }
            SwarmCmd::PutLocalRecord { record } => {
                let key = record.key.clone();
                match self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .put_verified(record)
                {
                    Ok(_) => {
                        let new_keys_to_fetch = self.replication_fetcher.notify_about_new_put(key);
                        if !new_keys_to_fetch.is_empty() {
                            self.send_event(NetworkEvent::KeysForReplication(new_keys_to_fetch));
                        }
                    }
                    Err(err) => return Err(err.into()),
                };
            }
            SwarmCmd::RecordStoreHasKey { key, sender } => {
                let has_key = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .contains(&key);
                let _ = sender.send(has_key);
            }
            SwarmCmd::GetAllLocalRecordAddresses { sender } => {
                let addresses = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .record_addresses();
                let _ = sender.send(addresses);
            }

            SwarmCmd::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::Dial { addr, sender } => {
                let _ = match self.dial(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::GetClosestPeers { key, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_peers(key.as_bytes());
                let _ = self
                    .pending_get_closest_peers
                    .insert(query_id, (sender, Default::default()));
            }
            SwarmCmd::GetClosestLocalPeers { key, sender } => {
                let key = key.as_kbucket_key();
                // calls `kbuckets.closest_keys(key)` internally, which orders the peers by
                // increasing distance
                // Note it will return all peers, heance a chop down is required.
                let closest_peers = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_local_peers(&key)
                    .map(|peer| peer.into_preimage())
                    .take(CLOSE_GROUP_SIZE)
                    .collect();

                let _ = sender.send(closest_peers);
            }
            SwarmCmd::GetAllLocalPeers { sender } => {
                let _ = sender.send(self.get_all_local_peers());
            }
            SwarmCmd::GetOurCloseGroup { sender } => {
                let _ = sender.send(self.close_group.clone());
            }
            SwarmCmd::SendRequest { req, peer, sender } => {
                // If `self` is the recipient, forward the request directly to our upper layer to
                // be handled.
                // `self` then handles the request and sends a response back again to itself.
                if peer == *self.swarm.local_peer_id() {
                    trace!("Sending request to self");

                    self.send_event(NetworkEvent::RequestReceived {
                        req,
                        channel: MsgResponder::FromSelf(sender),
                    });
                } else {
                    let request_id = self
                        .swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&peer, req);
                    trace!("Sending request {request_id:?} to peer {peer:?}");
                    let _ = self.pending_requests.insert(request_id, sender);
                }
            }
            SwarmCmd::SendResponse { resp, channel } => match channel {
                // If the response is for `self`, send it directly through the oneshot channel.
                MsgResponder::FromSelf(channel) => {
                    trace!("Sending response to self");
                    match channel {
                        Some(channel) => {
                            channel
                                .send(Ok(resp))
                                .map_err(|_| Error::InternalMsgChannelDropped)?;
                        }
                        None => {
                            // responses that are not awaited at the call site must be handled
                            // separately
                            self.send_event(NetworkEvent::ResponseReceived { res: resp });
                        }
                    }
                }
                MsgResponder::FromPeer(channel) => {
                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, resp)
                        .map_err(Error::OutgoingResponseDropped)?;
                }
            },
            SwarmCmd::GetSwarmLocalState(sender) => {
                let current_state = SwarmLocalState {
                    connected_peers: self.swarm.connected_peers().cloned().collect(),
                    listeners: self.swarm.listeners().cloned().collect(),
                };

                sender
                    .send(current_state)
                    .map_err(|_| Error::InternalMsgChannelDropped)?;
            }
        }

        // in case we're a node and not driving forward and there are keys to replicate, let's fire events for that
        if !self.is_client && !drives_forward_replication {
            let keys_to_fetch = self.replication_fetcher.next_keys_to_fetch();
            if !keys_to_fetch.is_empty() {
                self.send_event(NetworkEvent::KeysForReplication(keys_to_fetch));
            }
        }
        Ok(())
    }

    /// Dials the given multiaddress. If address contains a peer ID, simultaneous
    /// dials to that peer are prevented.
    pub(crate) fn dial(&mut self, mut addr: Multiaddr) -> Result<(), DialError> {
        debug!(%addr, "Dialing manually");

        let peer_id = multiaddr_pop_p2p(&mut addr);
        let opts = match peer_id {
            Some(peer_id) => DialOpts::peer_id(peer_id)
                // If we have a peer ID, we can prevent simultaneous dials.
                .condition(PeerCondition::NotDialing)
                .addresses(vec![addr])
                .build(),
            None => DialOpts::unknown_peer_id().address(addr).build(),
        };

        self.swarm.dial(opts)
    }

    // A close target doesn't falls into the close peers range:
    // For example, a node b11111X has an RT: [(1, b1111), (2, b111), (5, b11), (9, b1), (7, b0)]
    // Then for a target bearing b011111 as prefix, all nodes in (7, b0) are its close_group peers.
    // Then the node b11111X. But b11111X's close_group peers [(1, b1111), (2, b111), (5, b11)]
    // are none among target b011111's close range.
    // Hence, the ilog2 calculation based on close_range cannot cover such case.
    // And have to sort all nodes to figure out whether self is among the close_group to the target.
    fn is_in_close_range(&self, target: &NetworkAddress, all_peers: Vec<PeerId>) -> bool {
        if all_peers.len() <= CLOSE_GROUP_SIZE + 2 {
            return true;
        }

        // Margin of 2 to allow our RT being bit lagging.
        match sort_peers_by_address(all_peers, target, CLOSE_GROUP_SIZE + 2) {
            Ok(close_group) => close_group.contains(&self.self_peer_id),
            Err(err) => {
                warn!("Could not get sorted peers for {target:?} with error {err:?}");
                true
            }
        }
    }
}
