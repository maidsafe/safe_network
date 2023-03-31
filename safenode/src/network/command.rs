// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    chunk_codec::{ChunkRequest, ChunkResponse},
    EventLoop,
};
use futures::channel::oneshot;
use libp2p::{multiaddr::Protocol, request_response::ResponseChannel, Multiaddr, PeerId};
use std::{
    collections::{hash_map, HashSet},
    error::Error,
};
use xor_name::XorName;

/// Commands to send to the Swarm
#[derive(Debug)]
pub enum CmdToSwarm {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    StoreChunk {
        xor_name: XorName,
        sender: oneshot::Sender<()>,
    },
    GetChunkProviders {
        xor_name: XorName,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    RequestChunk {
        xor_name: XorName,
        peer: PeerId,
        sender: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
    },
    RespondChunk {
        file: Vec<u8>,
        channel: ResponseChannel<ChunkResponse>,
    },
}

impl EventLoop {
    pub async fn handle_command(&mut self, command: CmdToSwarm) {
        match command {
            CmdToSwarm::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
            CmdToSwarm::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self
                        .swarm
                        .dial(peer_addr.with(Protocol::P2p(peer_id.into())))
                    {
                        Ok(()) => {
                            e.insert(sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(Box::new(e)));
                        }
                    }
                } else {
                    todo!("Already dialing peer.");
                }
            }
            CmdToSwarm::StoreChunk { xor_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(xor_name.0.to_vec().into())
                    .expect("No store error.");
                self.pending_start_providing.insert(query_id, sender);
            }
            CmdToSwarm::GetChunkProviders { xor_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(xor_name.0.to_vec().into());
                self.pending_get_providers.insert(query_id, sender);
            }
            CmdToSwarm::RequestChunk {
                xor_name,
                peer,
                sender,
            } => {
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&peer, ChunkRequest(xor_name));
                self.pending_request_file.insert(request_id, sender);
            }
            CmdToSwarm::RespondChunk { file, channel } => {
                self.swarm
                    .behaviour_mut()
                    .request_response
                    .send_response(channel, ChunkResponse(file))
                    .expect("Connection to peer to be still open.");
            }
        }
    }
}
