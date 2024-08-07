// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    event::NodeEventsChannel,
    quote::quotes_verification,
    Marker, NodeEvent,
};
#[cfg(feature = "open-metrics")]
use crate::metrics::NodeMetrics;
use crate::RunningNode;
use bytes::Bytes;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
#[cfg(feature = "open-metrics")]
use prometheus_client::metrics::gauge::Gauge;
#[cfg(feature = "open-metrics")]
use prometheus_client::registry::Registry;
use rand::{rngs::StdRng, thread_rng, Rng, SeedableRng};
use sn_networking::{
    close_group_majority, Instant, Network, NetworkBuilder, NetworkError, NetworkEvent, NodeIssue,
    SwarmDriver,
};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{ChunkProof, CmdResponse, Query, QueryResponse, Request, Response},
    NetworkAddress, PrettyPrintRecordKey, CLOSE_GROUP_SIZE,
};
use sn_transfers::{HotWallet, MainPubkey, MainSecretKey, NanoTokens, PAYMENT_FORWARD_PK};
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::mpsc::Receiver,
    task::{spawn, JoinHandle},
};

#[cfg(feature = "reward-forward")]
use libp2p::kad::{Quorum, Record};
#[cfg(feature = "reward-forward")]
use sn_networking::PutRecordCfg;
#[cfg(feature = "reward-forward")]
use sn_protocol::storage::{try_serialize_record, RecordKind, SpendAddress};

/// Interval to trigger replication of all records to all peers.
/// This is the max time it should take. Minimum interval at any node will be half this
pub const PERIODIC_REPLICATION_INTERVAL_MAX_S: u64 = 45;

/// Interval to trigger bad node detection.
/// This is the max time it should take. Minimum interval at any node will be half this
const PERIODIC_BAD_NODE_DETECTION_INTERVAL_MAX_S: u64 = 600;

/// Interval to trigger reward forwarding.
/// This is the max time it should take. Minimum interval at any node will be half this
const PERIODIC_REWARD_FORWARD_INTERVAL_MAX_S: u64 = 450;

/// Max number of attempts that chunk proof verification will be carried out against certain target,
/// before classifying peer as a bad peer.
const MAX_CHUNK_PROOF_VERIFY_ATTEMPTS: usize = 3;

/// Interval between chunk proof verification to be retired against the same target.
const CHUNK_PROOF_VERIFY_RETRY_INTERVAL: Duration = Duration::from_secs(15);

#[cfg(feature = "reward-forward")]
/// Track the forward balance by storing the balance in a file. This is useful to restore the balance between restarts.
const FORWARDED_BALANCE_FILE_NAME: &str = "forwarded_balance";

/// Interval to update the nodes uptime metric
const UPTIME_METRICS_UPDATE_INTERVAL: Duration = Duration::from_secs(10);

/// Helper to build and run a Node
pub struct NodeBuilder {
    keypair: Keypair,
    addr: SocketAddr,
    initial_peers: Vec<Multiaddr>,
    local: bool,
    root_dir: PathBuf,
    #[cfg(feature = "open-metrics")]
    /// Set to Some to enable the metrics server
    metrics_server_port: Option<u16>,
    /// Enable hole punching for nodes connecting from home networks.
    pub is_behind_home_network: bool,
    owner: Option<String>,
    #[cfg(feature = "upnp")]
    upnp: bool,
}

impl NodeBuilder {
    /// Instantiate the builder
    pub fn new(
        keypair: Keypair,
        addr: SocketAddr,
        initial_peers: Vec<Multiaddr>,
        local: bool,
        root_dir: PathBuf,
        owner: Option<String>,
        #[cfg(feature = "upnp")] upnp: bool,
    ) -> Self {
        Self {
            keypair,
            addr,
            initial_peers,
            local,
            root_dir,
            #[cfg(feature = "open-metrics")]
            metrics_server_port: None,
            is_behind_home_network: false,
            owner,
            #[cfg(feature = "upnp")]
            upnp,
        }
    }

    #[cfg(feature = "open-metrics")]
    /// Set the port for the OpenMetrics server. Defaults to a random port if not set
    pub fn metrics_server_port(&mut self, port: Option<u16>) {
        self.metrics_server_port = port;
    }

    /// Asynchronously runs a new node instance, setting up the swarm driver,
    /// creating a data storage, and handling network events. Returns the
    /// created `RunningNode` which contains a `NodeEventsChannel` for listening
    /// to node-related events.
    ///
    /// # Returns
    ///
    /// A `RunningNode` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the `SwarmDriver`.
    pub fn build_and_run(self) -> Result<RunningNode> {
        // Using the signature as the seed of generating the reward_key
        let sig_vec = match self.keypair.sign(b"generate reward seed") {
            Ok(sig) => sig,
            Err(_err) => return Err(Error::FailedToGenerateRewardKey),
        };
        let mut rng = sn_transfers::rng::from_vec(&sig_vec);

        let reward_key = MainSecretKey::random_from_rng(&mut rng);
        let reward_address = reward_key.main_pubkey();

        let mut wallet = HotWallet::load_from_main_key(&self.root_dir, reward_key)?;
        // store in case it's a fresh wallet created if none was found
        wallet.deposit_and_store_to_disk(&vec![])?;

        #[cfg(feature = "open-metrics")]
        let (metrics_registry, node_metrics) = if self.metrics_server_port.is_some() {
            let mut metrics_registry = Registry::default();
            let node_metrics = NodeMetrics::new(&mut metrics_registry);
            (Some(metrics_registry), Some(node_metrics))
        } else {
            (None, None)
        };

        let mut network_builder = NetworkBuilder::new(self.keypair, self.local, self.root_dir);

        network_builder.listen_addr(self.addr);
        #[cfg(feature = "open-metrics")]
        network_builder.metrics_registry(metrics_registry);
        #[cfg(feature = "open-metrics")]
        network_builder.metrics_server_port(self.metrics_server_port);
        network_builder.initial_peers(self.initial_peers.clone());
        network_builder.is_behind_home_network(self.is_behind_home_network);

        #[cfg(feature = "upnp")]
        network_builder.upnp(self.upnp);

        let (network, network_event_receiver, swarm_driver) = network_builder.build_node()?;
        let node_events_channel = NodeEventsChannel::default();

        let node = NodeInner {
            network: network.clone(),
            events_channel: node_events_channel.clone(),
            initial_peers: self.initial_peers,
            reward_address,
            #[cfg(feature = "open-metrics")]
            node_metrics,
            owner: self.owner,
        };
        let node = Node {
            inner: Arc::new(node),
        };
        let running_node = RunningNode {
            network,
            node_events_channel,
        };

        // Run the node
        node.run(swarm_driver, network_event_receiver);

        Ok(running_node)
    }
}

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
#[derive(Clone)]
pub(crate) struct Node {
    inner: Arc<NodeInner>,
}

/// The actual implementation of the Node. The other is just a wrapper around this, so that we don't expose
/// the Arc from the interface.
struct NodeInner {
    events_channel: NodeEventsChannel,
    // Peers that are dialed at startup of node.
    initial_peers: Vec<Multiaddr>,
    network: Network,
    #[cfg(feature = "open-metrics")]
    node_metrics: Option<NodeMetrics>,
    /// Node owner's discord username, in readable format
    /// If not set, there will be no payment forward to be undertaken
    owner: Option<String>,
    reward_address: MainPubkey,
}

impl Node {
    /// Returns the NodeEventsChannel
    pub(crate) fn events_channel(&self) -> &NodeEventsChannel {
        &self.inner.events_channel
    }

    /// Returns the initial peers that the node will dial at startup
    pub(crate) fn initial_peers(&self) -> &Vec<Multiaddr> {
        &self.inner.initial_peers
    }

    /// Returns the instance of Network
    pub(crate) fn network(&self) -> &Network {
        &self.inner.network
    }

    #[cfg(feature = "open-metrics")]
    /// Returns a reference to the NodeMetrics if the `open-metrics` feature flag is enabled
    pub(crate) fn node_metrics(&self) -> Option<&NodeMetrics> {
        self.inner.node_metrics.as_ref()
    }

    /// Returns the owner of the node
    pub(crate) fn owner(&self) -> Option<&String> {
        self.inner.owner.as_ref()
    }

    /// Returns the reward address of the node
    pub(crate) fn reward_address(&self) -> &MainPubkey {
        &self.inner.reward_address
    }

    /// Runs the provided `SwarmDriver` and spawns a task to process for `NetworkEvents`
    fn run(self, swarm_driver: SwarmDriver, mut network_event_receiver: Receiver<NetworkEvent>) {
        let mut rng = StdRng::from_entropy();

        let peers_connected = Arc::new(AtomicUsize::new(0));

        // read the forwarded balance from the file and set the metric.
        // This is done initially because reward forwarding takes a while to kick in
        #[cfg(all(feature = "reward-forward", feature = "open-metrics"))]
        let node_copy = self.clone();
        #[cfg(all(feature = "reward-forward", feature = "open-metrics"))]
        let _handle = spawn(async move {
            let root_dir = node_copy.network().root_dir_path().clone();
            let balance_file_path = root_dir.join(FORWARDED_BALANCE_FILE_NAME);
            let balance = read_forwarded_balance_value(&balance_file_path);

            if let Some(node_metrics) = node_copy.node_metrics() {
                let _ = node_metrics.total_forwarded_rewards.set(balance as i64);
            }
        });

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            // use a random inactivity timeout to ensure that the nodes do not sync when messages
            // are being transmitted.
            let replication_interval: u64 = rng.gen_range(
                PERIODIC_REPLICATION_INTERVAL_MAX_S / 2..PERIODIC_REPLICATION_INTERVAL_MAX_S,
            );
            let replication_interval_time = Duration::from_secs(replication_interval);
            debug!("Replication interval set to {replication_interval_time:?}");

            let mut replication_interval = tokio::time::interval(replication_interval_time);
            let _ = replication_interval.tick().await; // first tick completes immediately

            // use a random timeout to ensure not sync when transmit messages.
            let bad_nodes_check_interval: u64 = rng.gen_range(
                PERIODIC_BAD_NODE_DETECTION_INTERVAL_MAX_S / 2
                    ..PERIODIC_BAD_NODE_DETECTION_INTERVAL_MAX_S,
            );
            let bad_nodes_check_time = Duration::from_secs(bad_nodes_check_interval);
            debug!("BadNodesCheck interval set to {bad_nodes_check_time:?}");

            let mut bad_nodes_check_interval = tokio::time::interval(bad_nodes_check_time);
            let _ = bad_nodes_check_interval.tick().await; // first tick completes immediately

            let mut rolling_index = 0;

            // use a random timeout to ensure not sync when transmit messages.
            let balance_forward_interval: u64 = rng.gen_range(
                PERIODIC_REWARD_FORWARD_INTERVAL_MAX_S / 2..PERIODIC_REWARD_FORWARD_INTERVAL_MAX_S,
            );
            let balance_forward_time = Duration::from_secs(balance_forward_interval);
            debug!(
                "BalanceForward interval set to {balance_forward_time:?} to: {:?}",
                PAYMENT_FORWARD_PK.to_hex(),
            );

            let mut balance_forward_interval = tokio::time::interval(balance_forward_time);
            let _ = balance_forward_interval.tick().await; // first tick completes immediately

            let mut uptime_metrics_update_interval =
                tokio::time::interval(UPTIME_METRICS_UPDATE_INTERVAL);
            let _ = uptime_metrics_update_interval.tick().await; // first tick completes immediately

            loop {
                let peers_connected = &peers_connected;

                tokio::select! {
                    net_event = network_event_receiver.recv() => {
                        match net_event {
                            Some(event) => {
                                let start = Instant::now();
                                let event_string = format!("{event:?}");

                                self.handle_network_event(event, peers_connected);
                                trace!("Handled non-blocking network event in {:?}: {:?}", start.elapsed(), event_string);

                            }
                            None => {
                                error!("The `NetworkEvent` channel is closed");
                                self.events_channel().broadcast(NodeEvent::ChannelClosed);
                                break;
                            }
                        }
                    }
                    // runs every replication_interval time
                    _ = replication_interval.tick() => {
                        let start = Instant::now();
                        debug!("Periodic replication triggered");
                        let network = self.network().clone();
                        self.record_metrics(Marker::IntervalReplicationTriggered);

                        let _handle = spawn(async move {
                            Self::try_interval_replication(network);
                            trace!("Periodic replication took {:?}", start.elapsed());
                        });
                    }
                    // runs every bad_nodes_check_time time
                    _ = bad_nodes_check_interval.tick() => {
                        let start = Instant::now();
                        debug!("Periodic bad_nodes check triggered");
                        let network = self.network().clone();
                        self.record_metrics(Marker::IntervalBadNodesCheckTriggered);

                        let _handle = spawn(async move {
                            Self::try_bad_nodes_check(network, rolling_index).await;
                            trace!("Periodic bad_nodes check took {:?}", start.elapsed());
                        });

                        if rolling_index == 511 {
                            rolling_index = 0;
                        } else {
                            rolling_index += 1;
                        }
                    }
                    // runs every balance_forward_interval time
                    _ = balance_forward_interval.tick() => {
                        if cfg!(feature = "reward-forward") {
                            if let Some(owner) = self.owner() {
                                let start = Instant::now();
                                debug!("Periodic balance forward triggered");
                                let network = self.network().clone();
                                let forwarding_reason = owner.clone();

                                #[cfg(feature = "open-metrics")]
                                let total_forwarded_rewards = self.node_metrics().map(|metrics|metrics.total_forwarded_rewards.clone());
                                #[cfg(feature = "open-metrics")]
                                let current_reward_wallet_balance = self.node_metrics().map(|metrics|metrics.current_reward_wallet_balance.clone());

                                let _handle = spawn(async move {

                                    #[cfg(feature = "open-metrics")]
                                    if let Err(err) =  Self::try_forward_balance(network, forwarding_reason, total_forwarded_rewards,current_reward_wallet_balance) {
                                        error!("Error while trying to forward balance: {err:?}");
                                    }
                                    #[cfg(not(feature = "open-metrics"))]
                                    if let Err(err) = Self::try_forward_balance(network, forwarding_reason) {
                                        error!("Error while trying to forward balance: {err:?}");
                                    }
                                    info!("Periodic balance forward took {:?}", start.elapsed());
                                });
                            }

                        }
                    }
                    _ = uptime_metrics_update_interval.tick() => {
                        #[cfg(feature = "open-metrics")]
                        if let Some(node_metrics) = self.node_metrics() {
                            let _ = node_metrics.uptime.set(node_metrics.started_instant.elapsed().as_secs() as i64);
                        }
                    }
                }
            }
        });
    }

    /// Calls Marker::log() to insert the marker into the log files.
    /// Also calls NodeMetrics::record() to record the metric if the `open-metrics` feature flag is enabled.
    pub(crate) fn record_metrics(&self, marker: Marker) {
        marker.log();
        #[cfg(feature = "open-metrics")]
        if let Some(node_metrics) = self.node_metrics() {
            node_metrics.record(marker)
        }
    }

    // **** Private helpers *****

    /// Handle a network event.
    /// Spawns a thread for any likely long running tasks
    fn handle_network_event(&self, event: NetworkEvent, peers_connected: &Arc<AtomicUsize>) {
        let start = Instant::now();
        let event_string = format!("{event:?}");
        let event_header;
        debug!("Handling NetworkEvent {event_string:?}");

        match event {
            NetworkEvent::PeerAdded(peer_id, connected_peers) => {
                event_header = "PeerAdded";
                // increment peers_connected and send ConnectedToNetwork event if have connected to K_VALUE peers
                let _ = peers_connected.fetch_add(1, Ordering::SeqCst);
                if peers_connected.load(Ordering::SeqCst) == CLOSE_GROUP_SIZE {
                    self.events_channel()
                        .broadcast(NodeEvent::ConnectedToNetwork);
                }

                self.record_metrics(Marker::PeersInRoutingTable(connected_peers));
                self.record_metrics(Marker::PeerAddedToRoutingTable(&peer_id));

                // try replication here
                let network = self.network().clone();
                self.record_metrics(Marker::IntervalReplicationTriggered);
                let _handle = spawn(async move {
                    Self::try_interval_replication(network);
                });
            }
            NetworkEvent::PeerRemoved(peer_id, connected_peers) => {
                event_header = "PeerRemoved";
                self.record_metrics(Marker::PeersInRoutingTable(connected_peers));
                self.record_metrics(Marker::PeerRemovedFromRoutingTable(&peer_id));

                let network = self.network().clone();
                self.record_metrics(Marker::IntervalReplicationTriggered);
                let _handle = spawn(async move {
                    Self::try_interval_replication(network);
                });
            }
            NetworkEvent::PeerWithUnsupportedProtocol { .. } => {
                event_header = "PeerWithUnsupportedProtocol";
            }
            NetworkEvent::NewListenAddr(_) => {
                event_header = "NewListenAddr";
                if !cfg!(feature = "local-discovery") {
                    let network = self.network().clone();
                    let peers = self.initial_peers().clone();
                    let _handle = spawn(async move {
                        for addr in peers {
                            if let Err(err) = network.dial(addr.clone()).await {
                                tracing::error!("Failed to dial {addr}: {err:?}");
                            };
                        }
                    });
                }
            }
            NetworkEvent::ResponseReceived { res } => {
                event_header = "ResponseReceived";
                debug!("NetworkEvent::ResponseReceived {res:?}");
                if let Err(err) = self.handle_response(res) {
                    error!("Error while handling NetworkEvent::ResponseReceived {err:?}");
                }
            }
            NetworkEvent::KeysToFetchForReplication(keys) => {
                event_header = "KeysToFetchForReplication";
                debug!("Going to fetch {:?} keys for replication", keys.len());
                self.record_metrics(Marker::fetching_keys_for_replication(&keys));

                if let Err(err) = self.fetch_replication_keys_without_wait(keys) {
                    error!("Error while trying to fetch replicated data {err:?}");
                }
            }
            NetworkEvent::QueryRequestReceived { query, channel } => {
                event_header = "QueryRequestReceived";
                let network = self.network().clone();
                let payment_address = *self.reward_address();

                let _handle = spawn(async move {
                    let res = Self::handle_query(&network, query, payment_address).await;
                    debug!("Sending response {res:?}");

                    network.send_response(res, channel);
                });
            }
            NetworkEvent::UnverifiedRecord(record) => {
                event_header = "UnverifiedRecord";
                // queries can be long running and require validation, so we spawn a task to handle them
                let self_clone = self.clone();
                let _handle = spawn(async move {
                    let key = PrettyPrintRecordKey::from(&record.key).into_owned();
                    match self_clone.validate_and_store_record(record).await {
                        Ok(()) => debug!("UnverifiedRecord {key} has been stored"),
                        Err(err) => {
                            self_clone.record_metrics(Marker::RecordRejected(&key, &err));
                        }
                    }
                });
            }

            NetworkEvent::TerminateNode { reason } => {
                event_header = "TerminateNode";
                error!("Received termination from swarm_driver due to {reason:?}");
                self.events_channel()
                    .broadcast(NodeEvent::TerminateNode(format!("{reason:?}")));
            }
            NetworkEvent::FailedToFetchHolders(bad_nodes) => {
                event_header = "FailedToFetchHolders";
                let network = self.network().clone();
                // Note: this log will be checked in CI, and expecting `not appear`.
                //       any change to the keyword `failed to fetch` shall incur
                //       correspondent CI script change as well.
                error!("Received notification from replication_fetcher, notifying {bad_nodes:?} failed to fetch replication copies from.");
                let _handle = spawn(async move {
                    for peer_id in bad_nodes {
                        network.record_node_issues(peer_id, NodeIssue::ReplicationFailure);
                    }
                });
            }
            NetworkEvent::QuoteVerification { quotes } => {
                event_header = "QuoteVerification";
                let network = self.network().clone();

                let _handle = spawn(async move {
                    quotes_verification(&network, quotes).await;
                });
            }
            NetworkEvent::ChunkProofVerification {
                peer_id,
                keys_to_verify,
            } => {
                event_header = "ChunkProofVerification";
                let network = self.network().clone();

                debug!("Going to verify chunk {keys_to_verify:?} against peer {peer_id:?}");

                let _handle = spawn(async move {
                    // To avoid the peer is in the process of getting the copy via replication,
                    // repeat the verification for couple of times (in case of error).
                    // Only report the node as bad when ALL the verification attempts failed.
                    let mut attempts = 0;
                    while attempts < MAX_CHUNK_PROOF_VERIFY_ATTEMPTS {
                        if chunk_proof_verify_peer(&network, peer_id, &keys_to_verify).await {
                            return;
                        }
                        // Replication interval is 22s - 45s.
                        // Hence some re-try erquired to allow copies to spread out.
                        tokio::time::sleep(CHUNK_PROOF_VERIFY_RETRY_INTERVAL).await;
                        attempts += 1;
                    }
                    // Now ALL attempts failed, hence report the issue.
                    // Note this won't immediately trigger the node to be considered as BAD.
                    // Only the same peer accumulated three same issue
                    // within 5 mins will be considered as BAD.
                    // As the chunk_proof_check will be triggered every periodical replication,
                    // a low performed or cheaty peer will raise multiple issue alerts during it.
                    network.record_node_issues(peer_id, NodeIssue::FailedChunkProofCheck);
                });
            }
        }

        trace!(
            "Network handling statistics, Event {event_header:?} handled in {:?} : {event_string:?}",
            start.elapsed()
        );
    }

    // Query close_group peers to the target to verifify whether the target is bad_node
    // Returns true when it is a bad_node, otherwise false
    async fn close_nodes_shunning_peer(network: &Network, peer_id: PeerId) -> bool {
        // using `client` to exclude self
        let closest_peers = match network
            .client_get_closest_peers(&NetworkAddress::from_peer(peer_id))
            .await
        {
            Ok(peers) => peers,
            Err(err) => {
                error!("Failed to finding closest_peers to {peer_id:?} client_get_closest_peers errored: {err:?}");
                return false;
            }
        };

        // Query the peer status from the close_group to the peer,
        // raise alert as long as getting alerts from majority(3) of the close_group.
        let req = Request::Query(Query::CheckNodeInProblem(NetworkAddress::from_peer(
            peer_id,
        )));
        let mut handles = Vec::new();
        for peer in closest_peers {
            let req_copy = req.clone();
            let network_copy = network.clone();
            let handle: JoinHandle<bool> = spawn(async move {
                debug!("getting node_status of {peer_id:?} from {peer:?}");
                if let Ok(resp) = network_copy.send_request(req_copy, peer).await {
                    match resp {
                        Response::Query(QueryResponse::CheckNodeInProblem {
                            is_in_trouble,
                            ..
                        }) => is_in_trouble,
                        other => {
                            error!("Cannot get node status of {peer_id:?} from node {peer:?}, with response {other:?}");
                            false
                        }
                    }
                } else {
                    false
                }
            });
            handles.push(handle);
        }
        let results: Vec<_> = futures::future::join_all(handles).await;

        results
            .iter()
            .filter(|r| *r.as_ref().unwrap_or(&false))
            .count()
            >= close_group_majority()
    }

    // Handle the response that was not awaited at the call site
    fn handle_response(&self, response: Response) -> Result<()> {
        match response {
            Response::Cmd(CmdResponse::Replicate(Ok(()))) => {
                // This should actually have been short-circuted when received
                warn!("Mishandled replicate response, should be handled earlier");
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

    async fn handle_query(
        network: &Network,
        query: Query,
        payment_address: MainPubkey,
    ) -> Response {
        let resp: QueryResponse = match query {
            Query::GetStoreCost(address) => {
                debug!("Got GetStoreCost request for {address:?}");
                let record_key = address.to_record_key();
                let self_id = network.peer_id();

                let store_cost = network.get_local_storecost(record_key.clone()).await;

                match store_cost {
                    Ok((cost, quoting_metrics)) => {
                        if cost == NanoTokens::zero() {
                            QueryResponse::GetStoreCost {
                                quote: Err(ProtocolError::RecordExists(
                                    PrettyPrintRecordKey::from(&record_key).into_owned(),
                                )),
                                payment_address,
                                peer_address: NetworkAddress::from_peer(self_id),
                            }
                        } else {
                            QueryResponse::GetStoreCost {
                                quote: Self::create_quote_for_storecost(
                                    network,
                                    cost,
                                    &address,
                                    &quoting_metrics,
                                ),
                                payment_address,
                                peer_address: NetworkAddress::from_peer(self_id),
                            }
                        }
                    }
                    Err(_) => QueryResponse::GetStoreCost {
                        quote: Err(ProtocolError::GetStoreCostFailed),
                        payment_address,
                        peer_address: NetworkAddress::from_peer(self_id),
                    },
                }
            }
            Query::GetReplicatedRecord { requester, key } => {
                debug!("Got GetReplicatedRecord from {requester:?} regarding {key:?}");

                let our_address = NetworkAddress::from_peer(network.peer_id());
                let mut result = Err(ProtocolError::ReplicatedRecordNotFound {
                    holder: Box::new(our_address.clone()),
                    key: Box::new(key.clone()),
                });
                let record_key = key.as_record_key();

                if let Some(record_key) = record_key {
                    if let Ok(Some(record)) = network.get_local_record(&record_key).await {
                        result = Ok((our_address, Bytes::from(record.value)));
                    }
                }

                QueryResponse::GetReplicatedRecord(result)
            }
            Query::GetChunkExistenceProof { key, nonce } => {
                debug!("Got GetChunkExistenceProof for chunk {key:?}");

                let mut result = Err(ProtocolError::ChunkDoesNotExist(key.clone()));
                if let Ok(Some(record)) = network.get_local_record(&key.to_record_key()).await {
                    let proof = ChunkProof::new(&record.value, nonce);
                    debug!("Chunk proof for {key:?} is {proof:?}");
                    result = Ok(proof)
                } else {
                    debug!(
                        "Could not get ChunkProof for {key:?} as we don't have the record locally."
                    );
                }

                QueryResponse::GetChunkExistenceProof(result)
            }
            Query::CheckNodeInProblem(target_address) => {
                debug!("Got CheckNodeInProblem for peer {target_address:?}");

                let is_in_trouble =
                    if let Ok(result) = network.is_peer_shunned(target_address.clone()).await {
                        result
                    } else {
                        debug!("Could not get status of {target_address:?}.");
                        false
                    };

                QueryResponse::CheckNodeInProblem {
                    reporter_address: NetworkAddress::from_peer(network.peer_id()),
                    target_address,
                    is_in_trouble,
                }
            }
        };
        Response::Query(resp)
    }

    async fn try_bad_nodes_check(network: Network, rolling_index: usize) {
        if let Ok(kbuckets) = network.get_kbuckets().await {
            let total_peers: usize = kbuckets.values().map(|peers| peers.len()).sum();
            if total_peers > 100 {
                // The `rolling_index` is rotating among 0-511,
                // meanwhile the returned `kbuckets` only holding non-empty buckets.
                // Hence using the `remainder` calculate to achieve a rolling check.
                // A further `divide by 2` is used to allow `upper or lower part` index within
                // a bucket, to further reduce the concurrent queries.
                let mut bucket_index = (rolling_index / 2) % kbuckets.len();
                let part_index = rolling_index / 2;

                for (distance, peers) in kbuckets.iter() {
                    if bucket_index == 0 {
                        let peers_to_query = if peers.len() > 10 {
                            let split_index = peers.len() / 2;
                            let (left, right) = peers.split_at(split_index);
                            if part_index == 0 {
                                left
                            } else {
                                right
                            }
                        } else {
                            peers
                        };

                        debug!(
                            "Undertake bad_nodes check against bucket {distance} having {} peers, {} candidates to be queried",
                            peers.len(), peers_to_query.len()
                        );
                        for peer_id in peers_to_query {
                            let peer_id_clone = *peer_id;
                            let network_clone = network.clone();
                            let _handle = spawn(async move {
                                let is_bad =
                                    Self::close_nodes_shunning_peer(&network_clone, peer_id_clone)
                                        .await;
                                if is_bad {
                                    network_clone.record_node_issues(
                                        peer_id_clone,
                                        NodeIssue::CloseNodesShunning,
                                    );
                                }
                            });
                        }
                        break;
                    } else {
                        bucket_index = bucket_index.saturating_sub(1);
                    }
                }
            } else {
                debug!("Skip bad_nodes check as not having too many nodes in RT");
            }
        }
    }

    /// Forward received rewards to another address
    fn try_forward_balance(
        network: Network,
        forward_reason: String,
        #[cfg(feature = "open-metrics")] forwarded_balance_metric: Option<Gauge>,
        #[cfg(feature = "open-metrics")] current_reward_wallet_balance: Option<Gauge>,
    ) -> Result<()> {
        let mut spend_requests = vec![];
        {
            // load wallet
            let mut wallet = HotWallet::load_from(network.root_dir_path())?;
            let balance = wallet.balance();

            if !balance.is_zero() {
                let payee = vec![(balance, *PAYMENT_FORWARD_PK)];
                spend_requests.extend(wallet.prepare_forward_signed_spend(payee, forward_reason)?);
            }
        }
        let total_forwarded_amount = spend_requests
            .iter()
            .map(|s| s.token().as_nano())
            .sum::<u64>();

        let record_kind = RecordKind::Spend;
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::Majority,
            retry_strategy: None,
            use_put_record_to: None,
            verification: None,
        };

        info!(
            "Reward forwarding sending {} spends in this iteration. Total forwarded amount: {total_forwarded_amount}",
            spend_requests.len()
        );

        for spend_request in spend_requests {
            let network_clone = network.clone();
            let put_cfg_clone = put_cfg.clone();

            // Sent out spend in separate thread to avoid blocking the main one
            let _handle = spawn(async move {
                let unique_pubkey = *spend_request.unique_pubkey();
                let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);
                let network_address = NetworkAddress::from_spend_address(cash_note_addr);

                let record_key = network_address.to_record_key();
                let pretty_key = PrettyPrintRecordKey::from(&record_key);

                debug!("Reward forwarding in spend {pretty_key:?}: {spend_request:#?}");

                let value = if let Ok(value) = try_serialize_record(&[spend_request], record_kind) {
                    value
                } else {
                    error!("Reward forwarding: Failed to serialise spend {pretty_key:?}");
                    return;
                };

                let record = Record {
                    key: record_key.clone(),
                    value: value.to_vec(),
                    publisher: None,
                    expires: None,
                };

                let result = network_clone.put_record(record, &put_cfg_clone).await;

                match result {
                    Ok(_) => info!("Reward forwarding completed sending spend {pretty_key:?}"),
                    Err(err) => {
                        info!("Reward forwarding: sending spend {pretty_key:?} failed with {err:?}")
                    }
                }
            });

            std::thread::sleep(Duration::from_millis(500));
        }

        // write the balance to a file
        let balance_file_path = network.root_dir_path().join(FORWARDED_BALANCE_FILE_NAME);
        let old_balance = read_forwarded_balance_value(&balance_file_path);
        let updated_balance = old_balance + total_forwarded_amount;
        debug!("Updating forwarded balance to {updated_balance}");
        write_forwarded_balance_value(&balance_file_path, updated_balance)?;

        #[cfg(feature = "open-metrics")]
        {
            if let Some(forwarded_balance_metric) = forwarded_balance_metric {
                let _ = forwarded_balance_metric.set(updated_balance as i64);
            }

            let wallet = HotWallet::load_from(network.root_dir_path())?;
            let balance = wallet.balance();
            if let Some(current_reward_wallet_balance) = current_reward_wallet_balance {
                let _ = current_reward_wallet_balance.set(balance.as_nano() as i64);
            }
        }

        Ok(())
    }
}

fn read_forwarded_balance_value(balance_file_path: &PathBuf) -> u64 {
    debug!("Reading forwarded balance from file {balance_file_path:?}");
    match std::fs::read_to_string(balance_file_path) {
        Ok(balance) => balance.parse::<u64>().unwrap_or_else(|_| {
            debug!("The balance from file is not a valid number");
            0
        }),
        Err(_) => {
            debug!("Error while reading to string, setting the balance to 0. This can happen at node init.");
            0
        }
    }
}

fn write_forwarded_balance_value(balance_file_path: &PathBuf, balance: u64) -> Result<()> {
    if let Err(err) = std::fs::write(balance_file_path, balance.to_string()) {
        error!(
            "Failed to write the updated balance to the file {balance_file_path:?} with {err:?}"
        );
    }
    Ok(())
}

async fn chunk_proof_verify_peer(
    network: &Network,
    peer_id: PeerId,
    keys: &[NetworkAddress],
) -> bool {
    for key in keys.iter() {
        let check_passed = if let Ok(Some(record)) =
            network.get_local_record(&key.to_record_key()).await
        {
            let nonce = thread_rng().gen::<u64>();
            let expected_proof = ChunkProof::new(&record.value, nonce);
            debug!("To verify peer {peer_id:?}, chunk_proof for {key:?} is {expected_proof:?}");

            let request = Request::Query(Query::GetChunkExistenceProof {
                key: key.clone(),
                nonce,
            });
            let responses = network
                .send_and_get_responses(&[peer_id], &request, true)
                .await;
            let n_verified = responses
                .into_iter()
                .filter_map(|(peer, resp)| {
                    received_valid_chunk_proof(key, &expected_proof, peer, resp)
                })
                .count();

            n_verified >= 1
        } else {
            error!(
                 "To verify peer {peer_id:?} Could not get ChunkProof for {key:?} as we don't have the record locally."
            );
            true
        };

        if !check_passed {
            return false;
        }
    }

    true
}

fn received_valid_chunk_proof(
    key: &NetworkAddress,
    expected_proof: &ChunkProof,
    peer: PeerId,
    resp: Result<Response, NetworkError>,
) -> Option<()> {
    if let Ok(Response::Query(QueryResponse::GetChunkExistenceProof(Ok(proof)))) = resp {
        if expected_proof.verify(&proof) {
            debug!(
                "Got a valid ChunkProof of {key:?} from {peer:?}, during peer chunk proof check."
            );
            Some(())
        } else {
            warn!("When verify {peer:?} with ChunkProof of {key:?}, the chunk might have been tampered?");
            None
        }
    } else {
        debug!("Did not get a valid response for the ChunkProof from {peer:?}");
        None
    }
}

#[cfg(test)]
mod tests {

    use crate::node::{read_forwarded_balance_value, write_forwarded_balance_value};
    use color_eyre::Result;
    use tempfile::tempdir;
    #[test]
    fn read_and_write_reward_to_file() -> Result<()> {
        let dir = tempdir()?;
        let balance_file_path = dir.path().join("forwarded_balance");

        let balance = read_forwarded_balance_value(&balance_file_path);
        assert_eq!(balance, 0);

        write_forwarded_balance_value(&balance_file_path, balance + 10)?;
        let balance = read_forwarded_balance_value(&balance_file_path);
        assert_eq!(balance, 10);

        write_forwarded_balance_value(&balance_file_path, balance + 100)?;
        let balance = read_forwarded_balance_value(&balance_file_path);
        assert_eq!(balance, 110);

        Ok(())
    }
}
