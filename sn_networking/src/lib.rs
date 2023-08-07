// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod circular_vec;
mod cmd;
mod error;
mod event;
mod msg;
mod record_store;
mod replication_fetcher;

pub use self::{
    cmd::SwarmLocalState,
    error::Error,
    event::{MsgResponder, NetworkEvent},
};

use self::{
    circular_vec::CircularVec,
    cmd::SwarmCmd,
    error::Result,
    event::{GetRecordResultMap, NodeBehaviour},
    record_store::{
        DiskBackedRecordStore, DiskBackedRecordStoreConfig, REPLICATION_INTERVAL_LOWER_BOUND,
        REPLICATION_INTERVAL_UPPER_BOUND,
    },
    replication_fetcher::ReplicationFetcher,
};
use futures::{future::select_all, StreamExt};
use itertools::Itertools;
#[cfg(feature = "local-discovery")]
use libp2p::mdns;
use libp2p::{
    identity::Keypair,
    kad::{
        KBucketDistance as Distance, KBucketKey, Kademlia, KademliaConfig, QueryId, Record,
        RecordKey,
    },
    multiaddr::Protocol,
    request_response::{self, Config as RequestResponseConfig, ProtocolSupport, RequestId},
    swarm::{behaviour::toggle::Toggle, StreamProtocol, Swarm, SwarmBuilder},
    Multiaddr, PeerId, Transport,
};
use rand::Rng;
use sn_dbc::Token;
use sn_protocol::{
    messages::{Query, QueryResponse, Request, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
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

// Timeout for requests sent/received through the request_response behaviour.
const REQUEST_TIMEOUT_DEFAULT_S: Duration = Duration::from_secs(30);
// Sets the keep-alive timeout of idle connections.
const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);

/// Our agent string has as a prefix that we can match against.
pub const IDENTIFY_AGENT_STR: &str = "safe/node/";

/// The suffix is the version of the node.
const SN_NODE_VERSION_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));
/// / first version for the req/response protocol
const REQ_RESPONSE_VERSION_STR: &str = concat!("/safe/node/", env!("CARGO_PKG_VERSION"));

/// The suffix is the version of the client.
const IDENTIFY_CLIENT_VERSION_STR: &str = concat!("safe/client/", env!("CARGO_PKG_VERSION"));
const IDENTIFY_PROTOCOL_STR: &str = concat!("safe/", env!("CARGO_PKG_VERSION"));

/// Duration to wait for verification
const REVERIFICATION_WAIT_TIME_S: std::time::Duration = std::time::Duration::from_secs(3);
/// Number of attempts to verify a record
const VERIFICATION_ATTEMPTS: usize = 30;

/// Number of attempts to re-put a record
const PUT_RECORD_RETRIES: usize = 3;

const NETWORKING_CHANNEL_SIZE: usize = 10_000;
/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    CLOSE_GROUP_SIZE / 2 + 1
}

type PendingGetClosest = HashMap<QueryId, (oneshot::Sender<HashSet<PeerId>>, HashSet<PeerId>)>;
type PendingGetRecord = HashMap<QueryId, (oneshot::Sender<Result<Record>>, GetRecordResultMap)>;

/// `SwarmDriver` is responsible for managing the swarm of peers, handling
/// swarm events, processing commands, and maintaining the state of pending
/// tasks. It serves as the core component for the network functionality.
pub struct SwarmDriver {
    self_peer_id: PeerId,
    swarm: Swarm<NodeBehaviour>,
    cmd_receiver: mpsc::Receiver<SwarmCmd>,
    // Do not access this directly to send. Use `send_event` instead.
    // This wraps the call and pushes it off thread so as to be non-blocking
    event_sender: mpsc::Sender<NetworkEvent>,
    pending_get_closest_peers: PendingGetClosest,
    pending_requests: HashMap<RequestId, Option<oneshot::Sender<Result<Response>>>>,
    pending_get_record: PendingGetRecord,
    replication_fetcher: ReplicationFetcher,
    local: bool,
    /// A list of the most recent peers we have dialed ourselves.
    dialed_peers: CircularVec<PeerId>,
    /// The peers that are closer to our PeerId. Includes self.
    close_group: Vec<PeerId>,
    is_client: bool,
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
            .set_max_packet_size(1024 * 1024)
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

        let (network, events_receiver, mut swarm_driver) = Self::with(
            root_dir,
            keypair,
            kad_cfg,
            local,
            false,
            replication_interval,
            None,
            ProtocolSupport::Full,
            SN_NODE_VERSION_STR.to_string(),
        )?;

        // Listen on the provided address
        let addr = Multiaddr::from(addr.ip()).with(Protocol::Tcp(addr.port()));
        let _listener_id = swarm_driver
            .swarm
            .listen_on(addr)
            .expect("Failed to listen on the provided address");

        Ok((network, events_receiver, swarm_driver))
    }

    /// Same as `new` API but creates the network components in client mode
    pub fn new_client(
        local: bool,
        request_timeout: Option<Duration>,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, Self)> {
        // Create a Kademlia behaviour for client mode, i.e. set req/resp protocol
        // to outbound-only mode and don't listen on any address
        let mut kad_cfg = KademliaConfig::default(); // default query timeout is 60 secs

        // 1mb packet size
        let _ = kad_cfg
            .set_max_packet_size(1024 * 1024)
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
    fn send_event(&self, event: NetworkEvent) {
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

    #[allow(clippy::too_many_arguments)]
    /// Private helper to create the network components with the provided config and req/res behaviour
    fn with(
        root_dir_path: PathBuf,
        keypair: Keypair,
        kad_cfg: KademliaConfig,
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
            // Configures the disk_store to store records under the provided path and increase the max record size
            let storage_dir_path = root_dir_path.join("record_store");
            if let Err(error) = std::fs::create_dir_all(&storage_dir_path) {
                return Err(Error::FailedToCreateRecordStoreDir {
                    path: storage_dir_path,
                    source: error,
                });
            }

            let store_cfg = DiskBackedRecordStoreConfig {
                max_value_bytes: 1024 * 1024,
                storage_dir: storage_dir_path,
                replication_interval,
                ..Default::default()
            };

            Kademlia::with_config(
                peer_id,
                DiskBackedRecordStore::with_config(
                    peer_id,
                    store_cfg,
                    Some(network_event_sender.clone()),
                ),
                kad_cfg,
            )
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
        let mut transport = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(
                libp2p::noise::Config::new(&keypair)
                    .expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(libp2p::yamux::Config::default())
            .boxed();

        if !local {
            debug!("Preventing non-global dials");
            // Wrap TCP in a transport that prevents dialing local addresses.
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
            close_group: Default::default(),
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

/// Sort the provided peers by their distance to the given `NetworkAddress`.
/// Return with the closest expected number of entries if has.
pub fn sort_peers_by_address(
    peers: Vec<PeerId>,
    address: &NetworkAddress,
    expected_entries: usize,
) -> Result<Vec<PeerId>> {
    sort_peers_by_key(peers, &address.as_kbucket_key(), expected_entries)
}

/// Sort the provided peers by their distance to the given `KBucketKey`.
/// Return with the closest expected number of entries if has.
pub fn sort_peers_by_key<T>(
    mut peers: Vec<PeerId>,
    key: &KBucketKey<T>,
    expected_entries: usize,
) -> Result<Vec<PeerId>> {
    peers.sort_by(|a, b| {
        let a = NetworkAddress::from_peer(*a);
        let b = NetworkAddress::from_peer(*b);
        key.distance(&a.as_kbucket_key())
            .cmp(&key.distance(&b.as_kbucket_key()))
    });
    let peers: Vec<PeerId> = peers.iter().take(expected_entries).cloned().collect();

    if CLOSE_GROUP_SIZE > peers.len() {
        warn!("Not enough peers in the k-bucket to satisfy the request");
        return Err(Error::NotEnoughPeers {
            found: peers.len(),
            required: CLOSE_GROUP_SIZE,
        });
    }
    Ok(peers)
}

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct Network {
    pub swarm_cmd_sender: mpsc::Sender<SwarmCmd>,
    pub peer_id: PeerId,
    pub root_dir_path: PathBuf,
    keypair: Keypair,
}

impl Network {
    /// Signs the given data with the node's keypair.
    pub fn sign(&self, msg: &[u8]) -> Result<Vec<u8>> {
        self.keypair.sign(msg).map_err(Error::from)
    }

    ///  Listen for incoming connections on the given address.
    pub async fn start_listening(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::StartListening { addr, sender })?;
        receiver.await?
    }

    /// Dial the given peer at the given address.
    pub async fn dial(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::Dial { addr, sender })?;
        receiver.await?
    }

    /// Returns the closest peers to the given `XorName`, sorted by their distance to the xor_name.
    /// Excludes the client's `PeerId` while calculating the closest peers.
    pub async fn client_get_closest_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        self.get_closest_peers(key, true).await
    }

    /// Returns the closest peers to the given `NetworkAddress`, sorted by their distance to the key.
    ///
    /// Includes our node's `PeerId` while calculating the closest peers.
    pub async fn node_get_closest_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        self.get_closest_peers(key, false).await
    }

    /// Returns the closest peers to the given `NetworkAddress` that is fetched from the local
    /// Routing Table. It is ordered by increasing distance of the peers
    /// Note self peer_id is not included in the result.
    pub async fn get_closest_local_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestLocalPeers {
            key: key.clone(),
            sender,
        })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Returns all the PeerId from all the KBuckets from our local Routing Table
    /// Also contains our own PeerId.
    pub async fn get_all_local_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalPeers { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Returns the current set of members in our close group. This list is sorted in ascending order based on the
    ///  distance to self. The first element is self.
    pub async fn get_our_close_group(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetOurCloseGroup { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Send `Request` to the closest peers. If `self` is among the closest_peers, the `Request` is
    /// forwarded to itself and handled. Then a corresponding `Response` is created and is
    /// forwarded to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn node_send_to_closest(&self, request: &Request) -> Result<Vec<Result<Response>>> {
        debug!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst()
        );
        let closest_peers = self.node_get_closest_peers(&request.dst()).await?;

        Ok(self
            .send_and_get_responses(closest_peers, request, true)
            .await)
    }

    /// Send `Request` to the closest peers and ignore reply
    /// If `self` is among the closest_peers, the `Request` is
    /// forwarded to itself and handled. Then a corresponding `Response` is created and is
    /// forwarded to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn send_req_no_reply_to_closest(&self, request: &Request) -> Result<()> {
        debug!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst()
        );
        let closest_peers = self.node_get_closest_peers(&request.dst()).await?;
        for peer in closest_peers {
            self.send_req_ignore_reply(request.clone(), peer)?;
        }
        Ok(())
    }

    /// Send `Request` to the closest peers to self
    pub async fn send_req_no_reply_to_self_closest(&self, request: &Request) -> Result<()> {
        debug!("Sending {request:?} to self closest peers.");
        // Using `client_get_closest_peers` to filter self out.
        let closest_peers = self.client_get_closest_peers(&request.dst()).await?;
        for peer in closest_peers {
            self.send_req_ignore_reply(request.clone(), peer)?;
        }
        Ok(())
    }

    /// Send `Request` to the closest peers. `Self` is not present among the recipients.
    pub async fn client_send_to_closest(
        &self,
        request: &Request,
        expect_all_responses: bool,
    ) -> Result<Vec<Result<Response>>> {
        debug!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst()
        );
        let closest_peers = self.client_get_closest_peers(&request.dst()).await?;
        Ok(self
            .send_and_get_responses(closest_peers, request, expect_all_responses)
            .await)
    }

    pub async fn get_store_cost_from_network(
        &self,
        record_address: NetworkAddress,
    ) -> Result<Token> {
        let (sender, receiver) = oneshot::channel();
        debug!("Attempting to get store cost");
        // first we need to get CLOSE_GROUP of the dbc_id
        self.send_swarm_cmd(SwarmCmd::GetClosestPeers {
            key: record_address.clone(),
            sender,
        })?;

        let close_nodes = receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)?
            .into_iter()
            .collect_vec();

        let request = Request::Query(Query::GetStoreCost(record_address));
        let responses = self
            .send_and_get_responses(close_nodes, &request, true)
            .await;

        // loop over responses, generating an avergae fee and storing all responses along side
        let mut all_costs = vec![];
        for response in responses.into_iter().flatten() {
            if let Response::Query(QueryResponse::GetStoreCost(Ok(cost))) = response {
                all_costs.push(cost);
            } else {
                println!("other response was {:?}", response);
            }
        }

        let token_fee = get_fee_from_store_cost_quotes(&mut all_costs)?;

        info!(
            "Final fee calculated as: {token_fee:?}, from: {:?}",
            all_costs
        );
        Ok(token_fee)
    }

    /// Get the Record from the network
    /// Carry out re-attempts if required
    /// In case a target_record is provided, only return when fetched target.
    /// Otherwise count it as a failure when all attempts completed.
    pub async fn get_record_from_network(
        &self,
        key: RecordKey,
        target_record: Option<Record>,
        re_attempt: bool,
    ) -> Result<Record> {
        let total_attempts = if re_attempt { VERIFICATION_ATTEMPTS } else { 1 };

        let mut verification_attempts = 0;

        while verification_attempts < total_attempts {
            verification_attempts += 1;
            debug!(
                "Getting record of {:?} attempts {verification_attempts:?}/{total_attempts:?}",
                PrettyPrintRecordKey::from(key.clone()),
            );

            let (sender, receiver) = oneshot::channel();
            self.send_swarm_cmd(SwarmCmd::GetNetworkRecord {
                key: key.clone(),
                sender,
            })?;

            match receiver
                .await
                .map_err(|_e| Error::InternalMsgChannelDropped)?
            {
                Ok(returned_record) => {
                    // Returning OK whenever fulfill one of the followings:
                    // 1, No targeting record
                    // 2, Fetched record matches the targeting record
                    //
                    // Returning mismatched error when: completed all attempts
                    if target_record.is_none()
                        || (target_record.is_some()
                            && target_record == Some(returned_record.clone()))
                    {
                        return Ok(returned_record);
                    } else if verification_attempts >= total_attempts {
                        return Err(Error::ReturnedRecordDoesNotMatch(
                            returned_record.key.into(),
                        ));
                    }
                }
                Err(Error::RecordNotEnoughCopies(returned_record)) => {
                    // Only return when completed all attempts
                    if verification_attempts >= total_attempts {
                        if target_record.is_none()
                            || (target_record.is_some()
                                && target_record == Some(returned_record.clone()))
                        {
                            return Ok(returned_record);
                        } else {
                            return Err(Error::ReturnedRecordDoesNotMatch(
                                returned_record.key.into(),
                            ));
                        }
                    }
                }
                Err(error) => {
                    warn!(
                        "Did not retrieve Record '{:?}' from network!. Retrying...",
                        PrettyPrintRecordKey::from(key.clone()),
                    );
                    error!("{error:?}");
                }
            }

            // wait for a bit before re-trying
            tokio::time::sleep(REVERIFICATION_WAIT_TIME_S).await;
        }

        Err(Error::RecordNotFound)
    }

    /// Get the cost of storing the next record from the network
    pub async fn get_local_storecost(&self) -> Result<Token> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalStoreCost { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Get `Record` from the local RecordStore
    pub async fn get_local_record(&self, key: &RecordKey) -> Result<Option<Record>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalRecord {
            key: key.clone(),
            sender,
        })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Put `Record` to network
    /// optionally verify the record is stored after putting it to network
    pub async fn put_record(&self, record: Record, verify_store: bool) -> Result<()> {
        if verify_store {
            self.put_record_with_retries(record).await
        } else {
            self.put_record_once(record, false).await
        }
    }

    /// Put `Record` to network
    /// Verify the record is stored after putting it to network
    /// Retry up to `PUT_RECORD_RETRIES` times if we can't verify the record is stored
    async fn put_record_with_retries(&self, record: Record) -> Result<()> {
        let mut retries = 0;
        while retries < PUT_RECORD_RETRIES {
            let res = self.put_record_once(record.clone(), true).await;
            if !matches!(res, Err(Error::FailedToVerifyRecordWasStored(_))) {
                return res;
            }
            retries += 1;
        }
        Err(Error::FailedToVerifyRecordWasStored(record.key.into()))
    }

    async fn put_record_once(&self, record: Record, verify_store: bool) -> Result<()> {
        debug!(
            "Putting record of {} - length {:?} to network",
            PrettyPrintRecordKey::from(record.key.clone()),
            record.value.len()
        );
        let the_record = record.clone();
        // Waiting for a response to avoid flushing to network too quick that causing choke
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::PutRecord {
            record: record.clone(),
            sender,
        })?;
        let response = receiver.await?;

        if verify_store {
            // Verify the record is stored, requiring re-attempts
            let _ = self
                .get_record_from_network(record.key.clone(), Some(record), true)
                .await
                .map_err(|e| {
                    trace!(
                        "Failing to verify the put record {:?} with error {e:?}",
                        the_record.key
                    );
                    Error::FailedToVerifyRecordWasStored(the_record.key.into())
                })?;
        }

        response
    }

    /// Put `Record` to the local RecordStore
    /// Must be called after the validations are performed on the Record
    pub fn put_local_record(&self, record: Record) -> Result<()> {
        debug!(
            "Writing Record locally, for {:?} - length {:?}",
            PrettyPrintRecordKey::from(record.key.clone()),
            record.value.len()
        );
        self.send_swarm_cmd(SwarmCmd::PutLocalRecord { record })
    }

    /// Returns true if a RecordKey is present locally in the RecordStore
    pub async fn is_key_present_locally(&self, key: &RecordKey) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::RecordStoreHasKey {
            key: key.clone(),
            sender,
        })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Returns the Addresses of all the locally stored Records
    pub async fn get_all_local_record_addresses(&self) -> Result<HashSet<NetworkAddress>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalRecordAddresses { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    // Add a list of keys of a holder to Replication Fetcher.
    pub fn add_keys_to_replication_fetcher(
        &self,
        peer: PeerId,
        keys: Vec<NetworkAddress>,
    ) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::AddKeysToReplicationFetcher { peer, keys })
    }

    /// Set the acceptable range of record entry. A record is removed from the storage if the
    /// distance between the record and the node is greater than the provided `distance`.
    pub fn set_record_distance_range(&self, distance: Distance) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SetRecordDistanceRange { distance })
    }

    /// Send `Request` to the given `PeerId` and await for the response. If `self` is the recipient,
    /// then the `Request` is forwarded to itself and handled, and a corresponding `Response` is created
    /// and returned to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn send_request(&self, req: Request, peer: PeerId) -> Result<Response> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::SendRequest {
            req,
            peer,
            sender: Some(sender),
        })?;
        receiver.await?
    }

    /// Send `Request` to the given `PeerId` and do _not_ await a response here.
    /// Instead the Response will be handled by the common `response_handler`
    pub fn send_req_ignore_reply(&self, req: Request, peer: PeerId) -> Result<()> {
        let swarm_cmd = SwarmCmd::SendRequest {
            req,
            peer,
            sender: None,
        };
        self.send_swarm_cmd(swarm_cmd)
    }

    /// Send a `Response` through the channel opened by the requester.
    pub fn send_response(&self, resp: Response, channel: MsgResponder) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SendResponse { resp, channel })
    }

    /// Return a `SwarmLocalState` with some information obtained from swarm's local state.
    pub async fn get_swarm_local_state(&self) -> Result<SwarmLocalState> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetSwarmLocalState(sender))?;
        let state = receiver.await?;
        Ok(state)
    }

    // Helper to send SwarmCmd
    fn send_swarm_cmd(&self, cmd: SwarmCmd) -> Result<()> {
        let capacity = self.swarm_cmd_sender.capacity();

        if capacity == 0 {
            error!("SwarmCmd channel is full. Dropping SwarmCmd: {:?}", cmd);

            // Lets error out just now.
            return Err(Error::NoSwarmCmdChannelCapacity);
        }
        let cmd_sender = self.swarm_cmd_sender.clone();

        // Spawn a task to send the SwarmCmd and keep this fn sync
        let _handle = tokio::spawn(async move {
            if let Err(error) = cmd_sender.send(cmd).await {
                error!("Failed to send SwarmCmd: {}", error);
            }
        });

        Ok(())
    }

    /// Returns the closest peers to the given `XorName`, sorted by their distance to the xor_name.
    /// If `client` is false, then include `self` among the `closest_peers`
    async fn get_closest_peers(&self, key: &NetworkAddress, client: bool) -> Result<Vec<PeerId>> {
        trace!("Getting the closest peers to {key:?}");
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestPeers {
            key: key.clone(),
            sender,
        })?;
        let k_bucket_peers = receiver.await?;

        // Count self in if among the CLOSE_GROUP_SIZE closest and sort the result
        let mut closest_peers: Vec<_> = k_bucket_peers.into_iter().collect();
        if !client {
            closest_peers.push(self.peer_id);
        }
        sort_peers_by_address(closest_peers, key, CLOSE_GROUP_SIZE)
    }

    /// Send a `Request` to the provided set of peers and wait for their responses concurrently.
    /// If `get_all_responses` is true, we wait for the responses from all the peers.
    /// NB TODO: Will return an error if the request timeouts.
    /// If `get_all_responses` is false, we return the first successful response that we get
    pub async fn send_and_get_responses(
        &self,
        peers: Vec<PeerId>,
        req: &Request,
        get_all_responses: bool,
    ) -> Vec<Result<Response>> {
        trace!("send_and_get_responses for {req:?}");
        let mut list_of_futures = peers
            .iter()
            .map(|peer| Box::pin(self.send_request(req.clone(), *peer)))
            .collect::<Vec<_>>();

        let mut responses = Vec::new();
        while !list_of_futures.is_empty() {
            let (res, _, remaining_futures) = select_all(list_of_futures).await;
            let res_string = match &res {
                Ok(res) => format!("{res}"),
                Err(err) => format!("{err:?}"),
            };
            trace!("Got response for the req: {req:?}, res: {res_string}");
            if !get_all_responses && res.is_ok() {
                return vec![res];
            }
            responses.push(res);
            list_of_futures = remaining_futures;
        }

        trace!("got all responses for {req:?}");
        responses
    }
}

/// Given `all_costs` it will return the CLOSE_GROUP majority cost.
fn get_fee_from_store_cost_quotes(all_costs: &mut Vec<Token>) -> Result<Token> {
    // we're zero indexed, so we want the middle index
    let target_cost_index = CLOSE_GROUP_SIZE / 2;

    // sort all costs by fee, lowest to highest
    all_costs.sort();

    let token_fee = *all_costs
        .get(target_cost_index)
        .ok_or(Error::NotEnoughCostQuotes)?;

    info!(
        "Final fee calculated as: {token_fee:?}, from: {:?}",
        all_costs
    );
    Ok(token_fee)
}

/// Verifies if `Multiaddr` contains IPv4 address that is not global.
/// This is used to filter out unroutable addresses from the Kademlia routing table.
pub fn multiaddr_is_global(multiaddr: &Multiaddr) -> bool {
    !multiaddr.iter().any(|addr| match addr {
        Protocol::Ip4(ip) => {
            // Based on the nightly `is_global` method (`Ipv4Addrs::is_global`), only using what is available in stable.
            // Missing `is_shared`, `is_benchmarking` and `is_reserved`.
            ip.is_unspecified()
                | ip.is_private()
                | ip.is_loopback()
                | ip.is_link_local()
                | ip.is_documentation()
                | ip.is_broadcast()
        }
        _ => false,
    })
}

/// Pop off the `/p2p/<peer_id>`. This mutates the `Multiaddr` and returns the `PeerId` if it exists.
pub(crate) fn multiaddr_pop_p2p(multiaddr: &mut Multiaddr) -> Option<PeerId> {
    if let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() {
        // Only actually strip the last protocol if it's indeed the peer ID.
        let _ = multiaddr.pop();
        Some(peer_id)
    } else {
        None
    }
}

/// Build a `Multiaddr` with the p2p protocol filtered out.
pub(crate) fn multiaddr_strip_p2p(multiaddr: &Multiaddr) -> Multiaddr {
    multiaddr
        .iter()
        .filter(|p| !matches!(p, Protocol::P2p(_)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_fee_from_store_cost_quotes() -> Result<()> {
        // for a vec of different costs of CLOUSE_GROUP size
        // ensure we return the CLOSE_GROUP / 2 indexed price
        let mut costs = vec![];
        for i in 0..CLOSE_GROUP_SIZE {
            println!("price: {}", i);
            costs.push(Token::from_nano(i as u64));
        }
        let price = get_fee_from_store_cost_quotes(&mut costs)?;

        assert_eq!(
            price,
            Token::from_nano(CLOSE_GROUP_SIZE as u64 / 2),
            "price should be {}",
            CLOSE_GROUP_SIZE / 2 + 1
        );

        Ok(())
    }
}
