// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod api;
mod error;
mod event;

pub use event::NodeEvent;

use self::{error::Result, event::VaultEventsChannel};
use crate::{
    network::{Network, NetworkEvent, NetworkSwarmLoop},
    protocol::{
        messages::{Request, Response},
        types::register::User,
    },
    storage::DataStorage,
};
use libp2p::request_response::ResponseChannel;
use tokio::task::spawn;

/// Safe node
#[derive(Clone)]
pub struct Vault {
    network: Network,
    storage: DataStorage,
    vault_events_channel: VaultEventsChannel,
}

impl Vault {
    /// Create and run the `Node`
    pub async fn run() -> Result<(Self, VaultEventsChannel)> {
        let (network, mut network_events, network_event_loop) = NetworkSwarmLoop::new()?;
        let storage = DataStorage::new();
        let vault_events_channel = VaultEventsChannel::default();
        let node = Self {
            network,
            storage,
            vault_events_channel: vault_events_channel.clone(),
        };
        let mut node_clone = node.clone();

        // Run the network in the background
        let _handle = spawn(network_event_loop.run());

        // Spawn a task to handle `NetworkEvents`
        let _handle = spawn(async move {
            loop {
                let event = match network_events.recv().await {
                    Some(event) => event,
                    None => {
                        error!("The `NetworkEvent` channel has been closed, something went wrong");
                        continue;
                    }
                };
                if let Err(err) = node_clone.handle_network_events(event).await {
                    warn!("Error while handling network events: {err}");
                }
            }
        });

        Ok((node, vault_events_channel))
    }

    /// Handle incoming `NetworkEvent`
    async fn handle_network_events(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await?
            }
            NetworkEvent::PeerAdded => {
                self.vault_events_channel
                    .broadcast(NodeEvent::ConnectedToNetwork);
            }
        }

        Ok(())
    }

    /// Handle incoming `Request` from a peer
    async fn handle_request(
        &mut self,
        request: Request,
        response_channel: ResponseChannel<Response>,
    ) -> Result<()> {
        trace!("Handling request: {request:?}");
        match request {
            Request::Query(query) => {
                let resp = self.storage.query(&query, User::Anyone).await;
                self.send_response(Response::Query(resp), response_channel)
                    .await;
            }
            Request::Cmd(cmd) => {
                let resp = self.storage.store(&cmd).await;
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
        }

        Ok(())
    }

    // Helper to sender a response back
    async fn send_response(&mut self, resp: Response, response_channel: ResponseChannel<Response>) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }
}
