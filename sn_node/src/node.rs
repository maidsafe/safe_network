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
    Marker, NodeEvent,
};
#[cfg(feature = "open-metrics")]
use crate::metrics::NodeMetrics;
use crate::RunningNode;
use bls::{PublicKey, PK_SIZE};
use bytes::Bytes;
use libp2p::{identity::Keypair, Multiaddr};
#[cfg(feature = "open-metrics")]
use prometheus_client::registry::Registry;
use rand::{rngs::StdRng, Rng, SeedableRng};
use sn_networking::{Network, NetworkBuilder, NetworkEvent, SwarmDriver, CLOSE_GROUP_SIZE};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{ChunkProof, CmdResponse, Query, QueryResponse, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::{CashNoteRedemption, HotWallet, MainPubkey, MainSecretKey, NanoTokens};
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
    sync::{broadcast, mpsc::Receiver},
    task::spawn,
};

/// Expected topic name where notifications of royalty transfers are sent on.
/// The notification msg is expected to contain the serialised public key, followed by the
/// serialised transfer info encrypted against the referenced public key.
pub const ROYALTY_TRANSFER_NOTIF_TOPIC: &str = "ROYALTY_TRANSFER_NOTIFICATION";

#[cfg(feature = "royalties-by-gossip")]
/// Defines the percentage (ie 1/FORWARDER_CHOOSING_FACTOR th of all nodes) of nodes
/// which will act as royalty_transfer_notify forwarder.
const FORWARDER_CHOOSING_FACTOR: usize = 10;

/// Interval to trigger replication of all records to all peers.
/// This is the max time it should take. Minimum interval at any ndoe will be half this
pub const PERIODIC_REPLICATION_INTERVAL_MAX_S: u64 = 45;

/// Helper to build and run a Node
pub struct NodeBuilder {
    keypair: Keypair,
    addr: SocketAddr,
    initial_peers: Vec<Multiaddr>,
    local: bool,
    root_dir: PathBuf,
    #[cfg(feature = "open-metrics")]
    metrics_server_port: u16,
}

impl NodeBuilder {
    /// Instantiate the builder
    pub fn new(
        keypair: Keypair,
        addr: SocketAddr,
        initial_peers: Vec<Multiaddr>,
        local: bool,
        root_dir: PathBuf,
    ) -> Self {
        Self {
            keypair,
            addr,
            initial_peers,
            local,
            root_dir,
            #[cfg(feature = "open-metrics")]
            metrics_server_port: 0,
        }
    }

    #[cfg(feature = "open-metrics")]
    /// Set the port for the OpenMetrics server. Defaults to a random port if not set
    pub fn metrics_server_port(&mut self, port: u16) {
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
        let (metrics_registry, node_metrics) = {
            let mut metrics_registry = Registry::default();
            let node_metrics = NodeMetrics::new(&mut metrics_registry);
            (metrics_registry, node_metrics)
        };

        let mut network_builder = NetworkBuilder::new(self.keypair, self.local, self.root_dir);

        network_builder.enable_gossip();
        network_builder.listen_addr(self.addr);
        #[cfg(feature = "open-metrics")]
        network_builder.metrics_registry(metrics_registry);
        #[cfg(feature = "open-metrics")]
        network_builder.metrics_server_port(self.metrics_server_port);

        let (network, network_event_receiver, swarm_driver) = network_builder.build_node()?;
        let node_events_channel = NodeEventsChannel::default();
        let (node_cmds, _) = broadcast::channel(10);

        let node = Node {
            network: network.clone(),
            events_channel: node_events_channel.clone(),
            node_cmds: node_cmds.clone(),
            initial_peers: Arc::new(self.initial_peers),
            reward_address: Arc::new(reward_address),
            transfer_notifs_filter: None,
            #[cfg(feature = "open-metrics")]
            node_metrics,
        };
        let running_node = RunningNode {
            network,
            node_events_channel,
            node_cmds,
        };

        // Run the node
        node.run(swarm_driver, network_event_receiver);

        // Feature guard ROYALTY_TRANSFER_NOTIF_TOPIC forwarder subscription
        #[cfg(feature = "royalties-by-gossip")]
        {
            // Having a portion of nodes (1/50) subscribe to the ROYALTY_TRANSFER_NOTIF_TOPIC
            // Such nodes become `forwarder` to ensure the actual beneficary won't miss.
            let index: usize = StdRng::from_entropy().gen_range(0..FORWARDER_CHOOSING_FACTOR);
            if index == FORWARDER_CHOOSING_FACTOR / 2 {
                info!("Picked as a forwarding node to subscribe to the {ROYALTY_TRANSFER_NOTIF_TOPIC} topic");
                // Forwarder only needs to forward topic msgs on libp2p level,
                // i.e. no need to handle topic msgs, hence not a `listener`.
                running_node
                    .subscribe_to_topic(ROYALTY_TRANSFER_NOTIF_TOPIC.to_string())
                    .map(|()| info!("Node has been subscribed to gossipsub topic '{ROYALTY_TRANSFER_NOTIF_TOPIC}' to receive network royalties payments notifications."))?;
            }
        }

        Ok(running_node)
    }
}

/// Commands that can be sent by the user to the Node instance, e.g. to mutate some settings.
#[derive(Clone)]
pub enum NodeCmd {
    /// Set a PublicKey to start decoding and accepting Transfer notifications received over gossipsub.
    TransferNotifsFilter(Option<PublicKey>),
}

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
#[derive(Clone)]
pub(crate) struct Node {
    pub(crate) network: Network,
    pub(crate) events_channel: NodeEventsChannel,
    // We keep a copy of the Sender which is clonable and we can obtain a receiver from.
    node_cmds: broadcast::Sender<NodeCmd>,
    // Peers that are dialed at startup of node.
    initial_peers: Arc<Vec<Multiaddr>>,
    reward_address: Arc<MainPubkey>,
    transfer_notifs_filter: Option<PublicKey>,
    #[cfg(feature = "open-metrics")]
    pub(crate) node_metrics: NodeMetrics,
}

impl Node {
    /// Runs the provided `SwarmDriver` and spawns a task to process for `NetworkEvents`
    fn run(
        mut self,
        swarm_driver: SwarmDriver,
        mut network_event_receiver: Receiver<NetworkEvent>,
    ) {
        let mut rng = StdRng::from_entropy();

        let peers_connected = Arc::new(AtomicUsize::new(0));
        let mut cmds_receiver = self.node_cmds.subscribe();

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

            loop {
                let peers_connected = &peers_connected;

                tokio::select! {
                    net_event = network_event_receiver.recv() => {
                        match net_event {
                            Some(event) => {
                                let start = std::time::Instant::now();
                                let event_string = format!("{event:?}");

                                self.handle_network_event(event, peers_connected);
                                trace!("Handled non-blocking network event in {:?}: {:?}", start.elapsed(), event_string);

                            }
                            None => {
                                error!("The `NetworkEvent` channel is closed");
                                self.events_channel.broadcast(NodeEvent::ChannelClosed);
                                break;
                            }
                        }
                    }
                    // runs every replication_interval time
                    _ = replication_interval.tick() => {
                        let start = std::time::Instant::now();
                        trace!("Periodic replication triggered");
                        let network = self.network.clone();
                        self.record_metrics(Marker::IntervalReplicationTriggered);

                        let _handle = spawn(async move {
                            if let Err(err) = Self::try_interval_replication(network)
                            {
                                error!("Error while triggering replication {err:?}");
                            }

                            trace!("Periodic replication took {:?}", start.elapsed());
                        });
                    }
                    node_cmd = cmds_receiver.recv() => {
                        match node_cmd {
                            Ok(NodeCmd::TransferNotifsFilter(filter)) => {
                                self.transfer_notifs_filter = filter;
                                let _ = self.network.start_handle_gossip();
                            }
                            Err(err) => error!("When trying to read from the NodeCmds channel/receiver: {err:?}")
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
        self.node_metrics.record(marker);
    }

    // **** Private helpers *****

    /// Handle a network event.
    /// Spawns a thread for any likely long running tasks
    fn handle_network_event(&self, event: NetworkEvent, peers_connected: &Arc<AtomicUsize>) {
        let start = std::time::Instant::now();
        let event_string = format!("{event:?}");
        let event_header;
        trace!("Handling NetworkEvent {event_string:?}");

        match event {
            NetworkEvent::PeerAdded(peer_id, connected_peers) => {
                event_header = "PeerAdded";
                // increment peers_connected and send ConnectedToNetwork event if have connected to K_VALUE peers
                let _ = peers_connected.fetch_add(1, Ordering::SeqCst);
                if peers_connected.load(Ordering::SeqCst) == CLOSE_GROUP_SIZE {
                    self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
                }

                self.record_metrics(Marker::PeersInRoutingTable(connected_peers));
                self.record_metrics(Marker::PeerAddedToRoutingTable(peer_id));

                // try replication here
                let net_clone = self.network.clone();
                self.record_metrics(Marker::IntervalReplicationTriggered);
                let _handle = spawn(async move {
                    if let Err(err) = Self::try_interval_replication(net_clone) {
                        error!("Error while triggering replication {err:?}");
                    }
                });
            }
            NetworkEvent::PeerRemoved(peer_id, connected_peers) => {
                event_header = "PeerRemoved";
                self.record_metrics(Marker::PeersInRoutingTable(connected_peers));
                self.record_metrics(Marker::PeerRemovedFromRoutingTable(peer_id));

                let net = self.network.clone();
                self.record_metrics(Marker::IntervalReplicationTriggered);
                let _handle = spawn(async move {
                    if let Err(e) = Self::try_interval_replication(net) {
                        error!("Error while triggering replication {e:?}");
                    }
                });
            }
            NetworkEvent::NewListenAddr(_) => {
                event_header = "NewListenAddr";
                if !cfg!(feature = "local-discovery") {
                    let network = self.network.clone();
                    let peers = self.initial_peers.clone();
                    let _handle = spawn(async move {
                        for addr in &*peers {
                            if let Err(err) = network.dial(addr.clone()).await {
                                tracing::error!("Failed to dial {addr}: {err:?}");
                            };
                        }
                    });
                }
            }
            NetworkEvent::ResponseReceived { res } => {
                event_header = "ResponseReceived";
                trace!("NetworkEvent::ResponseReceived {res:?}");
                if let Err(err) = self.handle_response(res) {
                    error!("Error while handling NetworkEvent::ResponseReceived {err:?}");
                }
            }
            NetworkEvent::KeysToFetchForReplication(keys) => {
                event_header = "KeysToFetchForReplication";
                info!("Going to fetch {:?} keys for replication", keys.len());
                self.record_metrics(Marker::fetching_keys_for_replication(&keys));

                if let Err(err) = self.fetch_replication_keys_without_wait(keys) {
                    error!("Error while trying to fetch replicated data {err:?}");
                }
            }
            NetworkEvent::QueryRequestReceived { query, channel } => {
                event_header = "QueryRequestReceived";
                let network = self.network.clone();
                let payment_address = *self.reward_address;

                let _handle = spawn(async move {
                    let res = Self::handle_query(&network, query, payment_address).await;
                    trace!("Sending response {res:?}");

                    if let Err(error) = network.send_response(res, channel) {
                        error!("Error while sending response form query req: {error:?}");
                    }
                });
            }
            NetworkEvent::UnverifiedRecord(record) => {
                event_header = "UnverifiedRecord";
                // queries can be long running and require validation, so we spawn a task to handle them
                let self_clone = self.clone();
                let _handle = spawn(async move {
                    let key = PrettyPrintRecordKey::from(&record.key).into_owned();
                    match self_clone.validate_and_store_record(record).await {
                        Ok(cmdok) => trace!("UnverifiedRecord {key} stored with {cmdok:?}."),
                        Err(err) => {
                            self_clone.record_metrics(Marker::RecordRejected(&key, &err));
                        }
                    }
                });
            }
            NetworkEvent::GossipsubMsgReceived { topic, msg }
            | NetworkEvent::GossipsubMsgPublished { topic, msg } => {
                event_header = "GossipsubMsg";
                trace!("Received a gossip msg for the topic of {topic}");
                let events_channel = self.events_channel.clone();

                if events_channel.receiver_count() == 0 {
                    trace!(
                        "Network handling statistics, Event {event_header:?} handled in {:?} : {event_string:?}",
                        start.elapsed()
                    );
                    return;
                }
                if topic == ROYALTY_TRANSFER_NOTIF_TOPIC {
                    // this is expected to be a notification of a transfer which we treat specially,
                    // and we try to decode it only if it's referring to a PK the user is interested in
                    if let Some(filter_pk) = self.transfer_notifs_filter {
                        let _handle = spawn(async move {
                            match try_decode_transfer_notif(&msg, filter_pk) {
                                Ok(Some(notif_event)) => events_channel.broadcast(notif_event),
                                Ok(None) => { /* transfer notif filered out */ }
                                Err(err) => {
                                    warn!("GossipsubMsg matching the transfer notif. topic name, couldn't be decoded as such: {err:?}");
                                    events_channel
                                        .broadcast(NodeEvent::GossipsubMsg { topic, msg });
                                }
                            }
                        });
                    }
                } else {
                    events_channel.broadcast(NodeEvent::GossipsubMsg { topic, msg });
                }
            }
        }

        trace!(
            "Network handling statistics, Event {event_header:?} handled in {:?} : {event_string:?}",
            start.elapsed()
        );
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
                trace!("Got GetStoreCost request for {address:?}");
                let record_key = address.to_record_key();
                let self_id = network.peer_id;

                let store_cost = network.get_local_storecost(record_key.clone()).await;

                match store_cost {
                    Ok(cost) => {
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
                                quote: Self::create_quote_for_storecost(network, cost, &address),
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
                trace!("Got GetReplicatedRecord from {requester:?} regarding {key:?}");

                let our_address = NetworkAddress::from_peer(network.peer_id);
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
                trace!("Got GetChunkExistenceProof for chunk {key:?}");

                let mut result = Err(ProtocolError::ChunkDoesNotExist(key.clone()));
                if let Ok(Some(record)) = network.get_local_record(&key.to_record_key()).await {
                    let proof = ChunkProof::new(&record.value, nonce);
                    trace!("Chunk proof for {key:?} is {proof:?}");
                    result = Ok(proof)
                } else {
                    trace!(
                        "Could not get ChunkProof for {key:?} as we don't have the record locally."
                    );
                }

                QueryResponse::GetChunkExistenceProof(result)
            }
        };
        Response::Query(resp)
    }
}

fn try_decode_transfer_notif(msg: &[u8], filter: PublicKey) -> eyre::Result<Option<NodeEvent>> {
    let mut key_bytes = [0u8; PK_SIZE];
    key_bytes.copy_from_slice(
        msg.get(0..PK_SIZE)
            .ok_or_else(|| eyre::eyre!("msg doesn't have enough bytes"))?,
    );
    let key = PublicKey::from_bytes(key_bytes)?;
    if key == filter {
        let cashnote_redemptions: Vec<CashNoteRedemption> = rmp_serde::from_slice(&msg[PK_SIZE..])?;
        Ok(Some(NodeEvent::TransferNotif {
            key,
            cashnote_redemptions,
        }))
    } else {
        Ok(None)
    }
}
