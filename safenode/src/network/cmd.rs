// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    network::error::Result,
    protocol::messages::{Request, Response},
};

use super::{error::Error, NetworkSwarmLoop};

use futures::channel::oneshot;
use libp2p::{multiaddr::Protocol, request_response::ResponseChannel, Multiaddr, PeerId};
use std::collections::{hash_map, HashSet};
use tracing::warn;
use xor_name::XorName;

/// Commands to send to the Swarm
#[derive(Debug)]
pub(crate) enum SwarmCmd {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    StoreData {
        xor_name: XorName,
        sender: oneshot::Sender<Result<()>>,
    },
    GetDataProviders {
        xor_name: XorName,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    SendRequest {
        req: Request,
        peer: PeerId,
        sender: oneshot::Sender<Result<Response>>,
    },
    SendResponse {
        resp: Response,
        channel: ResponseChannel<Response>,
    },
}

impl NetworkSwarmLoop {
    pub(crate) fn handle_cmd(&mut self, cmd: SwarmCmd) -> Result<(), Error> {
        match cmd {
            SwarmCmd::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
                    let _routing_update = self
                        .swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self
                        .swarm
                        .dial(peer_addr.with(Protocol::P2p(peer_id.into())))
                    {
                        Ok(()) => {
                            let _ = e.insert(sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(e.into()));
                        }
                    }
                } else {
                    warn!("Already dialing peer.");
                }
            }
            // todo: the `provider` api should not be used for chunks/dbcs.
            // 1. get the closest nodes to the data
            // 2. store data in them directly, not via provider
            SwarmCmd::StoreData { xor_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(xor_name.0.to_vec().into())?;
                let _ = self.pending_start_providing.insert(query_id, sender);
            }
            SwarmCmd::GetDataProviders { xor_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(xor_name.0.to_vec().into());
                let _ = self.pending_get_providers.insert(query_id, sender);
            }
            SwarmCmd::SendRequest { req, peer, sender } => {
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&peer, req);
                let _ = self.pending_requests.insert(request_id, sender);
            }
            SwarmCmd::SendResponse { resp, channel } => {
                self.swarm
                    .behaviour_mut()
                    .request_response
                    .send_response(channel, resp)
                    .map_err(|_| {
                        Error::Other("Connection to peer to be still open.".to_string())
                    })?;
            }
        }
        Ok(())
    }
}
