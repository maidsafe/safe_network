// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Network, Node, NodeEvent};
use libp2p::{autonat::NatStatus, identity::Keypair, kad::RecordKey, Multiaddr, PeerId};
use rand::{rngs::StdRng, Rng, SeedableRng};
use sn_networking::{
    multiaddr_strip_p2p, MsgResponder, NetworkEvent, SwarmDriver, SwarmLocalState,
};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{
        Cmd, CmdResponse, Query, QueryResponse, RegisterCmd, ReplicatedData, Request, Response,
    },
    storage::{registers::User, ChunkWithPayment, DbcAddress},
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
        keypair: Option<Keypair>,
        addr: SocketAddr,
        initial_peers: Vec<(PeerId, Multiaddr)>,
        local: bool,
        root_dir: &Path,
    ) -> Result<RunningNode> {
        let (network, mut network_event_receiver, swarm_driver) =
            SwarmDriver::new(keypair, addr, local, root_dir)?;
        let node_events_channel = NodeEventsChannel::default();

        let mut node = Self {
            network: network.clone(),
            registers: RegisterStorage::new(root_dir),
            events_channel: node_events_channel.clone(),
            initial_peers,
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

                        // Currently trigger the replication query once inactivity detected.
                        // Could reduce the frequence further say `after X times of inactivity`.
                        debug!("No network activity in the past {inactivity_timeout:?}, performing a replication query");
                        let request = Request::Cmd(Cmd::RequestReplication(NetworkAddress::from_peer(network_clone.peer_id)));
                        let _ = network_clone.send_req_no_reply_to_self_closest(&request).await;
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
        let mut stateless_node_copy = self.clone();

        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                trace!("RequestReceived: {req:?}, spawning a new task to handle it");
                // requests are network intensive so we run them in a background task
                let _handle =
                    spawn(async move { stateless_node_copy.handle_request(req, channel).await });
            }
            NetworkEvent::ResponseReceived { res } => {
                trace!("NetworkEvent::ResponseReceived that was not awaited at call site. {res:?}");
                // requests are network intensive so we run them in a background task
                let _handle = spawn(async move {
                    if let Err(err) = stateless_node_copy.handle_response(res).await {
                        error!("Error while handling NetworkEvent::ResponseReceived {err:?}");
                    }
                });
            }
            NetworkEvent::PutRequest { peer, record } => {
                debug!("Got a Record PutRequest from {peer:?}");
                if let Err(err) = self.validate_and_store_record(record).await {
                    error!("Error while validating PutRequest {err:?}");
                }
            }
            NetworkEvent::PeerAdded(peer_id) => {
                debug!("PeerAdded: {peer_id}");
                // perform a get_closest query to self on node join. This should help populate the node's RT
                // since this only runs once, we don't need to make it run in a background task
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
                if let Err(err) = self.try_trigger_replication(&peer_id, false).await {
                    error!("Error while triggering replication {err:?}");
                }
            }
            NetworkEvent::PeerRemoved(peer_id) => {
                if let Err(err) = self.try_trigger_replication(&peer_id, true).await {
                    error!("Error while triggering replication {err:?}");
                }
            }
            NetworkEvent::NewListenAddr(_) => {
                if !cfg!(feature = "local-discovery") {
                    let network = self.network.clone();
                    let peers = self.initial_peers.clone();
                    let _handle = spawn(async move {
                        for (peer_id, addr) in &peers {
                            // The addresses passed might contain the peer_id, which we already pass separately.
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

    // Handle the response that was not awaited at the call site
    async fn handle_response(&mut self, response: Response) -> Result<()> {
        match response {
            Response::Query(QueryResponse::GetReplicatedData(Ok((holder, replicated_data)))) => {
                let address = match replicated_data {
                    ReplicatedData::Chunk(chunk_with_payment) => {
                        let chunk_addr = *chunk_with_payment.chunk.address();
                        debug!("Chunk received for replication: {:?}", chunk_addr.name());
                        let addr =
                            NetworkAddress::from_record_key(RecordKey::new(chunk_addr.name()));

                        let success = self.validate_and_store_chunk(chunk_with_payment).await?;
                        trace!("ReplicatedData::Chunk with {chunk_addr:?} has been validated and stored. {success:?}");
                        addr
                    }
                    ReplicatedData::DbcSpend(signed_spend) => {
                        if let Some(spend) = signed_spend.first() {
                            let dbc_addr = DbcAddress::from_dbc_id(spend.dbc_id());
                            debug!("DbcSpend received for replication: {:?}", dbc_addr.name());
                            let addr =
                                NetworkAddress::from_record_key(RecordKey::new(dbc_addr.name()));

                            let success = self.validate_and_store_spends(signed_spend).await?;
                            trace!("ReplicatedData::Chunk with {addr:?} has been validated and stored. {success:?}");
                            addr
                        } else {
                            // Put validations make sure that we have >= 1 spends and with the same
                            // dbc_id
                            error!("Got ReplicatedData::DbcSpend with zero elements");
                            return Ok(());
                        }
                    }
                    other => {
                        warn!("Not support other type of replicated data {:?}", other);
                        return Ok(());
                    }
                };

                // notify the fetch result
                if let Some(peer_id) = holder.as_peer_id() {
                    let keys_to_fetch = self
                        .network
                        .notify_fetch_result(peer_id, address, true)
                        .await?;
                    self.fetch_replication_keys_without_wait(keys_to_fetch)
                        .await?;
                } else {
                    warn!("Cannot parse PeerId from {holder:?}");
                }
            }
            Response::Query(QueryResponse::GetReplicatedData(Err(
                ProtocolError::ReplicatedDataNotFound { holder, address },
            ))) => {
                // notify the fetch result
                if let Some(peer_id) = holder.as_peer_id() {
                    let keys_to_fetch = self
                        .network
                        .notify_fetch_result(peer_id, address, false)
                        .await?;
                    self.fetch_replication_keys_without_wait(keys_to_fetch)
                        .await?;
                } else {
                    warn!("Cannot parse PeerId from {holder:?}");
                }
            }
            other => {
                error!("handle_response not implemented for {other:?}");
                return Ok(());
            }
        };

        Ok(())
    }

    async fn handle_request(&mut self, request: Request, response_channel: MsgResponder) {
        trace!("Handling request: {request:?}");
        let response = match request {
            Request::Cmd(cmd) => self.handle_node_cmd(cmd).await,
            Request::Query(query) => self.handle_query(query).await,
        };
        self.send_response(response, response_channel).await;
    }

    async fn handle_query(&self, query: Query) -> Response {
        let resp = match query {
            Query::Register(query) => self.registers.read(&query, User::Anyone).await,
            Query::GetChunk(address) => {
                trace!("Got GetChunk query for {address:?}");
                let result = self.get_chunk_from_network(address).await;
                QueryResponse::GetChunk(result)
            }
            Query::GetSpend(address) => {
                trace!("Got GetSpend query for {address:?}");
                let result = self.get_spend_from_network(address).await;
                QueryResponse::GetDbcSpend(result)
            }
            Query::GetReplicatedData {
                requester: _,
                address,
            } => {
                trace!("Got GetReplicatedData query for {address:?}");
                let result = self
                    .get_replicated_data(address)
                    .await
                    .map(|replicated_data| {
                        (
                            NetworkAddress::from_peer(self.network.peer_id),
                            replicated_data,
                        )
                    });
                QueryResponse::GetReplicatedData(result)
            }
        };
        Response::Query(resp)
    }

    async fn handle_node_cmd(&mut self, cmd: Cmd) -> Response {
        let resp = match cmd {
            Cmd::StoreChunk { chunk, payment } => {
                let addr = *chunk.address();
                let chunk_with_payment = ChunkWithPayment { chunk, payment };

                match self.validate_and_store_chunk(chunk_with_payment).await {
                    Ok(cmd_ok) => {
                        self.events_channel.broadcast(NodeEvent::ChunkStored(addr));
                        CmdResponse::StoreChunk(Ok(cmd_ok))
                    }
                    Err(err) => {
                        error!("Failed to StoreChunk: {err:?}");
                        CmdResponse::StoreChunk(Err(err))
                    }
                }
            }
            Cmd::Replicate { holder, keys } => {
                debug!(
                    "Replicate list received from {:?} of {} keys",
                    holder.as_peer_id(),
                    keys.len()
                );

                // todo: error is not propagated to the caller here
                let _ = self.replication_keys_to_fetch(holder, keys).await;
                // if we do not send a response, we can cause connection failures.
                CmdResponse::Replicate(Ok(()))
            }
            Cmd::RequestReplication(sender) => {
                debug!("RequestReplication received from {sender:?}");
                if let Some(peer_id) = sender.as_peer_id() {
                    let _ = self.try_trigger_replication(&peer_id, false).await;
                } else {
                    warn!("Failed to parse peer_id for RequestReplication from {sender:?}");
                };

                // if we do not send a response, we can cause conneciton failures.
                CmdResponse::Replicate(Ok(()))
            }
            Cmd::Register(cmd) => {
                let result = self.registers.write(&cmd).await;

                let xorname = cmd.dst();
                match cmd {
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
                }
            }
            Cmd::SpendDbc(signed_spend) => {
                let dbc_id = *signed_spend.dbc_id();
                let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);
                match self.validate_and_store_spends(vec![signed_spend]).await {
                    Ok(cmd_ok) => {
                        debug!("Broadcasting valid spend: {dbc_id:?} at: {dbc_addr:?}");
                        self.events_channel
                            .broadcast(NodeEvent::SpendStored(dbc_id));
                        CmdResponse::Spend(Ok(cmd_ok))
                    }
                    Err(err) => {
                        error!("Failed to StoreSpend: {err:?}");
                        CmdResponse::Spend(Err(err))
                    }
                }
            }
        };

        Response::Cmd(resp)
    }

    async fn send_response(&self, resp: Response, response_channel: MsgResponder) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }
}
