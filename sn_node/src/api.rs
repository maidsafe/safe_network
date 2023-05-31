// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Network, Node, NodeEvent};
use crate::spendbook::SpendBook;
use libp2p::{
    autonat::NatStatus,
    kad::{Record, RecordKey},
    Multiaddr, PeerId,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use sn_networking::multiaddr_strip_p2p;
use sn_networking::{MsgResponder, NetworkEvent, SwarmDriver, SwarmLocalState};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{Cmd, CmdResponse, Query, QueryResponse, RegisterCmd, Request, Response},
    storage::{registers::User, Chunk, DbcAddress},
    NetworkAddress,
};
use sn_registers::RegisterStorage;

use std::{net::SocketAddr, path::Path, time::Duration};
use tokio::task::spawn;

/// Once a node is started and running, the user obtains
/// a `NodeRunning` object which can be used to interact with it.
pub struct RunningNode {
    network: Network,
    node_events_channel: NodeEventsChannel,
}

impl RunningNode {
    /// Returns this node's `PeerId`
    pub fn peer_id(&self) -> PeerId {
        self.network.peer_id
    }

    /// Returns a `SwarmLocalState` with some information obtained from swarm's local state.
    pub async fn get_swarm_local_state(&self) -> Result<SwarmLocalState> {
        let state = self.network.get_swarm_local_state().await?;
        Ok(state)
    }

    /// Returns the node events channel where to subscribe to receive `NodeEvent`s
    pub fn node_events_channel(&self) -> &NodeEventsChannel {
        &self.node_events_channel
    }
}

impl Node {
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
    pub async fn run(
        addr: SocketAddr,
        initial_peers: Vec<(PeerId, Multiaddr)>,
        local: bool,
        root_dir: &Path,
    ) -> Result<RunningNode> {
        let (network, mut network_event_receiver, swarm_driver) =
            SwarmDriver::new(addr, local, root_dir)?;
        let node_events_channel = NodeEventsChannel::default();

        let mut node = Self {
            network: network.clone(),
            registers: RegisterStorage::new(root_dir),
            events_channel: node_events_channel.clone(),
            initial_peers,
            spendbook: SpendBook::default(),
        };

        let network_clone = network.clone();
        let node_event_sender = node_events_channel.clone();
        let mut rng = StdRng::from_entropy();
        let mut initial_join_flows_done = false;

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            loop {
                // use a random inactivity timeout to ensure that the nodes do not sync when messages
                // are being transmitted.
                let inactivity_timeout: i32 = rng.gen_range(20..40);
                let inactivity_timeout = Duration::from_secs(inactivity_timeout as u64);

                tokio::select! {
                    net_event = network_event_receiver.recv() => {
                        match net_event {
                            Some(event) => node.handle_network_event(event, &mut initial_join_flows_done).await,
                            None => {
                                error!("The `NetworkEvent` channel is closed");
                                node_event_sender.broadcast(NodeEvent::ChannelClosed);
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(inactivity_timeout) => {
                        let random_target = NetworkAddress::from_peer(PeerId::random());

                        debug!("No network activity in the past {inactivity_timeout:?}, performing a random get_closest query to target: {random_target:?}");
                        if let Ok(closest) = network_clone.node_get_closest_peers(&random_target).await {
                            debug!("Network inactivity: get_closest returned {closest:?}");
                        }
                    }
                }
            }
        });

        Ok(RunningNode {
            network,
            node_events_channel,
        })
    }

    // **** Private helpers *****

    async fn handle_network_event(
        &mut self,
        event: NetworkEvent,
        initial_join_flows_done: &mut bool,
    ) {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await
            }
            NetworkEvent::PeerAdded(peer_id) => {
                debug!("PeerAdded: {peer_id}");
                // perform a get_closest query to self on node join. This should help populate the node's RT
                if !*initial_join_flows_done {
                    debug!("Performing a get_closest query to self on node join");
                    if let Ok(closest) = self
                        .network
                        .node_get_closest_peers(&NetworkAddress::from_peer(self.network.peer_id))
                        .await
                    {
                        debug!("closest to self on join returned: {closest:?}");
                    }

                    self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);

                    *initial_join_flows_done = true;
                }
            }
            NetworkEvent::NewListenAddr(_) => {
                if !cfg!(feature = "local-discovery") {
                    let network = self.network.clone();
                    let peers = self.initial_peers.clone();
                    let _handle = spawn(async move {
                        for (peer_id, addr) in &peers {
                            // The addresses passed might contain the peer_id, which we already pass seperately.
                            let addr = multiaddr_strip_p2p(addr);
                            if let Err(err) = network.dial(*peer_id, addr.clone()).await {
                                tracing::error!("Failed to dial {peer_id}: {err:?}");
                            };
                        }
                    });
                }
            }
            NetworkEvent::NatStatusChanged(status) => {
                if matches!(status, NatStatus::Private) {
                    tracing::warn!("NAT status is determined to be private!");
                    self.events_channel.broadcast(NodeEvent::BehindNat);
                }
            }
        }
    }

    async fn handle_request(&mut self, request: Request, response_channel: MsgResponder) {
        trace!("Handling request: {request:?}");
        match request {
            Request::Cmd(cmd) => self.handle_cmd(cmd, response_channel).await,
            Request::Query(query) => self.handle_query(query, response_channel).await,
        }
    }

    async fn handle_query(&mut self, query: Query, response_channel: MsgResponder) {
        let resp = match query {
            Query::Register(query) => self.registers.read(&query, User::Anyone).await,
            Query::GetChunk(address) => {
                match self
                    .network
                    .get_provided_data(RecordKey::new(address.name()))
                    .await
                {
                    Ok(Ok(data)) => QueryResponse::GetChunk(Ok(Chunk::new(data.into()))),
                    Err(err) | Ok(Err(err)) => {
                        error!("Error getting chunk from network: {err}");
                        QueryResponse::GetChunk(Err(ProtocolError::ChunkNotFound(address)))
                    }
                }
            }
            Query::GetSpend(address) => {
                trace!("Got GetSpend query for {address:?}");
                let res = self.spendbook.spend_get(&self.network, address).await;
                trace!("Sending response back on query DbcSpend {address:?}: {res:?}");
                QueryResponse::GetDbcSpend(res)
            }
        };
        self.send_response(Response::Query(resp), response_channel)
            .await;
    }

    async fn handle_cmd(&mut self, cmd: Cmd, response_channel: MsgResponder) {
        match cmd {
            Cmd::StoreChunk(chunk) => {
                let addr = *chunk.address();
                debug!("That's a store chunk in for :{:?}", addr.name());

                // Create a Kademlia record for storage
                let record = Record {
                    key: RecordKey::new(addr.name()),
                    value: chunk.value().to_vec(),
                    publisher: None,
                    expires: None,
                };

                let resp = match self.network.put_data_as_record(record).await {
                    Ok(()) => {
                        self.events_channel.broadcast(NodeEvent::ChunkStored(addr));
                        CmdResponse::StoreChunk(Ok(()))
                    }
                    Err(err) => {
                        error!("Failed to StoreChunk: {err:?}");
                        CmdResponse::StoreChunk(Err(ProtocolError::ChunkNotStored(*addr.name())))
                    }
                };
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
            Cmd::Replicate(replicated_data) => {
                debug!(
                    "That's a replicated data in for :{:?}",
                    replicated_data.name()
                );
                let _ = self
                    .network
                    .store_replicated_data_to_local(replicated_data)
                    .await;
            }
            Cmd::Register(cmd) => {
                let result = self.registers.write(&cmd).await;

                let xorname = cmd.dst();
                let resp = match cmd {
                    RegisterCmd::Create(_) => {
                        self.events_channel
                            .broadcast(NodeEvent::RegisterCreated(xorname));
                        CmdResponse::CreateRegister(result)
                    }
                    RegisterCmd::Edit(_) => {
                        self.events_channel
                            .broadcast(NodeEvent::RegisterEdited(xorname));
                        CmdResponse::EditRegister(result)
                    }
                };
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
            Cmd::SpendDbc(signed_spend) => {
                let dbc_id = *signed_spend.dbc_id();
                let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);

                let resp = match self.spendbook.spend_put(&self.network, signed_spend).await {
                    Ok(addr) => {
                        debug!("Broadcasting valid spend: {dbc_id:?} at: {addr:?}");
                        self.events_channel
                            .broadcast(NodeEvent::SpendStored(dbc_id));
                        CmdResponse::Spend(Ok(()))
                    }
                    Err(err) => {
                        error!("Failed to StoreSpend: {err:?}");
                        CmdResponse::Spend(Err(ProtocolError::FailedToStoreSpend(dbc_addr)))
                    }
                };

                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
        }
    }

    async fn send_response(&self, resp: Response, response_channel: MsgResponder) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }
}
