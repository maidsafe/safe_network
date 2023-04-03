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

    /// Advertise the local node as the provider of the given file on the DHT.
    pub async fn store_data(&mut self, xor_name: XorName) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::StoreData { xor_name, sender })
            .await?;
        receiver.await?
    }

    /// Find the providers for the given file on the DHT.
    pub async fn get_data_providers(&mut self, xor_name: XorName) -> Result<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::GetDataProviders { xor_name, sender })
            .await?;
        Ok(receiver.await?)
    }

    /// Request the content of the given file from the given peer.
    pub async fn send_safe_request(
        &mut self,
        peer: PeerId,
        xor_name: XorName,
    ) -> Result<SafeResponse> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::SendSafeRequest {
                req: SafeRequest::GetChunk(xor_name),
                peer,
                sender,
            })
            .await?;
        receiver.await?
    }

    /// Respond with the provided file content to the given request.
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
