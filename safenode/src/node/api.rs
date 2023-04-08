// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Node, NodeEvent};

use crate::{
    network::{NetworkEvent, SwarmDriver},
    protocol::{
        messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response},
        types::register::User,
    },
    storage::DataStorage,
};

use libp2p::request_response::ResponseChannel;
use tokio::task::spawn;

impl Node {
    /// Write to storage.
    pub async fn write(&self, cmd: &Cmd) -> CmdResponse {
        info!("Write: {cmd:?}");
        self.storage.write(cmd).await
    }

    /// Read from storage.
    pub async fn read(&self, query: &Query) -> QueryResponse {
        self.storage.read(query, User::Anyone).await
    }

    // /// Return the closest nodes
    // pub async fn get_closest(&self, xor_name: XorName) -> Result<HashSet<PeerId>> {
    //     info!("Getting the closest peers to {:?}", xor_name);

    //     let closest_peers = self
    //         .network
    //         .get_closest_peers(xor_name)
    //         .await?;

    //     Ok(closest_peers)
    // }

    /// Asynchronously runs a new node instance, setting up the swarm driver,
    /// creating a data storage, and handling network events. Returns the
    /// created node and a `NodeEventsChannel` for listening to node-related
    /// events.
    ///
    /// # Returns
    ///
    /// A tuple containing a `Node` instance and a `NodeEventsChannel`.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the `SwarmDriver`.
    pub async fn run() -> Result<(Self, NodeEventsChannel)> {
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new()?;
        let storage = DataStorage::new();
        let node_events_channel = NodeEventsChannel::default();
        let node = Self {
            network,
            storage,
            events_channel: node_events_channel.clone(),
        };
        let mut node_clone = node.clone();

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            loop {
                let event = match network_event_receiver.recv().await {
                    Some(event) => event,
                    None => {
                        error!("The `NetworkEvent` channel has been closed");
                        continue;
                    }
                };
                if let Err(err) = node_clone.handle_network_event(event).await {
                    warn!("Error handling network event: {err}");
                }
            }
        });

        Ok((node, node_events_channel))
    }

    async fn handle_network_event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await?
            }
            NetworkEvent::PeerAdded => {
                self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
            }
        }

        Ok(())
    }

    async fn handle_request(
        &mut self,
        request: Request,
        response_channel: ResponseChannel<Response>,
    ) -> Result<()> {
        trace!("Handling request: {request:?}");
        match request {
            Request::Cmd(cmd) => {
                let resp = self.storage.write(&cmd).await;
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
            Request::Query(query) => {
                let resp = self.storage.read(&query, User::Anyone).await;
                self.send_response(Response::Query(resp), response_channel)
                    .await;
            }
        }

        Ok(())
    }

    async fn send_response(&mut self, resp: Response, response_channel: ResponseChannel<Response>) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }
}
