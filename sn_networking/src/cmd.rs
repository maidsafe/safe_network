// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Error, MsgResponder, NetworkEvent, SwarmDriver};

use crate::error::Result;

use libp2p::{
    kad::{Record, RecordKey},
    multiaddr::Protocol,
    Multiaddr, PeerId,
};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{Query, QueryResponse, ReplicatedData, Request, Response},
    storage::{Chunk, RecordHeader, RecordKind},
    NetworkAddress,
};
use sn_record_store::DiskBackedRecordStore;
use std::collections::{hash_map, HashSet};
use tokio::sync::oneshot;

/// Commands to send to the Swarm
#[derive(Debug)]
pub enum SwarmCmd {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    AddToRoutingTable {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    GetClosestPeers {
        key: NetworkAddress,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    SendRequest {
        req: Request,
        peer: PeerId,
        sender: oneshot::Sender<Result<Response>>,
    },
    SendResponse {
        resp: Response,
        channel: MsgResponder,
    },
    GetSwarmLocalState(oneshot::Sender<SwarmLocalState>),
    /// Put data to the Kad network as record
    PutProvidedDataAsRecord {
        record: Record,
    },
    /// Get data from the kademlia store
    GetData {
        key: RecordKey,
        sender: oneshot::Sender<Result<Vec<u8>>>,
    },
    GetReplicatedData {
        address: NetworkAddress,
        sender: oneshot::Sender<Result<QueryResponse>>,
    },
    ReplicationKeysToFetch {
        holder: NetworkAddress,
        keys: Vec<NetworkAddress>,
    },
}

/// Snapshot of information kept in the Swarm's local state
#[derive(Debug, Clone)]
pub struct SwarmLocalState {
    /// List of currently connected peers
    pub connected_peers: Vec<PeerId>,
    /// List of aaddresses the node is currently listening on
    pub listeners: Vec<Multiaddr>,
}

impl SwarmDriver {
    pub(crate) async fn handle_cmd(&mut self, cmd: SwarmCmd) -> Result<(), Error> {
        match cmd {
            SwarmCmd::GetData { key, sender } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_record(key);
                let _ = self.pending_query.insert(query_id, sender);
            }
            SwarmCmd::GetReplicatedData { address, sender } => {
                let storage_dir = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .storage_dir();
                let mut resp =
                    QueryResponse::GetReplicatedData(Err(ProtocolError::ReplicatedDataNotFound {
                        holder: NetworkAddress::from_peer(self.self_peer_id),
                        address: address.clone(),
                    }));
                if let Some(record_key) = address.as_record_key() {
                    if let Some(record) =
                        DiskBackedRecordStore::read_from_disk(&record_key, &storage_dir)
                    {
                        let chunk = Chunk::new(record.value.clone().into());
                        trace!("Replicating chunk {:?} to {sender:?}", chunk.name());
                        resp = QueryResponse::GetReplicatedData(Ok((
                            NetworkAddress::from_peer(self.self_peer_id),
                            ReplicatedData::Chunk(chunk),
                        )));
                    }
                } else {
                    warn!("Cannot parse a record_key from {address:?}");
                }
                let _ = sender.send(Ok(resp));
            }
            SwarmCmd::PutProvidedDataAsRecord { record } => {
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, libp2p::kad::Quorum::All)?;
            }
            SwarmCmd::ReplicationKeysToFetch { holder, keys } => {
                self.replication_keys_to_fetch(holder, keys);
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
            SwarmCmd::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                let mut dial_error = None;
                if let hash_map::Entry::Vacant(dial_entry) = self.pending_dial.entry(peer_id) {
                    // immediately write to the pending dial hashmap, as dials can take time,
                    // if we wait until its done more may be in flight
                    let _ = dial_entry.insert(sender);
                    match self
                        .swarm
                        .dial(peer_addr.with(Protocol::P2p(peer_id.into())))
                    {
                        Ok(()) => {}
                        Err(e) => {
                            dial_error = Some(e);
                        }
                    }
                } else {
                    let _ = sender.send(Err(Error::AlreadyDialingPeer(peer_id)));
                }

                // let's inform of our error if we have one
                if let Some(error) = dial_error {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(error.into()));
                    }
                }
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
            SwarmCmd::SendRequest { req, peer, sender } => {
                // If `self` is the recipient, forward the request directly to our upper layer to
                // be handled.
                // `self` then handles the request and sends a response back again to itself.
                if peer == *self.swarm.local_peer_id() {
                    trace!("Sending request to self");
                    self.event_sender
                        .send(NetworkEvent::RequestReceived {
                            req,
                            channel: MsgResponder::FromSelf(sender),
                        })
                        .await?;
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
                    channel
                        .send(Ok(resp))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
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

    pub(crate) fn replicate_chunk_to_local(&mut self, chunk: Chunk) -> Result<()> {
        let addr = *chunk.address();
        debug!("Chunk received for replication: {:?}", addr.name());

        // Prepend Kademlia record with a header for storage
        let record_header = RecordHeader {
            kind: RecordKind::Chunk,
        };
        let mut record_value = bincode::serialize(&record_header)?;
        record_value.extend_from_slice(chunk.value());

        let record = Record {
            key: RecordKey::new(addr.name()),
            value: record_value,
            publisher: None,
            expires: None,
        };

        let _ = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .write_to_local(record);

        Ok(())
    }

    fn replication_keys_to_fetch(&mut self, holder: NetworkAddress, keys: Vec<NetworkAddress>) {
        let peer_id = if let Some(peer_id) = holder.as_peer_id() {
            peer_id
        } else {
            warn!("Cann't parse PeerId from NetworkAddress {holder:?}");
            return;
        };
        trace!("Convert {holder:?} to {peer_id:?}");
        let existing_keys: HashSet<NetworkAddress> = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .record_addresses();
        let non_existing_keys: Vec<NetworkAddress> = keys
            .iter()
            .filter(|key| !existing_keys.contains(key))
            .cloned()
            .collect();
        let keys_to_fetch = self
            .replication_fetcher
            .add_keys(peer_id, non_existing_keys);
        self.fetching_replication_keys(keys_to_fetch);
    }

    pub(crate) fn fetching_replication_keys(
        &mut self,
        keys_to_fetch: Vec<(PeerId, NetworkAddress)>,
    ) {
        for (peer, key) in keys_to_fetch {
            trace!("Fetching replication {key:?} from {peer:?}");
            let request = Request::Query(Query::GetReplicatedData {
                requester: NetworkAddress::from_peer(self.self_peer_id),
                address: key,
            });
            let request_id = self
                .swarm
                .behaviour_mut()
                .request_response
                .send_request(&peer, request);
            trace!("Request_id is {request_id:?}");
        }
    }
}
