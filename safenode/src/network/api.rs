// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::HashSet;

use super::{Result, SafeRequest, SafeResponse};
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use xor_name::XorName;

use super::command::CmdToSwarm;

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct NetworkApi {
    pub(super) sender: mpsc::Sender<CmdToSwarm>,
}

impl NetworkApi {
    //  Listen for incoming connections on the given address.
    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::StartListening { addr, sender })
            .await?;
        receiver.await?
    }

    /// Dial the given peer at the given address.
    pub async fn dial(&mut self, peer_id: PeerId, peer_addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await?;
        receiver.await?
    }

    /// Advertise the local node as the provider of a given piece of data; The XorName of the data
    /// is advertised to the nodes on the DHT
    pub async fn store_data(&mut self, xor_name: XorName) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::StoreData { xor_name, sender })
            .await?;
        receiver.await?
    }

    /// Find the providers for the given piece of data; The XorName is used to locate the nodes
    /// that hold the data
    pub async fn get_data_providers(&mut self, xor_name: XorName) -> Result<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::GetDataProviders { xor_name, sender })
            .await?;
        Ok(receiver.await?)
    }

    /// Send `SafeRequest` to the the given `PeerId`
    pub async fn send_safe_request(
        &mut self,
        req: SafeRequest,
        peer: PeerId,
    ) -> Result<SafeResponse> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::SendSafeRequest { req, peer, sender })
            .await?;
        receiver.await?
    }

    /// Send a `SafeResponse` through the channel opened by the requester.
    pub async fn send_safe_response(
        &mut self,
        resp: SafeResponse,
        channel: ResponseChannel<SafeResponse>,
    ) -> Result<()> {
        Ok(self
            .sender
            .send(CmdToSwarm::SendSafeResponse { resp, channel })
            .await?)
    }
}
