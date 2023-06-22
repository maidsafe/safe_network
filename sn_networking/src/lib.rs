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
    event::NodeBehaviour,
    msg::{MsgCodec, MsgProtocol},
    replication_fetcher::ReplicationFetcher,
};
use futures::{future::select_all, StreamExt};
use libp2p::{
    identity::Keypair,
    kad::{kbucket::Distance, KademliaStoreInserts},
    multiaddr::Protocol,
    request_response::{self, Config as RequestResponseConfig, ProtocolSupport, RequestId},
    swarm::{behaviour::toggle::Toggle, Swarm, SwarmBuilder},
    Multiaddr, PeerId, Transport,
};
#[cfg(feature = "local-discovery")]
use libp2p::{
    kad::{kbucket::Key as KBucketKey, Kademlia, KademliaConfig, QueryId, Record, RecordKey},
    mdns,
};
use rand::Rng;
use sn_protocol::{
    messages::{Request, Response},
    NetworkAddress,
};
use sn_record_store::{
    DiskBackedRecordStore, DiskBackedRecordStoreConfig, REPLICATION_INTERVAL_LOWER_BOUND,
    REPLICATION_INTERVAL_UPPER_BOUND,
};
use std::{
    collections::{HashMap, HashSet},
    iter,
    net::SocketAddr,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
pub const CLOSE_GROUP_SIZE: usize = 8;

// Timeout for requests sent/received through the request_response behaviour.
const REQUEST_TIMEOUT_DEFAULT_S: Duration = Duration::from_secs(30);
// Sets the keep-alive timeout of idle connections.
const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);

/// Our agent string has as a prefix that we can match against.
pub const IDENTIFY_AGENT_STR: &str = "safe/node/";

/// The suffix is the version of the node.
const IDENTIFY_AGENT_VERSION_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));
/// The suffix is the version of the client.
const IDENTIFY_CLIENT_VERSION_STR: &str = concat!("safe/client/", env!("CARGO_PKG_VERSION"));
const IDENTIFY_PROTOCOL_STR: &str = concat!("safe/", env!("CARGO_PKG_VERSION"));

const NETWORKING_CHANNEL_SIZE: usize = 10_000;
/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    CLOSE_GROUP_SIZE / 2 + 1
}

type PendingGetClosest = HashMap<QueryId, (oneshot::Sender<HashSet<PeerId>>, HashSet<PeerId>)>;

/// `SwarmDriver` is responsible for managing the swarm of peers, handling
/// swarm events, processing commands, and maintaining the state of pending
/// tasks. It serves as the core component for the network functionality.
pub struct SwarmDriver {
    self_peer_id: PeerId,
    swarm: Swarm<NodeBehaviour>,
    cmd_receiver: mpsc::Receiver<SwarmCmd>,
    event_sender: mpsc::Sender<NetworkEvent>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
    pending_get_closest_peers: PendingGetClosest,
    pending_requests: HashMap<RequestId, Option<oneshot::Sender<Result<Response>>>>,
    pending_query: HashMap<QueryId, oneshot::Sender<Result<Record>>>,
    replication_fetcher: ReplicationFetcher,
    local: bool,
    dialed_peers: CircularVec<PeerId>,
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
        keypair: Option<Keypair>,
        addr: SocketAddr,
        local: bool,
        root_dir: &Path,
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
            .set_record_filtering(KademliaStoreInserts::FilterBoth)
            // Disable provider records publication job
            .set_provider_publication_interval(None);

        let (network, events_receiver, mut swarm_driver) = Self::with(
            keypair,
            kad_cfg,
            local,
            false,
            replication_interval,
            None,
            Some(root_dir.join("record_store")),
            ProtocolSupport::Full,
            IDENTIFY_AGENT_VERSION_STR.to_string(),
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
            // clients use signer for transactions, but the network keypair is not used
            None,
            kad_cfg,
            local,
            true,
            // Nonsense interval for the client which never replicates
            Duration::from_secs(1000),
            request_timeout,
            None,
            ProtocolSupport::Outbound,
            IDENTIFY_CLIENT_VERSION_STR.to_string(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    /// Private helper to create the network components with the provided config and req/res behaviour
    fn with(
        keypair: Option<Keypair>,
        kad_cfg: KademliaConfig,
        local: bool,
        is_client: bool,
        replication_interval: Duration,
        request_response_timeout: Option<Duration>,
        disk_store_path: Option<PathBuf>,
        req_res_protocol: ProtocolSupport,
        identify_version: String,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, Self)> {
        // Create a random key for ourself if none provided
        let keypair = match keypair {
            Some(keypair) => {
                info!("Using provided keypair: {:?}", keypair.public());
                keypair
            }
            None => {
                info!("Generating a new keypair");
                Keypair::generate_ed25519()

                // TODO: store this somewhere? or just regenerate on every run?
                // How would we validate storage location + what to do if one exists?
            }
        };

        let peer_id = PeerId::from(keypair.public());

        info!("Node (PID: {}) with PeerId: {peer_id}", std::process::id());
        info!("PeerId: {peer_id} has replication interval of {replication_interval:?}");

        // RequestResponse Behaviour
        let request_response = {
            let mut cfg = RequestResponseConfig::default();
            let _ = cfg
                .set_request_timeout(request_response_timeout.unwrap_or(REQUEST_TIMEOUT_DEFAULT_S))
                .set_connection_keep_alive(CONNECTION_KEEP_ALIVE_TIMEOUT);

            request_response::Behaviour::new(
                MsgCodec(),
                iter::once((MsgProtocol(), req_res_protocol)),
                cfg,
            )
        };

        // Kademlia Behaviour
        let kademlia = {
            // Configures the disk_store to store records under the provided path and increase the max record size
            let storage_dir = disk_store_path.unwrap_or(std::env::temp_dir());
            if let Err(error) = std::fs::create_dir_all(&storage_dir) {
                return Err(Error::FailedToCreateRecordStoreDir {
                    path: storage_dir,
                    source: error,
                });
            }

            let store_cfg = DiskBackedRecordStoreConfig {
                max_value_bytes: 1024 * 1024,
                storage_dir,
                replication_interval,
                ..Default::default()
            };

            Kademlia::with_config(
                peer_id,
                DiskBackedRecordStore::with_config(peer_id, store_cfg),
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
                    .with_agent_version(identify_version)
                    // Default in future libp2p version. (TODO: check if default already)
                    .with_initial_delay(Duration::from_secs(0));
            libp2p::identify::Behaviour::new(cfg)
        };

        // Transport
        let transport = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(
                libp2p::noise::Config::new(&keypair)
                    .expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(libp2p::yamux::Config::default())
            .boxed();

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
        let (network_event_sender, network_event_receiver) = mpsc::channel(NETWORKING_CHANNEL_SIZE);
        let swarm_driver = Self {
            self_peer_id: peer_id,
            swarm,
            cmd_receiver: swarm_cmd_receiver,
            event_sender: network_event_sender,
            pending_dial: Default::default(),
            pending_get_closest_peers: Default::default(),
            pending_requests: Default::default(),
            pending_query: Default::default(),
            replication_fetcher: Default::default(),
            local,
            dialed_peers: CircularVec::new(63),
        };

        Ok((
            Network {
                swarm_cmd_sender,
                peer_id,
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
                    if let Err(err) = self.handle_swarm_events(swarm_event).await {
                        warn!("Error while handling swarm event: {err}");
                    }
                },
                some_cmd = self.cmd_receiver.recv() => match some_cmd {
                    Some(cmd) => {
                        if let Err(err) = self.handle_cmd(cmd).await {
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
}

impl Network {
    ///  Listen for incoming connections on the given address.
    pub async fn start_listening(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::StartListening { addr, sender })
            .await?;
        receiver.await?
    }

    /// Dial the given peer at the given address.
    pub async fn add_to_routing_table(&self, peer_id: PeerId, peer_addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::AddToRoutingTable {
            peer_id,
            peer_addr,
            sender,
        })
        .await?;
        receiver.await?
    }

    /// Dial the given peer at the given address.
    pub async fn dial(&self, peer_id: PeerId, peer_addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::Dial {
            peer_id,
            peer_addr,
            sender,
        })
        .await?;
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
        })
        .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Returns all the PeerId from all the KBuckets from our local Routing Table
    /// Also contains our own PeerId.
    pub async fn get_all_local_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalPeers { sender })
            .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Send `Request` to the closest peers. If `self` is among the closest_peers, the `Request` is
    /// forwarded to itself and handled. Then a corresponding `Response` is created and is
    /// forwarded to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn node_send_to_closest(&self, request: &Request) -> Result<Vec<Result<Response>>> {
        info!(
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
        info!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst()
        );
        let closest_peers = self.node_get_closest_peers(&request.dst()).await?;
        for peer in closest_peers {
            self.send_req_ignore_reply(request.clone(), peer).await?;
        }
        Ok(())
    }

    /// Send `Request` to the closest peers to self
    pub async fn send_req_no_reply_to_self_closest(&self, request: &Request) -> Result<()> {
        info!("Sending {request:?} to self closest peers.");
        // Using `client_get_closest_peers` to filter self out.
        let closest_peers = self.client_get_closest_peers(&request.dst()).await?;
        for peer in closest_peers {
            self.send_req_ignore_reply(request.clone(), peer).await?;
        }
        Ok(())
    }

    /// Send `Request` to the closest peers. `Self` is not present among the recipients.
    pub async fn client_send_to_closest(
        &self,
        request: &Request,
        expect_all_responses: bool,
    ) -> Result<Vec<Result<Response>>> {
        info!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst()
        );
        let closest_peers = self.client_get_closest_peers(&request.dst()).await?;
        Ok(self
            .send_and_get_responses(closest_peers, request, expect_all_responses)
            .await)
    }

    /// Returns the list of keys that are within the provided distance to the target
    pub async fn get_record_keys_closest_to_target(
        &self,
        target: &NetworkAddress,
        distance: Distance,
    ) -> Result<Vec<RecordKey>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetRecordKeysClosestToTarget {
            key: target.clone(),
            distance,
            sender,
        })
        .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Get the Record from the network
    pub async fn get_record_from_network(&self, key: RecordKey) -> Result<Record> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetNetworkRecord { key, sender })
            .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)?
    }

    /// Get `Record` from the local RecordStore
    pub async fn get_local_record(&self, key: &RecordKey) -> Result<Option<Record>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalRecord {
            key: key.clone(),
            sender,
        })
        .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Put `Record` to the local RecordStore
    /// Must be called after the validations are performed on the Record
    pub async fn put_local_record(&self, record: Record) -> Result<()> {
        debug!(
            "Writing Record locally, for {:?} - length {:?}",
            record.key,
            record.value.len()
        );
        self.send_swarm_cmd(SwarmCmd::PutLocalRecord { record })
            .await
    }

    /// Get the RecordAddress of all the Records stored locally
    pub async fn get_all_local_record_addresses(&self) -> Result<HashSet<NetworkAddress>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllRecordAddress { sender })
            .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Returns true if a RecordKey is present locally in the RecordStore
    pub async fn is_key_present_locally(&self, key: &RecordKey) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::RecordStoreHasKey {
            key: key.clone(),
            sender,
        })
        .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    // Add a list of keys of a holder to RecordFetcher.  Return with a list of keys to fetch, if present.
    pub async fn add_keys_to_replication_fetcher(
        &self,
        peer: PeerId,
        keys: Vec<NetworkAddress>,
    ) -> Result<Vec<(PeerId, NetworkAddress)>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::AddKeysToReplicationFetcher { peer, keys, sender })
            .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    // Notify the fetch result of a key from a holder. Return with a list of keys to fetch, if present.
    pub async fn notify_fetch_result(
        &self,
        peer: PeerId,
        key: NetworkAddress,
        result: bool,
    ) -> Result<Vec<(PeerId, NetworkAddress)>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::NotifyFetchResult {
            peer,
            key,
            result,
            sender,
        })
        .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Set the acceptable range of record entry. A record is removed from the storage if the
    /// distance between the record and the node is greater than the provided `distance`.
    pub async fn set_record_distance_range(&self, distance: Distance) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SetRecordDistanceRange { distance })
            .await
    }

    /// Send `Request` to the the given `PeerId` and await for the response. If `self` is the recipient,
    /// then the `Request` is forwarded to itself and handled, and a corresponding `Response` is created
    /// and returned to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn send_request(&self, req: Request, peer: PeerId) -> Result<Response> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::SendRequest {
            req,
            peer,
            sender: Some(sender),
        })
        .await?;
        receiver.await?
    }

    /// Send `Request` to the the given `PeerId` and do _not_ await a response here.
    /// Instead the Response will be handled by the common `response_handler`
    pub async fn send_req_ignore_reply(&self, req: Request, peer: PeerId) -> Result<()> {
        let swarm_cmd = SwarmCmd::SendRequest {
            req,
            peer,
            sender: None,
        };
        self.send_swarm_cmd(swarm_cmd).await
    }

    /// Send a `Response` through the channel opened by the requester.
    pub async fn send_response(&self, resp: Response, channel: MsgResponder) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SendResponse { resp, channel })
            .await
    }

    /// Return a `SwarmLocalState` with some information obtained from swarm's local state.
    pub async fn get_swarm_local_state(&self) -> Result<SwarmLocalState> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetSwarmLocalState(sender))
            .await?;
        let state = receiver.await?;
        Ok(state)
    }

    // Helper to send SwarmCmd
    async fn send_swarm_cmd(&self, cmd: SwarmCmd) -> Result<()> {
        self.swarm_cmd_sender.send(cmd).await?;
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
        })
        .await?;
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

// Strip out the p2p protocol from a multiaddr.
pub fn multiaddr_strip_p2p(multiaddr: &Multiaddr) -> Multiaddr {
    multiaddr
        .iter()
        .filter(|p| !matches!(p, Protocol::P2p(_)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SwarmDriver;
    use crate::{MsgResponder, NetworkEvent};
    use assert_matches::assert_matches;
    use bytes::Bytes;
    use eyre::{eyre, Result};
    use rand::{thread_rng, Rng};
    use sn_logging::init_test_logger;
    use sn_protocol::{
        messages::{CmdOk, CmdResponse, Query, Request, Response},
        storage::Chunk,
    };
    use std::{net::SocketAddr, path::Path, time::Duration};

    #[tokio::test]
    async fn msg_to_self_should_not_error_out() -> Result<()> {
        init_test_logger();
        let (net, mut event_rx, driver) = SwarmDriver::new(
            None,
            "0.0.0.0:0"
                .parse::<SocketAddr>()
                .expect("0.0.0.0:0 should parse into a valid `SocketAddr`"),
            true,
            Path::new(""),
        )?;
        let _driver_handle = tokio::spawn(driver.run());

        // Spawn a task to handle the Request that we recieve.
        // This handles the request and sends a response back.
        let _event_handler = tokio::spawn(async move {
            loop {
                if let Some(NetworkEvent::RequestReceived {
                    channel: MsgResponder::FromSelf(channel),
                    ..
                }) = event_rx.recv().await
                {
                    let res = Response::Cmd(CmdResponse::StoreChunk(Ok(CmdOk::StoredSuccessfully)));
                    if let Some(channel) = channel {
                        assert!(channel.send(Ok(res)).is_ok());
                    }
                }
            }
        });

        // Send a request to query a random chunk to `self`.
        let mut random_data = [0u8; 128];
        thread_rng().fill(&mut random_data);
        let req = Request::Query(Query::GetChunk(
            *Chunk::new(Bytes::copy_from_slice(&random_data)).address(),
        ));
        // Send the request to `self` and wait for a response.
        let now = tokio::time::Instant::now();
        loop {
            let mut res = net
                .send_and_get_responses(vec![net.peer_id], &req, true)
                .await;
            if res.is_empty() || res[0].is_err() {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if now.elapsed() > Duration::from_secs(10) {
                    return Err(eyre!("Timed out waiting for response."));
                }
            } else {
                let res = res
                    .remove(0)
                    .expect("There should be at least one response!");
                info!("Got response {:?}", res);
                assert_matches!(
                    res,
                    Response::Cmd(CmdResponse::StoreChunk(Ok(CmdOk::StoredSuccessfully)))
                );
                return Ok(());
            }
        }
    }
}
