// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    circular_vec::CircularVec,
    cmd::SwarmCmd,
    error::{Error, Result},
    event::NetworkEvent,
    event::{GetRecordResultMap, NodeEvent},
    record_store::{
        ClientRecordStore, NodeRecordStore, NodeRecordStoreConfig,
        REPLICATION_INTERVAL_LOWER_BOUND, REPLICATION_INTERVAL_UPPER_BOUND,
    },
    record_store_api::UnifiedRecordStore,
    replication_fetcher::ReplicationFetcher,
    Network,
};
use futures::StreamExt;
#[cfg(feature = "quic")]
use libp2p::core::muxing::StreamMuxerBox;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    autonat,
    identity::Keypair,
    kad::{Kademlia, KademliaConfig, QueryId, Record},
    multiaddr::Protocol,
    request_response::{self, Config as RequestResponseConfig, ProtocolSupport, RequestId},
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, StreamProtocol, Swarm, SwarmBuilder},
    Multiaddr, PeerId, Transport,
};
#[cfg(feature = "quic")]
use libp2p_quic as quic;
use rand::Rng;
use sn_protocol::messages::{Request, Response};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    num::NonZeroUsize,
    path::PathBuf,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
/// The peer should be present among the CLOSE_GROUP_SIZE if we're fetching the close_group(peer)
pub const CLOSE_GROUP_SIZE: usize = 8;

/// What is the largest packet to send over the network.
/// Records larger than this will be rejected.
// TODO: revisit once utxo is in
pub const MAX_PACKET_SIZE: usize = 1024 * 1024 * 5; // the chunk size is 1mb, so should be higher than that to prevent failures, 5mb here to allow for DBC storage

// Timeout for requests sent/received through the request_response behaviour.
pub const REQUEST_TIMEOUT_DEFAULT_S: Duration = Duration::from_secs(30);
// Sets the keep-alive timeout of idle connections.
pub const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);

/// Our agent string has as a prefix that we can match against.
pub const IDENTIFY_AGENT_STR: &str = "safe/node/";

/// The suffix is the version of the node.
pub const SN_NODE_VERSION_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));
/// / first version for the req/response protocol
pub const REQ_RESPONSE_VERSION_STR: &str = concat!("/safe/node/", env!("CARGO_PKG_VERSION"));

/// The suffix is the version of the client.
pub const IDENTIFY_CLIENT_VERSION_STR: &str = concat!("safe/client/", env!("CARGO_PKG_VERSION"));
pub const IDENTIFY_PROTOCOL_STR: &str = concat!("safe/", env!("CARGO_PKG_VERSION"));

/// Duration to wait for verification
pub const REVERIFICATION_WAIT_TIME_S: std::time::Duration = std::time::Duration::from_secs(3);
/// Number of attempts to verify a record
pub const VERIFICATION_ATTEMPTS: usize = 5;

/// Number of attempts to re-put a record
pub const PUT_RECORD_RETRIES: usize = 10;

pub const NETWORKING_CHANNEL_SIZE: usize = 10_000;
/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    CLOSE_GROUP_SIZE / 2 + 1
}

/// NodeBehaviour struct
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NodeEvent")]
pub(super) struct NodeBehaviour {
    pub(super) request_response: request_response::cbor::Behaviour<Request, Response>,
    pub(super) kademlia: Kademlia<UnifiedRecordStore>,
    #[cfg(feature = "local-discovery")]
    pub(super) mdns: mdns::tokio::Behaviour,
    pub(super) identify: libp2p::identify::Behaviour,
    pub(super) autonat: Toggle<autonat::Behaviour>,
}

type PendingGetClosest = HashMap<QueryId, (oneshot::Sender<HashSet<PeerId>>, HashSet<PeerId>)>;
type PendingGetRecord = HashMap<QueryId, (oneshot::Sender<Result<Record>>, GetRecordResultMap)>;

/// `SwarmDriver` is responsible for managing the swarm of peers, handling
pub struct SwarmDriver {
    pub(crate) self_peer_id: PeerId,
    pub(crate) swarm: Swarm<NodeBehaviour>,
    pub(crate) cmd_receiver: mpsc::Receiver<SwarmCmd>,
    // Do not access this directly to send. Use `send_event` instead.
    // This wraps the call and pushes it off thread so as to be non-blocking
    pub(crate) event_sender: mpsc::Sender<NetworkEvent>,
    pub(crate) pending_get_closest_peers: PendingGetClosest,
    pub(crate) pending_requests: HashMap<RequestId, Option<oneshot::Sender<Result<Response>>>>,
    pub(crate) pending_get_record: PendingGetRecord,
    pub(crate) replication_fetcher: ReplicationFetcher,
    pub(crate) local: bool,
    /// A list of the most recent peers we have dialed ourselves.
    pub(crate) dialed_peers: CircularVec<PeerId>,
    pub(crate) unroutable_peers: CircularVec<PeerId>,
    /// The peers that are closer to our PeerId. Includes self.
    pub(crate) close_group: Vec<PeerId>,
    /// Is the bootstrap process currently running
    pub(crate) bootstrap_ongoing: bool,
    pub(crate) is_client: bool,
}

impl SwarmDriver {
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
    #[allow(clippy::result_large_err)]
    pub fn new(
        keypair: Keypair,
        addr: SocketAddr,
        local: bool,
        root_dir: PathBuf,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, Self)> {
        // get a random integer between REPLICATION_INTERVAL_LOWER_BOUND and REPLICATION_INTERVAL_UPPER_BOUND
        let replication_interval = rand::thread_rng()
            .gen_range(REPLICATION_INTERVAL_LOWER_BOUND..REPLICATION_INTERVAL_UPPER_BOUND);

        let mut kad_cfg = KademliaConfig::default();
        let _ = kad_cfg
            .set_kbucket_inserts(libp2p::kad::KademliaBucketInserts::Manual)
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
            .set_query_timeout(Duration::from_secs(5 * 60))
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
            let storage_dir_path = root_dir.join("record_store");
            if let Err(error) = std::fs::create_dir_all(&storage_dir_path) {
                return Err(Error::FailedToCreateRecordStoreDir {
                    path: storage_dir_path,
                    source: error,
                });
            }
            NodeRecordStoreConfig {
                max_value_bytes: MAX_PACKET_SIZE, // TODO, does this need to be _less_ than MAX_PACKET_SIZE
                storage_dir: storage_dir_path,
                replication_interval,
                ..Default::default()
            }
        };

        let (network, events_receiver, mut swarm_driver) = Self::with(
            root_dir,
            keypair,
            kad_cfg,
            Some(store_cfg),
            local,
            false,
            replication_interval,
            None,
            ProtocolSupport::Full,
            SN_NODE_VERSION_STR.to_string(),
        )?;

        // Listen on the provided address
        #[cfg(not(feature = "quic"))]
        let addr = Multiaddr::from(addr.ip()).with(Protocol::Tcp(addr.port()));

        #[cfg(feature = "quic")]
        let addr = Multiaddr::from(addr.ip())
            .with(Protocol::Udp(addr.port()))
            .with(Protocol::QuicV1);

        let _listener_id = swarm_driver
            .swarm
            .listen_on(addr)
            .expect("Failed to listen on the provided address");

        Ok((network, events_receiver, swarm_driver))
    }

    /// Same as `new` API but creates the network components in client mode
    #[allow(clippy::result_large_err)]
    pub fn new_client(
        local: bool,
        request_timeout: Option<Duration>,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, Self)> {
        // Create a Kademlia behaviour for client mode, i.e. set req/resp protocol
        // to outbound-only mode and don't listen on any address
        let mut kad_cfg = KademliaConfig::default(); // default query timeout is 60 secs

        // 1mb packet size
        let _ = kad_cfg
            .set_max_packet_size(MAX_PACKET_SIZE)
            // Require iterative queries to use disjoint paths for increased resiliency in the presence of potentially adversarial nodes.
            .disjoint_query_paths(true)
            // How many nodes _should_ store data.
            .set_replication_factor(
                NonZeroUsize::new(CLOSE_GROUP_SIZE).ok_or_else(|| Error::InvalidCloseGroupSize)?,
            );

        Self::with(
            std::env::temp_dir(),
            Keypair::generate_ed25519(),
            kad_cfg,
            None,
            local,
            true,
            // Nonsense interval for the client which never replicates
            Duration::from_secs(1000),
            request_timeout,
            ProtocolSupport::Outbound,
            IDENTIFY_CLIENT_VERSION_STR.to_string(),
        )
    }

    /// Sends an event after pushing it off thread so as to be non-blocking
    /// this is a wrapper around the `mpsc::Sender::send` call
    pub(crate) fn send_event(&self, event: NetworkEvent) {
        let event_sender = self.event_sender.clone();
        let capacity = event_sender.capacity();

        if capacity == 0 {
            warn!(
                "NetworkEvent channel is full. Dropping NetworkEvent: {:?}",
                event
            );

            // Lets error out just now.
            return;
        }

        // push the event off thread so as to be non-blocking
        let _handle = tokio::spawn(async move {
            if let Err(error) = event_sender.send(event).await {
                error!("SwarmDriver failed to send event: {}", error);
            }
        });
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    /// Private helper to create the network components with the provided config and req/res behaviour
    fn with(
        root_dir_path: PathBuf,
        keypair: Keypair,
        kad_cfg: KademliaConfig,
        record_store_cfg: Option<NodeRecordStoreConfig>,
        local: bool,
        is_client: bool,
        replication_interval: Duration,
        request_response_timeout: Option<Duration>,
        req_res_protocol: ProtocolSupport,
        identify_version: String,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, Self)> {
        let peer_id = PeerId::from(keypair.public());
        info!("Node (PID: {}) with PeerId: {peer_id}", std::process::id());
        info!("PeerId: {peer_id} has replication interval of {replication_interval:?}");

        // RequestResponse Behaviour
        let request_response = {
            let mut cfg = RequestResponseConfig::default();
            let _ = cfg
                .set_request_timeout(request_response_timeout.unwrap_or(REQUEST_TIMEOUT_DEFAULT_S))
                .set_connection_keep_alive(CONNECTION_KEEP_ALIVE_TIMEOUT);

            request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new(REQ_RESPONSE_VERSION_STR),
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
                    let store = UnifiedRecordStore::Node(node_record_store);
                    debug!("Using Kademlia with NodeRecordStore!");
                    Kademlia::with_config(peer_id, store, kad_cfg)
                }
                // no cfg provided for client
                None => {
                    let store = UnifiedRecordStore::Client(ClientRecordStore::default());
                    debug!("Using Kademlia with ClientRecordStore!");
                    Kademlia::with_config(peer_id, store, kad_cfg)
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
            let cfg =
                libp2p::identify::Config::new(IDENTIFY_PROTOCOL_STR.to_string(), keypair.public())
                    .with_agent_version(identify_version);
            libp2p::identify::Behaviour::new(cfg)
        };

        // Transport
        #[cfg(not(feature = "quic"))]
        let mut transport = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(
                libp2p::noise::Config::new(&keypair)
                    .expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(libp2p::yamux::Config::default())
            .boxed();

        #[cfg(feature = "quic")]
        let mut transport = libp2p_quic::tokio::Transport::new(quic::Config::new(&keypair))
            .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
            .boxed();

        if !local {
            debug!("Preventing non-global dials");
            // Wrap TCP or UDP in a transport that prevents dialing local addresses.
            transport = libp2p::core::transport::global_only::Transport::new(transport).boxed();
        }

        // Disable AutoNAT if we are either running locally or a client.
        let autonat = if !local && !is_client {
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
        };
        let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();

        let (swarm_cmd_sender, swarm_cmd_receiver) = mpsc::channel(NETWORKING_CHANNEL_SIZE);
        let swarm_driver = Self {
            self_peer_id: peer_id,
            swarm,
            cmd_receiver: swarm_cmd_receiver,
            event_sender: network_event_sender,
            pending_get_closest_peers: Default::default(),
            pending_requests: Default::default(),
            pending_get_record: Default::default(),
            replication_fetcher: Default::default(),
            local,
            // We use 63 here, as in practice the capacity will be rounded to the nearest 2^n-1.
            // Source: https://users.rust-lang.org/t/the-best-ring-buffer-library/58489/8
            // 63 will mean at least 63 most recent peers we have dialed, which should be allow for enough time for the
            // `identify` protocol to kick in and get them in the routing table.
            dialed_peers: CircularVec::new(63),
            unroutable_peers: CircularVec::new(127),
            close_group: Default::default(),
            bootstrap_ongoing: false,
            is_client,
        };

        Ok((
            Network {
                swarm_cmd_sender,
                peer_id,
                root_dir_path,
                keypair,
            },
            network_event_receiver,
            swarm_driver,
        ))
    }

    /// Asynchronously drives the swarm event loop, handling events from both
    /// the swarm and command receiver. This function will run indefinitely,
    /// until the command channel is closed.
    ///
    /// The `tokio::select` macro is used to concurrently process swarm events
    /// and command receiver messages, ensuring efficient handling of multiple
    /// asynchronous tasks.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                swarm_event = self.swarm.select_next_some() => {
                    if let Err(err) = self.handle_swarm_events(swarm_event) {
                        warn!("Error while handling swarm event: {err}");
                    }
                },
                some_cmd = self.cmd_receiver.recv() => match some_cmd {
                    Some(cmd) => {
                        if let Err(err) = self.handle_cmd(cmd) {
                            warn!("Error while handling cmd: {err}");
                        }
                    },
                    None =>  continue,
                },
            }
        }
    }
}
