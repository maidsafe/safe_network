// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg(feature = "open-metrics")]
use crate::metrics::NetworkMetrics;
#[cfg(feature = "open-metrics")]
use crate::metrics_service::run_metrics_server;
use crate::{
    bootstrap::{ContinuousBootstrap, BOOTSTRAP_INTERVAL},
    circular_vec::CircularVec,
    cmd::SwarmCmd,
    error::{NetworkError, Result},
    event::NetworkEvent,
    event::NodeEvent,
    get_record_handler::PendingGetRecord,
    multiaddr_pop_p2p,
    network_discovery::NetworkDiscovery,
    record_store::{ClientRecordStore, NodeRecordStore, NodeRecordStoreConfig},
    record_store_api::UnifiedRecordStore,
    replication_fetcher::ReplicationFetcher,
    target_arch::{interval, spawn, Instant},
    transport, Network, CLOSE_GROUP_SIZE,
};
use crate::{NodeIssue, REPLICATE_RANGE};
use futures::StreamExt;
use libp2p::kad::KBucketDistance as Distance;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    identity::Keypair,
    kad::{self, QueryId, Quorum, Record, K_VALUE},
    multiaddr::Protocol,
    request_response::{self, Config as RequestResponseConfig, OutboundRequestId, ProtocolSupport},
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        ConnectionId, DialError, NetworkBehaviour, StreamProtocol, Swarm,
    },
    Multiaddr, PeerId, Transport,
};
#[cfg(feature = "open-metrics")]
use prometheus_client::registry::Registry;
use sn_protocol::{
    messages::{ChunkProof, Nonce, Request, Response},
    storage::RetryStrategy,
    version::{
        IDENTIFY_CLIENT_VERSION_STR, IDENTIFY_NODE_VERSION_STR, IDENTIFY_PROTOCOL_STR,
        REQ_RESPONSE_VERSION_STR,
    },
    NetworkAddress, PrettyPrintKBucketKey, PrettyPrintRecordKey,
};
use sn_transfers::PaymentQuote;
use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Debug,
    net::SocketAddr,
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use tracing::warn;

/// Interval over which we check for the farthest record we _should_ be holding
/// based upon our knowledge of the CLOSE_GROUP
pub(crate) const CLOSET_RECORD_CHECK_INTERVAL: Duration = Duration::from_secs(15);

/// The ways in which the Get Closest queries are used.
pub(crate) enum PendingGetClosestType {
    /// The network discovery method is present at the networking layer
    /// Thus we can just process the queries made by NetworkDiscovery without using any channels
    NetworkDiscovery,
    /// These are queries made by a function at the upper layers and contains a channel to send the result back.
    FunctionCall(oneshot::Sender<Vec<PeerId>>),
}
type PendingGetClosest = HashMap<QueryId, (PendingGetClosestType, Vec<PeerId>)>;

/// What is the largest packet to send over the network.
/// Records larger than this will be rejected.
// TODO: revisit once cashnote_redemption is in
const MAX_PACKET_SIZE: usize = 1024 * 1024 * 5; // the chunk size is 1mb, so should be higher than that to prevent failures, 5mb here to allow for CashNote storage

// Timeout for requests sent/received through the request_response behaviour.
const REQUEST_TIMEOUT_DEFAULT_S: Duration = Duration::from_secs(30);
// Sets the keep-alive timeout of idle connections.
const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);

const NETWORKING_CHANNEL_SIZE: usize = 10_000;

/// Time before a Kad query times out if no response is received
const KAD_QUERY_TIMEOUT_S: Duration = Duration::from_secs(10);

/// The various settings to apply to when fetching a record from network
#[derive(Clone)]
pub struct GetRecordCfg {
    /// The query will result in an error if we get records less than the provided Quorum
    pub get_quorum: Quorum,
    /// If enabled, the provided `RetryStrategy` is used to retry if a GET attempt fails.
    pub retry_strategy: Option<RetryStrategy>,
    /// Only return if we fetch the provided record.
    pub target_record: Option<Record>,
    /// Logs if the record was not fetched from the provided set of peers.
    pub expected_holders: HashSet<PeerId>,
}

impl GetRecordCfg {
    pub fn does_target_match(&self, record: &Record) -> bool {
        self.target_record.as_ref().is_some_and(|t| t == record)
    }
}

impl Debug for GetRecordCfg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("GetRecordCfg");
        f.field("get_quorum", &self.get_quorum)
            .field("retry_strategy", &self.retry_strategy);

        match &self.target_record {
            Some(record) => {
                let pretty_key = PrettyPrintRecordKey::from(&record.key);
                f.field("target_record", &pretty_key);
            }
            None => {
                f.field("target_record", &"None");
            }
        };

        f.field("expected_holders", &self.expected_holders).finish()
    }
}

/// The various settings related to writing a record to the network.
#[derive(Debug, Clone)]
pub struct PutRecordCfg {
    /// The quorum used by KAD PUT. KAD still sends out the request to all the peers set by the `replication_factor`, it
    /// just makes sure that we get atleast `n` successful responses defined by the Quorum.
    /// Our nodes currently send `Ok()` response for every KAD PUT. Thus this field does not do anything atm.
    pub put_quorum: Quorum,
    /// If enabled, the provided `RetryStrategy` is used to retry if a PUT attempt fails.
    pub retry_strategy: Option<RetryStrategy>,
    /// Use the `kad::put_record_to` to PUT the record only to the specified peers. If this option is set to None, we
    /// will be using `kad::put_record` which would PUT the record to all the closest members of the record.
    pub use_put_record_to: Option<Vec<PeerId>>,
    /// Enables verification after writing. The VerificationKind is used to determine the method to use.
    pub verification: Option<(VerificationKind, GetRecordCfg)>,
}

/// The methods in which verification on a PUT can be carried out.
#[derive(Debug, Clone)]
pub enum VerificationKind {
    /// Uses the default KAD GET to perform verification.
    Network,
    /// Uses the hash based verification for chunks.
    ChunkProof {
        expected_proof: ChunkProof,
        nonce: Nonce,
    },
}

/// NodeBehaviour struct
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NodeEvent")]
pub(super) struct NodeBehaviour {
    pub(super) request_response: request_response::cbor::Behaviour<Request, Response>,
    pub(super) kademlia: kad::Behaviour<UnifiedRecordStore>,
    #[cfg(feature = "local-discovery")]
    pub(super) mdns: mdns::tokio::Behaviour,
    pub(super) identify: libp2p::identify::Behaviour,
}

#[derive(Debug)]
pub struct NetworkBuilder {
    keypair: Keypair,
    local: bool,
    root_dir: PathBuf,
    listen_addr: Option<SocketAddr>,
    request_timeout: Option<Duration>,
    concurrency_limit: Option<usize>,
    #[cfg(feature = "open-metrics")]
    metrics_registry: Option<Registry>,
    #[cfg(feature = "open-metrics")]
    metrics_server_port: u16,
}

impl NetworkBuilder {
    pub fn new(keypair: Keypair, local: bool, root_dir: PathBuf) -> Self {
        Self {
            keypair,
            local,
            root_dir,
            listen_addr: None,
            request_timeout: None,
            concurrency_limit: None,
            #[cfg(feature = "open-metrics")]
            metrics_registry: None,
            #[cfg(feature = "open-metrics")]
            metrics_server_port: 0,
        }
    }

    pub fn listen_addr(&mut self, listen_addr: SocketAddr) {
        self.listen_addr = Some(listen_addr);
    }

    pub fn request_timeout(&mut self, request_timeout: Duration) {
        self.request_timeout = Some(request_timeout);
    }

    pub fn concurrency_limit(&mut self, concurrency_limit: usize) {
        self.concurrency_limit = Some(concurrency_limit);
    }

    #[cfg(feature = "open-metrics")]
    pub fn metrics_registry(&mut self, metrics_registry: Registry) {
        self.metrics_registry = Some(metrics_registry);
    }

    #[cfg(feature = "open-metrics")]
    pub fn metrics_server_port(&mut self, port: u16) {
        self.metrics_server_port = port;
    }

    /// Creates a new `SwarmDriver` instance, along with a `Network` handle
    /// for sending commands and an `mpsc::Receiver<NetworkEvent>` for receiving
    /// network events. It initializes the swarm, sets up the transport, and
    /// configures the Kademlia and mDNS behaviour for peer discovery.
    ///
    /// # Returns
    ///
    /// A tuple containing a `Network` handle, an `mpsc::Receiver<NetworkEvent>`,
    /// and a `SwarmDriver` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the mDNS behaviour.
    pub fn build_node(self) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        let mut kad_cfg = kad::Config::default();
        let _ = kad_cfg
            .set_kbucket_inserts(libp2p::kad::BucketInserts::Manual)
            // how often a node will replicate records that it has stored, aka copying the key-value pair to other nodes
            // this is a heavier operation than publication, so it is done less frequently
            // Set to `None` to ensure periodic replication disabled.
            .set_replication_interval(None)
            // how often a node will publish a record key, aka telling the others it exists
            // Set to `None` to ensure periodic publish disabled.
            .set_publication_interval(None)
            // 1mb packet size
            .set_max_packet_size(MAX_PACKET_SIZE)
            // How many nodes _should_ store data.
            .set_replication_factor(
                NonZeroUsize::new(CLOSE_GROUP_SIZE)
                    .ok_or_else(|| NetworkError::InvalidCloseGroupSize)?,
            )
            .set_query_timeout(KAD_QUERY_TIMEOUT_S)
            // Require iterative queries to use disjoint paths for increased resiliency in the presence of potentially adversarial nodes.
            .disjoint_query_paths(true)
            // Records never expire
            .set_record_ttl(None)
            // Emit PUT events for validation prior to insertion into the RecordStore.
            // This is no longer needed as the record_storage::put now can carry out validation.
            // .set_record_filtering(KademliaStoreInserts::FilterBoth)
            // Disable provider records publication job
            .set_provider_publication_interval(None);

        let store_cfg = {
            // Configures the disk_store to store records under the provided path and increase the max record size
            let storage_dir_path = self.root_dir.join("record_store");
            if let Err(error) = std::fs::create_dir_all(&storage_dir_path) {
                return Err(NetworkError::FailedToCreateRecordStoreDir {
                    path: storage_dir_path,
                    source: error,
                });
            }
            NodeRecordStoreConfig {
                max_value_bytes: MAX_PACKET_SIZE, // TODO, does this need to be _less_ than MAX_PACKET_SIZE
                storage_dir: storage_dir_path,
                ..Default::default()
            }
        };

        let listen_addr = self.listen_addr;

        let (network, events_receiver, mut swarm_driver) = self.build(
            kad_cfg,
            Some(store_cfg),
            false,
            ProtocolSupport::Full,
            IDENTIFY_NODE_VERSION_STR.to_string(),
        )?;

        // Listen on the provided address
        let listen_socket_addr = listen_addr.ok_or(NetworkError::ListenAddressNotProvided)?;

        // Listen on QUIC
        let addr_quic = Multiaddr::from(listen_socket_addr.ip())
            .with(Protocol::Udp(listen_socket_addr.port()))
            .with(Protocol::QuicV1);
        let _listener_id = swarm_driver
            .swarm
            .listen_on(addr_quic)
            .expect("Multiaddr should be supported by our configured transports");

        // Listen on WebSocket
        #[cfg(any(feature = "websockets", target_arch = "wasm32"))]
        {
            let addr_ws = Multiaddr::from(listen_socket_addr.ip())
                .with(Protocol::Tcp(listen_socket_addr.port()))
                .with(Protocol::Ws("/".into()));
            let _listener_id = swarm_driver
                .swarm
                .listen_on(addr_ws)
                .expect("Multiaddr should be supported by our configured transports");
        }

        Ok((network, events_receiver, swarm_driver))
    }

    /// Same as `build_node` API but creates the network components in client mode
    pub fn build_client(self) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        // Create a Kademlia behaviour for client mode, i.e. set req/resp protocol
        // to outbound-only mode and don't listen on any address
        let mut kad_cfg = kad::Config::default(); // default query timeout is 60 secs

        // 1mb packet size
        let _ = kad_cfg
            .set_max_packet_size(MAX_PACKET_SIZE)
            // Require iterative queries to use disjoint paths for increased resiliency in the presence of potentially adversarial nodes.
            .disjoint_query_paths(true)
            // How many nodes _should_ store data.
            .set_replication_factor(
                NonZeroUsize::new(CLOSE_GROUP_SIZE)
                    .ok_or_else(|| NetworkError::InvalidCloseGroupSize)?,
            );

        let (network, net_event_recv, driver) = self.build(
            kad_cfg,
            None,
            true,
            ProtocolSupport::Outbound,
            IDENTIFY_CLIENT_VERSION_STR.to_string(),
        )?;

        Ok((network, net_event_recv, driver))
    }

    /// Private helper to create the network components with the provided config and req/res behaviour
    fn build(
        self,
        kad_cfg: kad::Config,
        record_store_cfg: Option<NodeRecordStoreConfig>,
        is_client: bool,
        req_res_protocol: ProtocolSupport,
        identify_version: String,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        let peer_id = PeerId::from(self.keypair.public());
        // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
        #[cfg(not(target_arch = "wasm32"))]
        info!(
            "Process (PID: {}) with PeerId: {peer_id}",
            std::process::id()
        );
        info!(
            "Self PeerID {peer_id} is represented as kbucket_key {:?}",
            PrettyPrintKBucketKey(NetworkAddress::from_peer(peer_id).as_kbucket_key())
        );

        #[cfg(feature = "open-metrics")]
        let network_metrics = {
            let mut metrics_registry = self.metrics_registry.unwrap_or_default();
            let metrics = NetworkMetrics::new(&mut metrics_registry);
            run_metrics_server(metrics_registry, self.metrics_server_port);
            metrics
        };

        // RequestResponse Behaviour
        let request_response = {
            let cfg = RequestResponseConfig::default()
                .with_request_timeout(self.request_timeout.unwrap_or(REQUEST_TIMEOUT_DEFAULT_S));

            info!(
                "Building request response with {:?}",
                REQ_RESPONSE_VERSION_STR.as_str()
            );
            request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new(&REQ_RESPONSE_VERSION_STR),
                    req_res_protocol,
                )],
                cfg,
            )
        };

        let (network_event_sender, network_event_receiver) = mpsc::channel(NETWORKING_CHANNEL_SIZE);
        let (swarm_cmd_sender, swarm_cmd_receiver) = mpsc::channel(NETWORKING_CHANNEL_SIZE);

        // Kademlia Behaviour
        let kademlia = {
            match record_store_cfg {
                Some(store_cfg) => {
                    let node_record_store = NodeRecordStore::with_config(
                        peer_id,
                        store_cfg,
                        network_event_sender.clone(),
                        swarm_cmd_sender.clone(),
                    );
                    #[cfg(feature = "open-metrics")]
                    let node_record_store = node_record_store
                        .set_record_count_metric(network_metrics.records_stored.clone());
                    let store = UnifiedRecordStore::Node(node_record_store);
                    debug!("Using Kademlia with NodeRecordStore!");
                    kad::Behaviour::with_config(peer_id, store, kad_cfg)
                }
                // no cfg provided for client
                None => {
                    let store = UnifiedRecordStore::Client(ClientRecordStore::default());
                    debug!("Using Kademlia with ClientRecordStore!");
                    kad::Behaviour::with_config(peer_id, store, kad_cfg)
                }
            }
        };

        #[cfg(feature = "local-discovery")]
        let mdns_config = mdns::Config {
            // lower query interval to speed up peer discovery
            // this increases traffic, but means we no longer have clients unable to connect
            // after a few minutes
            query_interval: Duration::from_secs(5),
            ..Default::default()
        };

        #[cfg(feature = "local-discovery")]
        let mdns = mdns::tokio::Behaviour::new(mdns_config, peer_id)?;

        // Identify Behaviour
        let identify_protocol_str = IDENTIFY_PROTOCOL_STR.to_string();
        info!("Building Identify with identify_protocol_str: {identify_protocol_str:?} and identify_version: {identify_version:?}");
        let identify = {
            let cfg = libp2p::identify::Config::new(identify_protocol_str, self.keypair.public())
                .with_agent_version(identify_version);
            libp2p::identify::Behaviour::new(cfg)
        };

        let main_transport = transport::build_transport(&self.keypair);

        let transport = if !self.local {
            debug!("Preventing non-global dials");
            // Wrap upper in a transport that prevents dialing local addresses.
            libp2p::core::transport::global_only::Transport::new(main_transport).boxed()
        } else {
            main_transport
        };

        let behaviour = NodeBehaviour {
            request_response,
            kademlia,
            identify,
            #[cfg(feature = "local-discovery")]
            mdns,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let swarm_config = libp2p::swarm::Config::with_tokio_executor()
            .with_idle_connection_timeout(CONNECTION_KEEP_ALIVE_TIMEOUT);
        #[cfg(target_arch = "wasm32")]
        let swarm_config = libp2p::swarm::Config::with_wasm_executor()
            .with_idle_connection_timeout(CONNECTION_KEEP_ALIVE_TIMEOUT);

        let swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

        let bootstrap = ContinuousBootstrap::new();
        let replication_fetcher = ReplicationFetcher::new(peer_id, network_event_sender.clone());

        let swarm_driver = SwarmDriver {
            swarm,
            self_peer_id: peer_id,
            local: self.local,
            listen_port: self.listen_addr.map(|addr| addr.port()),
            is_client,
            connected_peers: 0,
            bootstrap,
            close_group: Default::default(),
            replication_fetcher,
            #[cfg(feature = "open-metrics")]
            network_metrics,
            cmd_receiver: swarm_cmd_receiver,
            event_sender: network_event_sender,
            pending_get_closest_peers: Default::default(),
            pending_requests: Default::default(),
            pending_get_record: Default::default(),
            // We use 255 here which allows covering a network larger than 64k without any rotating.
            // This is based on the libp2p kad::kBuckets peers distribution.
            dialed_peers: CircularVec::new(255),
            network_discovery: NetworkDiscovery::new(&peer_id),
            bootstrap_peers: Default::default(),
            live_connected_peers: Default::default(),
            handling_statistics: Default::default(),
            handled_times: 0,
            hard_disk_write_error: 0,
            bad_nodes: Default::default(),
            bad_nodes_ongoing_verifications: Default::default(),
            quotes_history: Default::default(),
        };

        Ok((
            Network {
                swarm_cmd_sender,
                peer_id: Arc::new(peer_id),
                root_dir_path: Arc::new(self.root_dir),
                keypair: Arc::new(self.keypair),
            },
            network_event_receiver,
            swarm_driver,
        ))
    }
}

pub struct SwarmDriver {
    pub(crate) swarm: Swarm<NodeBehaviour>,
    pub(crate) self_peer_id: PeerId,
    pub(crate) local: bool,
    pub(crate) is_client: bool,
    /// The port that was set by the user
    pub(crate) listen_port: Option<u16>,
    pub(crate) connected_peers: usize,
    pub(crate) bootstrap: ContinuousBootstrap,
    /// The peers that are closer to our PeerId. Includes self.
    pub(crate) close_group: Vec<PeerId>,
    pub(crate) replication_fetcher: ReplicationFetcher,
    #[cfg(feature = "open-metrics")]
    #[allow(unused)]
    pub(crate) network_metrics: NetworkMetrics,

    cmd_receiver: mpsc::Receiver<SwarmCmd>,
    event_sender: mpsc::Sender<NetworkEvent>, // Use `self.send_event()` to send a NetworkEvent.

    /// Trackers for underlying behaviour related events
    pub(crate) pending_get_closest_peers: PendingGetClosest,
    pub(crate) pending_requests:
        HashMap<OutboundRequestId, Option<oneshot::Sender<Result<Response>>>>,
    pub(crate) pending_get_record: PendingGetRecord,
    /// A list of the most recent peers we have dialed ourselves.
    pub(crate) dialed_peers: CircularVec<PeerId>,
    // A list of random `PeerId` candidates that falls into kbuckets,
    // This is to ensure a more accurate network discovery.
    pub(crate) network_discovery: NetworkDiscovery,
    pub(crate) bootstrap_peers: BTreeMap<Option<u32>, HashSet<PeerId>>,
    // Peers that having live connection to. Any peer got contacted during kad network query
    // will have live connection established. And they may not appear in the RT.
    pub(crate) live_connected_peers: BTreeMap<ConnectionId, (PeerId, Instant)>,
    // Record the handling time of the recent 10 for each handling kind.
    handling_statistics: BTreeMap<String, Vec<Duration>>,
    handled_times: usize,
    pub(crate) hard_disk_write_error: usize,
    // 10 is the max number of issues per node we track to avoid mem leaks
    // the boolean flag to indicate whether the node is considered as bad or not
    pub(crate) bad_nodes: BTreeMap<PeerId, (Vec<(NodeIssue, Instant)>, bool)>,
    pub(crate) bad_nodes_ongoing_verifications: BTreeSet<PeerId>,
    pub(crate) quotes_history: BTreeMap<PeerId, PaymentQuote>,
}

impl SwarmDriver {
    /// Asynchronously drives the swarm event loop, handling events from both
    /// the swarm and command receiver. This function will run indefinitely,
    /// until the command channel is closed.
    ///
    /// The `tokio::select` macro is used to concurrently process swarm events
    /// and command receiver messages, ensuring efficient handling of multiple
    /// asynchronous tasks.
    pub async fn run(mut self) {
        let mut bootstrap_interval = interval(BOOTSTRAP_INTERVAL);
        let mut set_farthest_record_interval = interval(CLOSET_RECORD_CHECK_INTERVAL);

        loop {
            tokio::select! {
                swarm_event = self.swarm.select_next_some() => {
                    // logging for handling events happens inside handle_swarm_events
                    // otherwise we're rewriting match statements etc around this anwyay
                    if let Err(err) = self.handle_swarm_events(swarm_event) {
                        warn!("Error while handling swarm event: {err}");
                    }
                },
                some_cmd = self.cmd_receiver.recv() => match some_cmd {
                    Some(cmd) => {
                        let start = Instant::now();
                        let cmd_string = format!("{cmd:?}");
                        if let Err(err) = self.handle_cmd(cmd) {
                            warn!("Error while handling cmd: {err}");
                        }
                        trace!("SwarmCmd handled in {:?}: {cmd_string:?}", start.elapsed());
                    },
                    None =>  continue,
                },
                // runs every bootstrap_interval time
                _ = bootstrap_interval.tick() => {
                    if let Some(new_interval) = self.run_bootstrap_continuously(bootstrap_interval.period()).await {
                        bootstrap_interval = new_interval;
                    }
                }
                _ = set_farthest_record_interval.tick() => {
                    let closest_k_peers = self
                        .get_closest_k_value_local_peers();

                    if let Some(distance) = self.get_farthest_relevant_address_estimate(&closest_k_peers) {
                        // set any new distance to fathest record in the store
                        self.swarm.behaviour_mut().kademlia.store_mut().set_distance_range(distance);
                        // the distance range within the replication_fetcher shall be in sync as well
                        self.replication_fetcher.set_distance_range(distance);
                    }
                }
            }
        }
    }

    // --------------------------------------------
    // ---------- Crate helpers -------------------
    // --------------------------------------------

    /// Return a far address, close to but probably farther than our responsibilty range.
    /// This simply uses the closest k peers to estimate the farthest address as
    /// `REPLICATE_RANGE`th peer's address distance.
    fn get_farthest_relevant_address_estimate(
        &mut self,
        // Sorted list of closest k peers to our peer id.
        closest_k_peers: &[PeerId],
    ) -> Option<Distance> {
        // if we don't have enough peers we don't set the distance range yet.
        let mut farthest_distance = None;

        let our_address = NetworkAddress::from_peer(self.self_peer_id);

        // get REPLICATE_RANGE  + 2 peer's address distance
        // This is a rough estimate of the farthest address we might be responsible for.
        // We want this to be higher than actually necessary, so we retain more data
        // and can be sure to pass bad node checks
        if let Some(peer) = closest_k_peers.get(REPLICATE_RANGE) {
            let address = NetworkAddress::from_peer(*peer);
            let distance = our_address.distance(&address);
            farthest_distance = Some(distance);
        }

        farthest_distance
    }

    /// Sends an event after pushing it off thread so as to be non-blocking
    /// this is a wrapper around the `mpsc::Sender::send` call
    pub(crate) fn send_event(&self, event: NetworkEvent) {
        let event_sender = self.event_sender.clone();
        let capacity = event_sender.capacity();

        // push the event off thread so as to be non-blocking
        let _handle = spawn(async move {
            if capacity == 0 {
                warn!(
                    "NetworkEvent channel is full. Await capacity to send: {:?}",
                    event
                );
            }
            if let Err(error) = event_sender.send(event).await {
                error!("SwarmDriver failed to send event: {}", error);
            }
        });
    }

    // get all the peers from our local RoutingTable. Contains self
    pub(crate) fn get_all_local_peers(&mut self) -> Vec<PeerId> {
        let mut all_peers: Vec<PeerId> = vec![];
        for kbucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            for entry in kbucket.iter() {
                all_peers.push(entry.node.key.clone().into_preimage());
            }
        }
        all_peers.push(self.self_peer_id);
        all_peers
    }

    /// get closest k_value the peers from our local RoutingTable. Contains self.
    /// Is sorted for closeness to self.
    pub(crate) fn get_closest_k_value_local_peers(&mut self) -> Vec<PeerId> {
        let self_peer_id = self.self_peer_id.into();

        // get closest peers from buckets, sorted by increasing distance to us
        let peers = self
            .swarm
            .behaviour_mut()
            .kademlia
            .get_closest_local_peers(&self_peer_id)
            // Map KBucketKey<PeerId> to PeerId.
            .map(|key| key.into_preimage());

        // Start with our own PeerID and chain the closest.
        std::iter::once(self.self_peer_id)
            .chain(peers)
            // Limit ourselves to K_VALUE (20) peers.
            .take(K_VALUE.get())
            .collect()
    }

    /// Dials the given multiaddress. If address contains a peer ID, simultaneous
    /// dials to that peer are prevented.
    pub(crate) fn dial(&mut self, mut addr: Multiaddr) -> Result<(), DialError> {
        trace!(%addr, "Dialing manually");

        let peer_id = multiaddr_pop_p2p(&mut addr);
        let opts = match peer_id {
            Some(peer_id) => DialOpts::peer_id(peer_id)
                // If we have a peer ID, we can prevent simultaneous dials.
                .condition(PeerCondition::NotDialing)
                .addresses(vec![addr])
                .build(),
            None => DialOpts::unknown_peer_id().address(addr).build(),
        };

        self.swarm.dial(opts)
    }

    /// Dials with the `DialOpts` given.
    pub(crate) fn dial_with_opts(&mut self, opts: DialOpts) -> Result<(), DialError> {
        trace!(?opts, "Dialing manually");

        self.swarm.dial(opts)
    }

    /// Record one handling time.
    /// Log for every 100 received.
    pub(crate) fn log_handling(&mut self, handle_string: String, handle_time: Duration) {
        if handle_string.is_empty() {
            return;
        }

        match self.handling_statistics.entry(handle_string) {
            Entry::Occupied(mut entry) => {
                let records = entry.get_mut();
                records.push(handle_time);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![handle_time]);
            }
        }

        self.handled_times += 1;

        if self.handled_times >= 100 {
            self.handled_times = 0;

            let mut stats: Vec<(String, usize, Duration)> = self
                .handling_statistics
                .iter()
                .map(|(kind, durations)| {
                    let count = durations.len();
                    let avg_time = durations.iter().sum::<Duration>() / count as u32;
                    (kind.clone(), count, avg_time)
                })
                .collect();

            stats.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count in descending order

            trace!("SwarmDriver Handling Statistics: {:?}", stats);
            // now we've logged, lets clear the stats from the btreemap
            self.handling_statistics.clear();
        }
    }
}
