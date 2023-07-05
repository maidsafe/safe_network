// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Error, MsgResponder, NetworkEvent, SwarmDriver};
use crate::{error::Result, multiaddr_pop_p2p, CLOSE_GROUP_SIZE};
use libp2p::{
    kad::{kbucket::Distance, store::RecordStore, Quorum, Record, RecordKey},
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        DialError,
    },
    Multiaddr, PeerId,
};
use sn_protocol::{
    messages::{Request, Response},
    NetworkAddress,
};
use std::collections::HashSet;
use tokio::sync::oneshot;

/// Commands to send to the Swarm
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
    AddToRoutingTable {
        peer_id: PeerId,
        peer_addr: Multiaddr,
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
    GetSwarmLocalState(oneshot::Sender<SwarmLocalState>),
    /// Check if the local RecordStore contains the provided key
    RecordStoreHasKey {
        key: RecordKey,
        sender: oneshot::Sender<bool>,
    },
    /// Get Record from the Kad network
    GetNetworkRecord {
        key: RecordKey,
        sender: oneshot::Sender<Result<Record>>,
    },
    /// Get data from the local RecordStore
    GetLocalRecord {
        key: RecordKey,
        sender: oneshot::Sender<Option<Record>>,
    },
    /// Put record to network
    PutRecord {
        record: Record,
    },
    /// Put record to the local RecordStore
    PutLocalRecord {
        record: Record,
    },
    /// Get all the Addresses of all Records stored locally
    GetAllRecordAddress {
        sender: oneshot::Sender<HashSet<NetworkAddress>>,
    },
    /// Get the list of keys that within the provided distance to the target Key
    GetRecordKeysClosestToTarget {
        key: NetworkAddress,
        distance: Distance,
        sender: oneshot::Sender<Vec<RecordKey>>,
    },
    AddKeysToReplicationFetcher {
        peer: PeerId,
        keys: Vec<NetworkAddress>,
        sender: oneshot::Sender<Vec<(PeerId, NetworkAddress)>>,
    },
    NotifyFetchResult {
        peer: PeerId,
        key: NetworkAddress,
        result: bool,
        sender: oneshot::Sender<Vec<(PeerId, NetworkAddress)>>,
    },
    // Set the acceptable range of `Record` entry
    SetRecordDistanceRange {
        distance: Distance,
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
    pub(crate) async fn handle_cmd(&mut self, cmd: SwarmCmd) -> Result<(), Error> {
        match cmd {
            SwarmCmd::GetAllRecordAddress { sender } => {
                let addresses = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .record_addresses();
                let _ = sender.send(addresses);
            }
            SwarmCmd::GetRecordKeysClosestToTarget {
                key,
                distance,
                sender,
            } => {
                let peers = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .get_record_keys_closest_to_target(key.as_kbucket_key(), distance);
                let _ = sender.send(peers);
            }
            SwarmCmd::AddKeysToReplicationFetcher { peer, keys, sender } => {
                let keys_to_fetch = self.replication_fetcher.add_keys(peer, keys);
                let _ = sender.send(keys_to_fetch);
            }
            SwarmCmd::NotifyFetchResult {
                peer,
                key,
                result,
                sender,
            } => {
                let keys_to_fetch = self
                    .replication_fetcher
                    .notify_fetch_result(peer, key, result);
                let _ = sender.send(keys_to_fetch);
            }

            SwarmCmd::SetRecordDistanceRange { distance } => {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .set_distance_range(distance);
            }
            SwarmCmd::GetNetworkRecord { key, sender } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_record(key);
                let _ = self.pending_query.insert(query_id, sender);
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
            SwarmCmd::PutRecord { record } => {
                // TODO: shall we wait for the response?
                let _query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, Quorum::All)?;
            }
            SwarmCmd::PutLocalRecord { record } => {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .put_verified(record)?;
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

            SwarmCmd::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::AddToRoutingTable {
                peer_id,
                peer_addr,
                sender,
            } => {
                // TODO: This returns RoutingUpdate, but it doesn't implement `Debug`, so it's a hassle to return.
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, peer_addr);
                let _ = sender.send(Ok(()));
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
                let mut all_peers: Vec<PeerId> = vec![];
                for kbucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
                    for entry in kbucket.iter() {
                        all_peers.push(entry.node.key.clone().into_preimage());
                    }
                }
                all_peers.push(self.self_peer_id);
                let _ = sender.send(all_peers);
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
}
