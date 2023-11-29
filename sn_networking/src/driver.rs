// Copyright 2023 MaidSafe.net limited.
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
    error::{Error, Result},
    event::NetworkEvent,
    event::{GetRecordResultMap, NodeEvent},
    multiaddr_pop_p2p,
    network_discovery::NetworkDiscovery,
    record_store::{ClientRecordStore, NodeRecordStore, NodeRecordStoreConfig},
    record_store_api::UnifiedRecordStore,
    replication_fetcher::ReplicationFetcher,
    GetQuorum, Network, CLOSE_GROUP_SIZE,
};
use futures::StreamExt;
#[cfg(feature = "quic")]
use libp2p::core::muxing::StreamMuxerBox;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
#[cfg(feature = "quic")]
use libp2p::quic;
use libp2p::{
    autonat,
    identity::Keypair,
    kad::{self, QueryId, Record, K_VALUE},
    multiaddr::Protocol,
    request_response::{self, Config as RequestResponseConfig, OutboundRequestId, ProtocolSupport},
    swarm::{
        behaviour::toggle::Toggle, dial_opts::DialOpts, DialError, NetworkBehaviour,
        StreamProtocol, Swarm,
    },
    Multiaddr, PeerId, Transport,
};
#[cfg(feature = "open-metrics")]
use prometheus_client::registry::Registry;
use sn_protocol::{
    messages::{Request, Response},
    NetworkAddress, PrettyPrintKBucketKey,
};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    num::NonZeroUsize,
    path::PathBuf,
    time::Duration,
};
use tiny_keccak::{Hasher, Sha3};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

/// List of expected record holders to be verified.
pub(super) type ExpectedHoldersList = HashSet<PeerId>;

/// The ways in which the Get Closest queries are used.
pub(crate) enum PendingGetClosestType {
    /// The network discovery method is present at the networking layer
    /// Thus we can just process the queries made by NetworkDiscovery without using any channels
    NetworkDiscovery,
    /// These are queries made by a function at the upper layers and contains a channel to send the result back.
    FunctionCall(oneshot::Sender<HashSet<PeerId>>),
}
type PendingGetClosest = HashMap<QueryId, (PendingGetClosestType, HashSet<PeerId>)>;
type PendingGetRecord = HashMap<
    QueryId,
    (
        oneshot::Sender<Result<Record>>,
        GetRecordResultMap,
        GetQuorum,
        ExpectedHoldersList,
    ),
>;

/// What is the largest packet to send over the network.
/// Records larger than this will be rejected.
// TODO: revisit once cashnote_redemption is in
const MAX_PACKET_SIZE: usize = 1024 * 1024 * 5; // the chunk size is 1mb, so should be higher than that to prevent failures, 5mb here to allow for CashNote storage

// Timeout for requests sent/received through the request_response behaviour.
const REQUEST_TIMEOUT_DEFAULT_S: Duration = Duration::from_secs(30);
// Sets the timeout of idle connections.
const IDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// The suffix is the version of the node.
const SN_NODE_VERSION_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));
/// / first version for the req/response protocol
const REQ_RESPONSE_VERSION_STR: &str = concat!("/safe/node/", env!("CARGO_PKG_VERSION"));

/// The suffix is the version of the client.
const IDENTIFY_CLIENT_VERSION_STR: &str = concat!("safe/client/", env!("CARGO_PKG_VERSION"));
const IDENTIFY_PROTOCOL_STR: &str = concat!("safe/", env!("CARGO_PKG_VERSION"));

const NETWORKING_CHANNEL_SIZE: usize = 10_000;

/// Time before a Kad query times out if no response is received
const KAD_QUERY_TIMEOUT_S: Duration = Duration::from_secs(25);

// Protocol support shall be downward compatible for patch only version update.
// i.e. versions of `A.B.X` shall be considered as a same protocol of `A.B`
pub(crate) fn truncate_patch_version(full_str: &str) -> &str {
    if full_str.matches('.').count() == 2 {
        match full_str.rfind('.') {
            Some(pos) => &full_str[..pos],
            None => full_str,
        }
    } else {
        full_str
    }
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
    pub(super) autonat: Toggle<autonat::Behaviour>,
    pub(super) gossipsub: Toggle<libp2p::gossipsub::Behaviour>,
}

#[derive(Debug)]
pub struct NetworkBuilder {
    keypair: Keypair,
    local: bool,
    root_dir: PathBuf,
    listen_addr: Option<SocketAddr>,
    enable_gossip: bool,
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
            enable_gossip: false,
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

    /// Enable gossip for the network
    pub fn enable_gossip(&mut self) {
        self.enable_gossip = true;
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
                NonZeroUsize::new(CLOSE_GROUP_SIZE).ok_or_else(|| Error::InvalidCloseGroupSize)?,
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
                return Err(Error::FailedToCreateRecordStoreDir {
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
            truncate_patch_version(SN_NODE_VERSION_STR).to_string(),
        )?;

        // Listen on the provided address
        let listen_addr = listen_addr.ok_or(Error::ListenAddressNotProvided)?;
        #[cfg(not(feature = "quic"))]
        let listen_addr = Multiaddr::from(listen_addr.ip()).with(Protocol::Tcp(listen_addr.port()));

        #[cfg(feature = "quic")]
        let listen_addr = Multiaddr::from(listen_addr.ip())
            .with(Protocol::Udp(listen_addr.port()))
            .with(Protocol::QuicV1);

        let _listener_id = swarm_driver
            .swarm
            .listen_on(listen_addr)
            .expect("Failed to listen on the provided address");

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
                NonZeroUsize::new(CLOSE_GROUP_SIZE).ok_or_else(|| Error::InvalidCloseGroupSize)?,
            );

        let (network, net_event_recv, driver) = self.build(
            kad_cfg,
            None,
            true,
            ProtocolSupport::Outbound,
            truncate_patch_version(IDENTIFY_CLIENT_VERSION_STR).to_string(),
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
        info!("Node (PID: {}) with PeerId: {peer_id}", std::process::id());
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

            request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new(truncate_patch_version(REQ_RESPONSE_VERSION_STR)),
                    req_res_protocol,
                )],
                cfg,
            )
        };

        let (network_event_sender, network_event_receiver) = mpsc::channel(NETWORKING_CHANNEL_SIZE);

        // Kademlia Behaviour
        let kademlia = {
            match record_store_cfg {
                Some(store_cfg) => {
                    let node_record_store = NodeRecordStore::with_config(
                        peer_id,
                        store_cfg,
                        Some(network_event_sender.clone()),
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
        let identify = {
            let cfg = libp2p::identify::Config::new(
                truncate_patch_version(IDENTIFY_PROTOCOL_STR).to_string(),
                self.keypair.public(),
            )
            .with_agent_version(identify_version);
            libp2p::identify::Behaviour::new(cfg)
        };

        // Transport
        #[cfg(not(feature = "quic"))]
        let mut transport = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(
                libp2p::noise::Config::new(&self.keypair)
                    .expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(libp2p::yamux::Config::default())
            .boxed();

        #[cfg(feature = "quic")]
        let mut transport = libp2p::quic::tokio::Transport::new(quic::Config::new(&self.keypair))
            .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
            .boxed();

        let gossipsub = if self.enable_gossip {
            // Gossipsub behaviour
            let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
                // disable sending to ALL_PEERS subscribed to a topic, which is the default behaviour
                .flood_publish(false)
                // we don't currently require source peer id and/or signing
                .validation_mode(libp2p::gossipsub::ValidationMode::Permissive)
                // we use the hash of the msg content as the msg id to deduplicate them
                .message_id_fn(|msg| {
                    let mut sha3 = Sha3::v256();
                    let mut msg_id = [0; 32];
                    sha3.update(&msg.data);
                    sha3.finalize(&mut msg_id);
                    msg_id.into()
                })
                // set the heartbeat interval to be higher than default 1sec
                .heartbeat_interval(Duration::from_secs(5))
                .build()
                .map_err(|err| Error::GossipsubConfigError(err.to_string()))?;

            // Set the message authenticity
            let message_authenticity = libp2p::gossipsub::MessageAuthenticity::Anonymous;

            // build a gossipsub network behaviour
            let gossipsub: libp2p::gossipsub::Behaviour =
                libp2p::gossipsub::Behaviour::new(message_authenticity, gossipsub_config)
                    .expect("Failed to instantiate Gossipsub behaviour.");
            Some(gossipsub)
        } else {
            None
        };

        let gossipsub = Toggle::from(gossipsub);

        if !self.local {
            debug!("Preventing non-global dials");
            // Wrap TCP or UDP in a transport that prevents dialing local addresses.
            transport = libp2p::core::transport::global_only::Transport::new(transport).boxed();
        }

        // Disable AutoNAT if we are either running locally or a client.
        let autonat = if !self.local && !is_client {
            let cfg = libp2p::autonat::Config {
                // Defaults to 15. But we want to be a little quicker on checking for our NAT status.
                boot_delay: Duration::from_secs(3),
                // The time to wait for an AutoNAT server to respond.
                // This is increased due to the fact that a server might take a while before it determines we are unreachable.
                // There likely is a bug in libp2p AutoNAT that causes us to use this workaround.
                // E.g. a TCP connection might only time out after 2 minutes, thus taking the server 2 minutes to determine we are unreachable.
                timeout: Duration::from_secs(301),
                // Defaults to 90. If we get a timeout and only have one server, we want to try again with the same server.
                throttle_server_period: Duration::from_secs(15),
                ..Default::default()
            };
            Some(libp2p::autonat::Behaviour::new(peer_id, cfg))
        } else {
            None
        };
        let autonat = Toggle::from(autonat);

        let behaviour = NodeBehaviour {
            request_response,
            kademlia,
            identify,
            #[cfg(feature = "local-discovery")]
            mdns,
            autonat,
            gossipsub,
        };
        let swarm_config = libp2p::swarm::Config::with_tokio_executor()
            .with_idle_connection_timeout(IDLE_CONNECTION_TIMEOUT);

        let swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

        let (swarm_cmd_sender, swarm_cmd_receiver) = mpsc::channel(NETWORKING_CHANNEL_SIZE);
        let swarm_driver = SwarmDriver {
            swarm,
            self_peer_id: peer_id,
            local: self.local,
            is_client,
            connected_peers: 0,
            bootstrap: ContinuousBootstrap::new(),
            close_group: Default::default(),
            replication_fetcher: ReplicationFetcher::new(peer_id),
            #[cfg(feature = "open-metrics")]
            network_metrics,
            cmd_receiver: swarm_cmd_receiver,
            event_sender: network_event_sender,
            pending_get_closest_peers: Default::default(),
            pending_requests: Default::default(),
            pending_get_record: Default::default(),
            // We use 63 here, as in practice the capacity will be rounded to the nearest 2^n-1.
            // Source: https://users.rust-lang.org/t/the-best-ring-buffer-library/58489/8
            // 63 will mean at least 63 most recent peers we have dialed, which should be allow for enough time for the
            // `identify` protocol to kick in and get them in the routing table.
            dialed_peers: CircularVec::new(63),
            is_gossip_handler: false,
            network_discovery: NetworkDiscovery::new(&peer_id),
        };

        Ok((
            Network {
                swarm_cmd_sender,
                peer_id,
                root_dir_path: self.root_dir,
                keypair: self.keypair,
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
    pub(crate) connected_peers: usize,
    pub(crate) bootstrap: ContinuousBootstrap,
    /// The peers that are closer to our PeerId. Includes self.
    pub(crate) close_group: Vec<PeerId>,
    pub(crate) replication_fetcher: ReplicationFetcher,
    #[cfg(feature = "open-metrics")]
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
    // For normal nodes, though they subscribe to the gossip topic
    // (to ensure no miss-up by carrying out libp2p low level gossip forwarding),
    // they are not supposed to process the gossip msg that received from libp2p.
    pub(crate) is_gossip_handler: bool,
    // A list of random `PeerId` candidates that falls into kbuckets,
    // This is to ensure a more accurate network discovery.
    pub(crate) network_discovery: NetworkDiscovery,
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
        let mut bootstrap_interval = tokio::time::interval(BOOTSTRAP_INTERVAL);
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
                        let start = std::time::Instant::now();
                        let cmd_string = format!("{:?}", cmd);
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
            }
        }
    }

    // --------------------------------------------
    // ---------- Crate helpers -------------------
    // --------------------------------------------

    /// Sends an event after pushing it off thread so as to be non-blocking
    /// this is a wrapper around the `mpsc::Sender::send` call
    pub(crate) fn send_event(&self, event: NetworkEvent) {
        let event_sender = self.event_sender.clone();
        let capacity = event_sender.capacity();

        // push the event off thread so as to be non-blocking
        let _handle = tokio::spawn(async move {
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

    // get closest k_value the peers from our local RoutingTable. Contains self
    pub(crate) fn get_closest_k_value_local_peers(&mut self) -> HashSet<PeerId> {
        let self_peer_id = self.self_peer_id.into();
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
            Some(peer_id) => DialOpts::peer_id(peer_id).addresses(vec![addr]).build(),
            None => DialOpts::unknown_peer_id().address(addr).build(),
        };

        self.swarm.dial(opts)
    }

    /// Dials with the `DialOpts` given.
    pub(crate) fn dial_with_opts(&mut self, opts: DialOpts) -> Result<(), DialError> {
        trace!(?opts, "Dialing manually");

        self.swarm.dial(opts)
    }
}
