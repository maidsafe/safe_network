// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Marker, Network, Node, NodeEvent};
#[cfg(feature = "open-metrics")]
use crate::metrics::NodeMetrics;
use libp2p::{autonat::NatStatus, identity::Keypair, Multiaddr, PeerId};
#[cfg(feature = "open-metrics")]
use prometheus_client::registry::Registry;
use rand::{rngs::StdRng, Rng, SeedableRng};
use sn_networking::{
    MsgResponder, NetworkBuilder, NetworkEvent, SwarmLocalState, CLOSE_GROUP_SIZE,
};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::LocalWallet;
use sn_transfers::MainSecretKey;
use std::{
    collections::HashSet,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
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

    /// Subscribe to given gossipsub topic
    pub fn subscribe_to_topic(&self, topic_id: String) -> Result<()> {
        self.network.subscribe_to_topic(topic_id)?;
        Ok(())
    }

    /// Unsubscribe from given gossipsub topic
    pub fn unsubscribe_from_topic(&self, topic_id: String) -> Result<()> {
        self.network.unsubscribe_from_topic(topic_id)?;
        Ok(())
    }

    /// Publish a message on a given gossipsub topic
    pub fn publish_on_topic(&self, topic_id: String, msg: Vec<u8>) -> Result<()> {
        self.network.publish_on_topic(topic_id, msg)?;
        Ok(())
    }
}

impl Node {
    /// Asynchronously runs a new node instance, setting up the swarm driver,
    /// creating a data storage, and handling network events. Returns the
    /// created `RunningNode` which contians a `NodeEventsChannel` for listening
    /// to node-related events.
    ///
    /// # Returns
    ///
    /// A `RunningNode` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the `SwarmDriver`.
    pub fn run(
        keypair: Keypair,
        addr: SocketAddr,
        initial_peers: Vec<Multiaddr>,
        local: bool,
        root_dir: PathBuf,
    ) -> Result<RunningNode> {
        // TODO: Make this key settable, and accessible via API
        let reward_key = MainSecretKey::random();
        let reward_address = reward_key.main_pubkey();

        let mut wallet = LocalWallet::load_from_main_key(&root_dir, reward_key)?;
        // store in case it's a fresh wallet created if none was found
        wallet.deposit_and_store_to_disk(&vec![])?;

        #[cfg(feature = "open-metrics")]
        let (metrics_registry, node_metrics) = {
            let mut metrics_registry = Registry::default();
            let node_metrics = NodeMetrics::new(&mut metrics_registry);
            (metrics_registry, node_metrics)
        };

        let mut network_builder = NetworkBuilder::new(keypair, local, root_dir);
        network_builder.listen_addr(addr);
        #[cfg(feature = "open-metrics")]
        network_builder.metrics_registry(metrics_registry);

        let (network, mut network_event_receiver, swarm_driver) = network_builder.build_node()?;
        let node_events_channel = NodeEventsChannel::default();

        let node = Self {
            network: network.clone(),
            events_channel: node_events_channel.clone(),
            initial_peers,
            reward_address,
            #[cfg(feature = "open-metrics")]
            node_metrics,
        };

        let network_clone = network.clone();
        let node_event_sender = node_events_channel.clone();
        let mut rng = StdRng::from_entropy();

        let peers_connected = Arc::new(AtomicUsize::new(0));

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            // use a random inactivity timeout to ensure that the nodes do not sync when messages
            // are being transmitted.
            let inactivity_timeout: i32 = rng.gen_range(20..40);
            let inactivity_timeout = Duration::from_secs(inactivity_timeout as u64);

            loop {
                let peers_connected = peers_connected.clone();

                tokio::select! {
                    net_event = network_event_receiver.recv() => {
                        match net_event {
                            Some(event) => {
                                let stateless_node_copy = node.clone();
                                let _handle =
                                    spawn(async move { stateless_node_copy.handle_network_event(event, peers_connected).await });
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

    /// Calls Marker::log() to insert the marker into the log files.
    /// Also calls NodeMetrics::record() to record the metric if the `open-metrics` feature flag is enabled.
    pub(crate) fn record_metrics(&self, marker: Marker) {
        marker.log();
        #[cfg(feature = "open-metrics")]
        self.node_metrics.record(marker);
    }

    // **** Private helpers *****

    async fn handle_network_event(&self, event: NetworkEvent, peers_connected: Arc<AtomicUsize>) {
        // when the node has not been connected to enough peers, it should not perform activities
        // that might require peers in the RT to succeed.
        let mut log_when_not_enough_peers = true;
        loop {
            if peers_connected.load(Ordering::Relaxed) >= CLOSE_GROUP_SIZE {
                break;
            }
            match &event {
                // these activities requires the node to be connected to some peer to be able to carry
                // out get kad.get_record etc. This happens during replication/PUT. So we should wait
                // until we have enough nodes, else these might fail.
                NetworkEvent::RequestReceived { .. }
                | NetworkEvent::UnverifiedRecord(_)
                | NetworkEvent::FailedToWrite(_)
                | NetworkEvent::ResponseReceived { .. }
                | NetworkEvent::KeysForReplication(_) => {
                    if log_when_not_enough_peers {
                        debug!("Waiting before processing certain NetworkEvent before reaching {CLOSE_GROUP_SIZE} peers");
                    }
                    log_when_not_enough_peers = false;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                // These events do not need to wait until there are enough peers
                NetworkEvent::PeerAdded(_)
                | NetworkEvent::PeerRemoved(_)
                | NetworkEvent::NewListenAddr(_)
                | NetworkEvent::NatStatusChanged(_)
                | NetworkEvent::GossipsubMsg { .. } => break,
            }
        }
        trace!("Handling NetworkEvent {event:?}");

        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await;
            }
            NetworkEvent::ResponseReceived { res } => {
                trace!("NetworkEvent::ResponseReceived {res:?}");
                if let Err(err) = self.handle_response(res) {
                    error!("Error while handling NetworkEvent::ResponseReceived {err:?}");
                }
            }
            NetworkEvent::PeerAdded(peer_id) => {
                // increment peers_connected and send ConnectedToNetwork event if have connected to K_VALUE peers
                let _ = peers_connected.fetch_add(1, Ordering::SeqCst);
                if peers_connected.load(Ordering::SeqCst) == CLOSE_GROUP_SIZE {
                    self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
                }

                self.record_metrics(Marker::PeerAddedToRoutingTable(peer_id));

                if let Err(err) = self.try_trigger_replication(peer_id, false).await {
                    error!("During CloseGroupUpdate, error while triggering replication {err:?}");
                }
            }
            NetworkEvent::PeerRemoved(peer_id) => {
                self.record_metrics(Marker::PeerRemovedFromRoutingTable(peer_id));
                // During a node restart, the new node got added before the old one got removed.
                // If the old one is `pushed out of close_group by the new one`, then the records
                // that being close to the old one won't got replicated during the CloseGroupUpdate
                // of the new one, as the old one still sits in the local kBuckets.
                // Hence, the replication attempts shall also be undertaken when PeerRemoved.
                if let Err(err) = self.try_trigger_replication(peer_id, true).await {
                    error!("During PeerRemoved, error while triggering replication {err:?}");
                }
            }
            NetworkEvent::KeysForReplication(keys) => {
                self.record_metrics(Marker::fetching_keys_for_replication(&keys));

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
                match self.validate_and_store_record(record).await {
                    Ok(cmdok) => trace!("UnverifiedRecord {key} stored with {cmdok:?}."),
                    Err(err) => {
                        self.record_metrics(Marker::RecordRejected(&key));
                        trace!("UnverifiedRecord {key} failed to be stored with error {err:?}.")
                    }
                }
            }
            NetworkEvent::FailedToWrite(key) => {
                if let Err(e) = self.network.remove_failed_local_record(key) {
                    error!("Failed to remove local record: {e:?}");
                }
            }
            NetworkEvent::GossipsubMsg { topic, msg } => {
                self.events_channel
                    .broadcast(NodeEvent::GossipsubMsg { topic, msg });
            }
        }
    }

    // Handle the response that was not awaited at the call site
    fn handle_response(&self, response: Response) -> Result<()> {
        match response {
            Response::Cmd(CmdResponse::Replicate(Ok(()))) => {
                // Nothing to do, response was fine
                // This only exists to ensure we dont drop the handle and
                // exit early, potentially logging false connection woes
            }
            Response::Query(QueryResponse::GetReplicatedRecord(resp)) => {
                error!("Response to replication shall be handled by called not by common handler, {resp:?}");
            }
            other => {
                warn!("handle_response not implemented for {other:?}");
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
                let payment_address = self.reward_address;
                let store_cost = self.current_storecost().await;
                QueryResponse::GetStoreCost {
                    store_cost,
                    payment_address,
                }
            }
            Query::GetReplicatedRecord { requester, key } => {
                trace!("Got GetReplicatedRecord from {requester:?} regarding {key:?}");

                let our_address = NetworkAddress::from_peer(self.network.peer_id);
                let mut result = Err(ProtocolError::ReplicatedRecordNotFound {
                    holder: Box::new(our_address.clone()),
                    key: Box::new(key.clone()),
                });
                let record_key = key.as_record_key();

                if let Some(record_key) = record_key {
                    if let Ok(Some(record)) = self.network.get_local_record(&record_key).await {
                        result = Ok((our_address, record.value));
                    }
                }

                QueryResponse::GetReplicatedRecord(result)
            }
        };
        Response::Query(resp)
    }

    fn handle_node_cmd(&self, cmd: Cmd) -> Response {
        Marker::NodeCmdReceived(&cmd).log();
        let resp = match cmd {
            Cmd::Replicate { holder, keys } => {
                trace!(
                    "Received replication list from {holder:?} of {} keys {keys:?}",
                    keys.len()
                );

                if let Some(peer_id) = holder.as_peer_id() {
                    // todo: error is not propagated to the caller here
                    let _ = self.add_keys_to_replication_fetcher(peer_id, keys);
                } else {
                    error!("Within the replication list, Can not parse peer_id from {holder:?}");
                }

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
