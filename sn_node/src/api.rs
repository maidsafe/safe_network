// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Marker, Network, Node, NodeEvent};
use libp2p::{autonat::NatStatus, identity::Keypair, Multiaddr, PeerId};
use rand::{rngs::StdRng, Rng, SeedableRng};
use sn_networking::{MsgResponder, NetworkEvent, SwarmDriver, SwarmLocalState};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{Cmd, CmdResponse, Query, QueryResponse, ReplicatedData, Request, Response},
    storage::DbcAddress,
    NetworkAddress, PrettyPrintRecordKey,
};
use std::{
    collections::HashSet,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::spawn;

/// Once a node is started and running, the user obtains
/// a `NodeRunning` object which can be used to interact with it.
#[derive(Clone)]
pub struct RunningNode {
    network: Network,
    node_events_channel: NodeEventsChannel,
}

impl RunningNode {
    /// Returns this node's `PeerId`
    pub fn peer_id(&self) -> PeerId {
        self.network.peer_id
    }

    /// Returns the root directory path for the node.
    ///
    /// This will either be a value defined by the user, or a default location, plus the peer ID
    /// appended. The default location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/node/<peer-id>
    ///  - macOS: $HOME/Library/Application Support/safe/node/<peer-id>
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\node\<peer-id>
    #[allow(rustdoc::invalid_html_tags)]
    pub fn root_dir_path(&self) -> PathBuf {
        self.network.root_dir_path.clone()
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

    /// Returns the list of all the RecordKeys held by the node
    pub async fn get_all_record_addresses(&self) -> Result<HashSet<NetworkAddress>> {
        let addresses = self.network.get_all_local_record_addresses().await?;
        Ok(addresses)
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
        keypair: Keypair,
        addr: SocketAddr,
        initial_peers: Vec<Multiaddr>,
        local: bool,
        root_dir: PathBuf,
    ) -> Result<RunningNode> {
        let (network, mut network_event_receiver, swarm_driver) =
            SwarmDriver::new(keypair, addr, local, root_dir)?;
        let node_events_channel = NodeEventsChannel::default();

        let node = Self {
            network: network.clone(),
            events_channel: node_events_channel.clone(),
            initial_peers,
        };

        let network_clone = network.clone();
        let node_event_sender = node_events_channel.clone();
        let mut rng = StdRng::from_entropy();

        let initial_join_flows_done = Arc::new(AtomicBool::new(false));

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            // use a random inactivity timeout to ensure that the nodes do not sync when messages
            // are being transmitted.
            let inactivity_timeout: i32 = rng.gen_range(20..40);
            let inactivity_timeout = Duration::from_secs(inactivity_timeout as u64);

            loop {
                trace!("NetworkEvent loop started");
                tokio::select! {
                    net_event = network_event_receiver.recv() => {
                        trace!("Handling NetworkEvent: {net_event:?}");
                        let initial_join_flows_done = initial_join_flows_done.clone();
                        match net_event {
                            Some(event) => {
                                let stateless_node_copy = node.clone();
                                let _handle =
                                    spawn(async move { stateless_node_copy.handle_network_event(event, initial_join_flows_done).await });
                            }
                            None => {
                                error!("The `NetworkEvent` channel is closed");
                                node_event_sender.broadcast(NodeEvent::ChannelClosed);
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(inactivity_timeout) => {
                        trace!("NetworkEvent inactivity timeout hit");

                        let network_clone = network_clone.clone();

                        Marker::NoNetworkActivity( inactivity_timeout ).log();
                        let _handle = spawn ( async move {
                            let random_target = NetworkAddress::from_peer(PeerId::random());
                            debug!("No network activity in the past {inactivity_timeout:?}, performing a random get_closest query to target: {random_target:?}");
                            match network_clone.node_get_closest_peers(&random_target).await {
                                Ok(closest) => debug!("Network inactivity: get_closest returned {closest:?}"),
                                Err(e) => {
                                    warn!("get_closest query failed after network inactivity timeout - check your connection: {}", e);
                                    Marker::OperationFailedAfterNetworkInactivityTimeout.log();
                                }
                            }
                        });
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
        &self,
        event: NetworkEvent,
        initial_join_underway_or_done: Arc<AtomicBool>,
    ) {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                trace!("RequestReceived: {req:?}");
                self.handle_request(req, channel).await;
            }
            NetworkEvent::ResponseReceived { res } => {
                trace!("NetworkEvent::ResponseReceived {res:?}");
                if let Err(err) = self.handle_response(res).await {
                    error!("Error while handling NetworkEvent::ResponseReceived {err:?}");
                }
            }
            NetworkEvent::PeerAdded(peer_id) => {
                Marker::PeerAddedToRoutingTable(peer_id).log();

                // perform a get_closest query to self on node join. This should help populate the node's RT
                // since this only runs once, we don't need to make it run in a background task
                debug!(
                    "Initial join query underway or complete: {initial_join_underway_or_done:?}"
                );
                if !initial_join_underway_or_done.load(Ordering::SeqCst) {
                    debug!("Performing a get_closest query to self on node join");
                    initial_join_underway_or_done.store(true, Ordering::SeqCst);

                    if let Ok(closest) = self
                        .network
                        .node_get_closest_peers(&NetworkAddress::from_peer(self.network.peer_id))
                        .await
                    {
                        debug!("closest to self on join returned: {closest:?}");
                    } else {
                        error!("Error while performing a get_closest query to self on node join");
                        initial_join_underway_or_done.store(false, Ordering::SeqCst);
                    }
                }
            }
            NetworkEvent::PeerRemoved(peer_id) => {
                Marker::PeerRemovedFromRoutingTable(peer_id).log();
            }
            NetworkEvent::CloseGroupUpdated(new_members) => {
                Marker::CloseGroupUpdated(&new_members).log();
                if let Err(err) = self.try_trigger_replication().await {
                    error!("Error while triggering replication {err:?}");
                }
            }
            NetworkEvent::KeysForReplication(keys) => {
                Marker::fetching_keys_for_replication(&keys).log();

                if let Err(err) = self.fetch_replication_keys_without_wait(keys) {
                    error!("Error while trying to fetch replicated data {err:?}");
                }
            }
            NetworkEvent::NewListenAddr(_) => {
                if !cfg!(feature = "local-discovery") {
                    let network = self.network.clone();
                    let peers = self.initial_peers.clone();
                    let _handle = spawn(async move {
                        for addr in &peers {
                            if let Err(err) = network.dial(addr.clone()).await {
                                tracing::error!("Failed to dial {addr}: {err:?}");
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
            NetworkEvent::UnverifiedRecord(record) => {
                let key = PrettyPrintRecordKey::from(record.key.clone());
                match self.validate_and_store_record(record, true).await {
                    Ok(cmdok) => trace!("UnverifiedRecord {key:?} stored with {cmdok:?}."),
                    Err(err) => {
                        trace!("UnverifiedRecord {key:?} failed to be stored with error {err:?}.")
                    }
                }
            }
        }
    }

    // Handle the response that was not awaited at the call site
    async fn handle_response(&self, response: Response) -> Result<()> {
        match response {
            Response::Query(QueryResponse::GetReplicatedData(Ok((_holder, replicated_data)))) => {
                match replicated_data {
                    ReplicatedData::Chunk(chunk_with_payment) => {
                        let chunk_addr = *chunk_with_payment.chunk.address();
                        debug!("Chunk received for replication: {:?}", chunk_addr.xorname());

                        let success = self
                            .validate_and_store_chunk(chunk_with_payment, false)
                            .await?;
                        trace!("ReplicatedData::Chunk with {chunk_addr:?} has been validated and stored. {success:?}");
                    }
                    ReplicatedData::DbcSpend(signed_spend) => {
                        if let Some(spend) = signed_spend.first() {
                            let dbc_addr = DbcAddress::from_dbc_id(spend.dbc_id());
                            debug!(
                                "DbcSpend received for replication: {:?}",
                                dbc_addr.xorname()
                            );
                            let addr = NetworkAddress::from_dbc_address(dbc_addr);

                            let success = self.validate_and_store_spends(signed_spend).await?;
                            trace!("ReplicatedData::Dbc with {addr:?} has been validated and stored. {success:?}");
                        } else {
                            // Put validations make sure that we have >= 1 spends and with the same
                            // dbc_id
                            error!("Got ReplicatedData::DbcSpend with zero elements");
                            return Ok(());
                        }
                    }
                    ReplicatedData::Register(register) => {
                        let register_addr = *register.address();
                        debug!(
                            "Register received for replication: {:?}",
                            register_addr.xorname()
                        );

                        let success = self.validate_and_store_register(register).await?;
                        trace!("ReplicatedData::Register with {register_addr:?} has been validated and stored. {success:?}");
                    }
                }
            }
            Response::Query(QueryResponse::GetReplicatedData(Err(
                ProtocolError::ReplicatedDataNotFound { holder, address },
            ))) => {
                warn!("Replicated data {address:?} could not be retrieved from peer: {holder:?}");
            }
            Response::Cmd(CmdResponse::Replicate(Ok(()))) => {
                // Nothing to do, response was fine
                // This only exists to ensure we dont drop the handle and
                // exit early, potentially logging false connection woes
            }
            other => {
                warn!("handle_response not implemented for {other:?}");
                return Ok(());
            }
        };

        Ok(())
    }

    async fn handle_request(&self, request: Request, response_channel: MsgResponder) {
        trace!("Handling request: {request:?}");
        let response = match request {
            Request::Cmd(cmd) => self.handle_node_cmd(cmd),
            Request::Query(query) => self.handle_query(query).await,
        };
        self.send_response(response, response_channel);
    }

    async fn handle_query(&self, query: Query) -> Response {
        let resp: QueryResponse = match query {
            Query::GetStoreCost(_address) => {
                trace!("Got GetStoreCost");
                let result = self.current_storecost().await;
                QueryResponse::GetStoreCost(result)
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

    fn handle_node_cmd(&self, cmd: Cmd) -> Response {
        Marker::NodeCmdReceived(&cmd).log();
        let resp = match cmd {
            Cmd::Replicate { holder, keys } => {
                debug!(
                    "Replicate list received from {:?} of {} keys",
                    holder.as_peer_id(),
                    keys.len()
                );

                // todo: error is not propagated to the caller here
                let _ = self.add_keys_to_replication_fetcher(holder, keys);
                // if we do not send a response, we can cause connection failures.
                CmdResponse::Replicate(Ok(()))
            }
        };

        Marker::NodeCmdResponded(&resp).log();

        Response::Cmd(resp)
    }

    fn send_response(&self, resp: Response, response_channel: MsgResponder) {
        if let Err(err) = self.network.send_response(resp, response_channel) {
            warn!("Error while sending response: {err:?}");
        }
    }
}
