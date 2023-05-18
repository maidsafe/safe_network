// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Error, MsgResponder, NetworkEvent, SwarmDriver};

use crate::{
    network::error::Result,
    protocol::{
        messages::{QueryResponse, ReplicatedData, Request, Response},
        storage::Chunk,
        NetworkAddress,
    },
};

use libp2p::{
    kad::{Record, RecordKey},
    multiaddr::Protocol,
    Multiaddr, PeerId,
};
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
        sender: oneshot::Sender<Result<QueryResponse>>,
    },
    StoreReplicatedData {
        replicated_data: ReplicatedData,
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
            SwarmCmd::PutProvidedDataAsRecord { record } => {
                // TODO: when do we remove records. Do we need to?
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, libp2p::kad::Quorum::All)?;
            }
            SwarmCmd::StoreReplicatedData { replicated_data } => {
                self.store_repliated_data(replicated_data);
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
                    trace!("Sending request to peer {peer:?}");
                    let request_id = self
                        .swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&peer, req);
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

    fn store_repliated_data(&mut self, replicated_data: ReplicatedData) {
        match replicated_data {
            ReplicatedData::Chunk(chunk) => self.replicate_chunk_to_local(chunk),
            other => warn!("Not supporter other type of replicated data {:?}", other),
        }
    }

    fn replicate_chunk_to_local(&mut self, chunk: Chunk) {
        let addr = *chunk.address();
        debug!("That's a replicate chunk in for :{:?}", addr.name());

        // Create a Kademlia record for storage
        let record = Record {
            key: RecordKey::new(addr.name()),
            value: chunk.value().to_vec(),
            publisher: None,
            expires: None,
        };

        let _ = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .write_to_local(record);
    }
}
