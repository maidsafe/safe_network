// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod cmd;
mod error;
mod event;
mod msg;

pub use self::{
    cmd::SwarmLocalState,
    error::Error,
    event::{MsgResponder, NetworkEvent},
};

use self::{
    cmd::SwarmCmd,
    error::Result,
    event::NodeBehaviour,
    msg::{MsgCodec, MsgProtocol},
};

use crate::domain::storage::{DiskBackedRecordStore, DiskBackedRecordStoreConfig};
use crate::protocol::messages::{QueryResponse, Request, Response};
use futures::{future::select_all, StreamExt};
use libp2p::{
    core::muxing::StreamMuxerBox,
    identity,
    kad::{KBucketKey, Kademlia, KademliaConfig, QueryId, Record, RecordKey},
    mdns,
    multiaddr::Protocol,
    request_response::{self, Config as RequestResponseConfig, ProtocolSupport, RequestId},
    swarm::{Swarm, SwarmBuilder},
    Multiaddr, PeerId, Transport,
};
use std::{
    collections::{HashMap, HashSet},
    iter,
    net::SocketAddr,
    num::NonZeroUsize,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;
use xor_name::XorName;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
pub(crate) const CLOSE_GROUP_SIZE: usize = 8;

// Timeout for requests sent/received through the request_response behaviour.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
// Sets the keep-alive timeout of idle connections.
const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Our agent string has as a prefix that we can match against.
pub const IDENTIFY_AGENT_STR: &str = "safe/node/";

/// The suffix is the version of the node.
const IDENTIFY_AGENT_VERSION_STR: &str = concat!("safe/node/", env!("CARGO_PKG_VERSION"));
/// The suffix is the version of the client.
const IDENTIFY_CLIENT_VERSION_STR: &str = concat!("safe/client/", env!("CARGO_PKG_VERSION"));
const IDENTIFY_PROTOCOL_STR: &str = concat!("safe/", env!("CARGO_PKG_VERSION"));

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
    swarm: Swarm<NodeBehaviour>,
    cmd_receiver: mpsc::Receiver<SwarmCmd>,
    event_sender: mpsc::Sender<NetworkEvent>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
    pending_get_closest_peers: PendingGetClosest,
    pending_requests: HashMap<RequestId, oneshot::Sender<Result<Response>>>,
    pending_query: HashMap<QueryId, oneshot::Sender<Result<QueryResponse>>>,
}

impl SwarmDriver {
    /// Creates a new `SwarmDriver` instance, along with a `Network` handle
    /// for sending commands and an `mpsc::Receiver<NetworkEvent>` for receiving
    /// network events. It initializes the swarm, sets up the transport, and
    /// configures the Kademlia and mDNS behaviours for peer discovery.
    ///
    /// # Returns
    ///
    /// A tuple containing a `Network` handle, an `mpsc::Receiver<NetworkEvent>`,
    /// and a `SwarmDriver` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the mDNS behaviour.
    pub fn new(addr: SocketAddr) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        let mut kad_cfg = KademliaConfig::default();
        let _ = kad_cfg
            // how often a node will replicate records that it has stored, aka copying the key-value pair to other nodes
            // this is a heavier operation than publication, so it is done less frequently
            .set_replication_interval(Some(Duration::from_secs(20)))
            // how often a node will announce that it is providing a value for a specific key
            .set_provider_publication_interval(Some(Duration::from_secs(10)))
            // how often a node will publish a record key, aka telling the others it exists
            .set_publication_interval(Some(Duration::from_secs(5)))
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
            .set_record_ttl(None);

        let (network, events_receiver, mut swarm_driver) =
            Self::with(kad_cfg, ProtocolSupport::Full, false)?;

        // Listen on the provided address
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
    pub fn new_client() -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        // Create a Kademlia behaviour for client mode, i.e. set req/resp protocol
        // to outbound-only mode and don't listen on any address
        let mut cfg = KademliaConfig::default(); // default query timeout is 60 secs

        // 1mb packet size
        let _ = cfg.set_max_packet_size(1024 * 1024);
        // Require iterative queries to use disjoint paths for increased resiliency in the presence of potentially adversarial nodes.
        let _ = cfg.disjoint_query_paths(true);
        // How many nodes _should_ store data.
        let _ = cfg.set_replication_factor(
            NonZeroUsize::new(CLOSE_GROUP_SIZE).ok_or_else(|| Error::InvalidCloseGroupSize)?,
        );

        Self::with(cfg, ProtocolSupport::Outbound, true)
    }

    // Private helper to create the network components with the provided config and req/res behaviour
    fn with(
        kad_cfg: KademliaConfig,
        req_res_protocol: ProtocolSupport,
        is_client: bool,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        // Create a random key for ourself.
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        info!("Node (PID: {}) with PeerId: {peer_id}", std::process::id());
        info!(
            "PeerId converted to XorName: {peer_id} - {:?}",
            XorName::from_content(&peer_id.to_bytes())
        );

        // RequestResponse configuration
        let mut req_res_config = RequestResponseConfig::default();
        let _ = req_res_config.set_request_timeout(REQUEST_TIMEOUT);
        let _ = req_res_config.set_connection_keep_alive(CONNECTION_KEEP_ALIVE_TIMEOUT);

        let request_response = request_response::Behaviour::new(
            MsgCodec(),
            iter::once((MsgProtocol(), req_res_protocol)),
            req_res_config,
        );

        // QUIC configuration
        let quic_config = libp2p_quic::Config::new(&keypair);
        let transport = libp2p_quic::tokio::Transport::new(quic_config);
        let transport = transport
            .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
            .boxed();

        // Configures the memory store to be able to hold larger
        // records than by default
        let memory_store_cfg = DiskBackedRecordStoreConfig {
            max_value_bytes: 1024 * 1024,
            max_providers_per_key: CLOSE_GROUP_SIZE,
            ..Default::default()
        };

        // Create a Kademlia behaviour for client mode, i.e. set req/resp protocol
        // to outbound-only mode and don't listen on any address
        let kademlia = Kademlia::with_config(
            peer_id,
            DiskBackedRecordStore::with_config(peer_id, memory_store_cfg),
            kad_cfg,
        );

        let mdns_config = mdns::Config {
            // lower query interval to speed up peer discovery
            // this increases traffic, but means we no longer have clients unable to connect
            // after a few minutes
            query_interval: Duration::from_secs(5),
            ..Default::default()
        };
        let mdns = mdns::tokio::Behaviour::new(mdns_config, peer_id)?;

        let identify_cfg = if is_client {
            libp2p::identify::Config::new(IDENTIFY_PROTOCOL_STR.to_string(), keypair.public())
                .with_agent_version(IDENTIFY_CLIENT_VERSION_STR.to_string())
        } else {
            libp2p::identify::Config::new(IDENTIFY_PROTOCOL_STR.to_string(), keypair.public())
                .with_agent_version(IDENTIFY_AGENT_VERSION_STR.to_string())
        };
        let identify = libp2p::identify::Behaviour::new(identify_cfg);

        let behaviour = NodeBehaviour {
            request_response,
            kademlia,
            mdns,
            identify,
        };

        let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();

        let (swarm_cmd_sender, swarm_cmd_receiver) = mpsc::channel(100);
        let (network_event_sender, network_event_receiver) = mpsc::channel(100);
        let swarm_driver = Self {
            swarm,
            cmd_receiver: swarm_cmd_receiver,
            event_sender: network_event_sender,
            pending_dial: Default::default(),
            pending_get_closest_peers: Default::default(),
            pending_requests: Default::default(),
            pending_query: Default::default(),
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
                        warn!("Error while handling event: {err}");
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

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct Network {
    pub(super) swarm_cmd_sender: mpsc::Sender<SwarmCmd>,
    pub(super) peer_id: PeerId,
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
    pub async fn client_get_closest_peers(&self, xor_name: XorName) -> Result<Vec<PeerId>> {
        self.get_closest_peers(xor_name, true).await
    }

    /// Returns the closest peers to the given `XorName`, sorted by their distance to the xor_name.
    /// Includes our node's `PeerId` while calculating the closest peers.
    pub async fn node_get_closest_peers(&self, xor_name: XorName) -> Result<Vec<PeerId>> {
        self.get_closest_peers(xor_name, false).await
    }

    /// Send `Request` to the closest peers. If `self` is among the closest_peers, the `Request` is
    /// forwarded to itself and handled. Then a corresponding `Response` is created and is
    /// forwarded to iself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn node_send_to_closest(&self, request: &Request) -> Result<Vec<Result<Response>>> {
        info!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst().name()
        );
        let closest_peers = self.node_get_closest_peers(*request.dst().name()).await?;

        Ok(self
            .send_and_get_responses(closest_peers, request, true)
            .await)
    }

    /// Send `Request` to the closest peers. If `self` is among the closest_peers, the `Request` is
    /// forwarded to itself and handled. Then a corresponding `Response` is created and is
    /// forwarded to iself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn fire_and_forget_to_closest(&self, request: &Request) -> Result<()> {
        info!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst().name()
        );
        let closest_peers = self.node_get_closest_peers(*request.dst().name()).await?;
        for peer in closest_peers {
            self.fire_and_forget(request.clone(), peer).await?;
        }
        Ok(())
    }

    /// Send `Request` to the closest peers. `Self` is not present among the recipients.
    pub async fn client_send_to_closest(&self, request: &Request) -> Result<Vec<Result<Response>>> {
        info!(
            "Sending {request:?} with dst {:?} to the closest peers.",
            request.dst().name()
        );
        let closest_peers = self.client_get_closest_peers(*request.dst().name()).await?;

        Ok(self
            .send_and_get_responses(closest_peers, request, true)
            .await)
    }

    /// Get `Key` from our Storage
    pub async fn get_provided_data(&self, key: RecordKey) -> Result<Result<QueryResponse>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetData { key, sender })
            .await?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Put data to KAD network as record
    pub async fn put_data_as_record(&self, record: Record) -> Result<()> {
        debug!(
            "Putting data as record, for {:?} - length {:?}",
            record.key,
            record.value.len()
        );
        self.send_swarm_cmd(SwarmCmd::PutProvidedDataAsRecord { record })
            .await
    }

    /// Send `Request` to the the given `PeerId` and await for the response. If `self` is the recipient,
    /// then the `Request` is forwarded to itself and handled, and a corresponding `Response` is created
    /// and returned to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn send_request(&self, req: Request, peer: PeerId) -> Result<Response> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::SendRequest { req, peer, sender })
            .await?;
        receiver.await?
    }

    /// Send a `Response` through the channel opened by the requester.
    pub async fn send_response(&self, resp: Response, channel: MsgResponder) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SendResponse { resp, channel })
            .await
    }

    /// Send `Request` to the the given `PeerId` and do _not_ await a response.
    pub async fn fire_and_forget(&self, req: Request, peer: PeerId) -> Result<()> {
        let (sender, _) = oneshot::channel();
        let swarm_cmd = SwarmCmd::SendRequest { req, peer, sender };
        self.send_swarm_cmd(swarm_cmd).await
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
    async fn get_closest_peers(&self, xor_name: XorName, client: bool) -> Result<Vec<PeerId>> {
        debug!("Getting the closest peers to {xor_name:?}");
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestPeers { xor_name, sender })
            .await?;
        let k_bucket_peers = receiver.await?;

        // Count self in if among the CLOSE_GROUP_SIZE closest and sort the result
        let mut closest_peers: Vec<_> = k_bucket_peers.into_iter().collect();
        if !client {
            closest_peers.push(self.peer_id);
        }
        self.sort_peers_by_key(closest_peers, xor_name.0.to_vec())
    }

    /// Sort the provided peers by their distance to the given key.
    fn sort_peers_by_key(&self, mut peers: Vec<PeerId>, key: Vec<u8>) -> Result<Vec<PeerId>> {
        let target = KBucketKey::new(key);
        peers.sort_by(|a, b| {
            let a = KBucketKey::new(a.to_bytes());
            let b = KBucketKey::new(b.to_bytes());
            target.distance(&a).cmp(&target.distance(&b))
        });
        let peers: Vec<PeerId> = peers.iter().take(CLOSE_GROUP_SIZE).cloned().collect();

        if CLOSE_GROUP_SIZE > peers.len() {
            warn!("Not enough peers in the k-bucket to satisfy the request");
            return Err(Error::NotEnoughPeers);
        }
        Ok(peers)
    }

    // Send a `Request` to the provided set of peers and wait for their responses concurrently.
    // If `get_all_responses` is true, we wait for the responses from all the peers. Will return an
    // error if the request timeouts.
    // If `get_all_responses` is false, we return the first successful response that we get
    async fn send_and_get_responses(
        &self,
        peers: Vec<PeerId>,
        req: &Request,
        get_all_responses: bool,
    ) -> Vec<Result<Response>> {
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

        responses
    }
}

#[cfg(test)]
mod tests {
    use super::SwarmDriver;

    use crate::{
        log::init_test_logger,
        network::{MsgResponder, NetworkEvent},
        protocol::{
            messages::{Cmd, CmdResponse, Request, Response},
            storage::Chunk,
        },
    };

    use assert_matches::assert_matches;
    use bytes::Bytes;
    use eyre::{eyre, Result};
    use libp2p::{
        kad::{
            kbucket::{Entry, InsertResult, KBucketsTable, NodeStatus},
            KBucketKey,
        },
        PeerId,
    };
    use rand::{thread_rng, Rng};
    use std::{
        collections::{BTreeMap, HashMap},
        fmt,
        net::SocketAddr,
        time::Duration,
    };
    use xor_name::XorName;

    #[tokio::test(flavor = "multi_thread")]
    async fn closest() -> Result<()> {
        init_test_logger();
        let mut networks_list = Vec::new();
        let mut network_events_recievers = BTreeMap::new();
        for _ in 1..25 {
            let (net, event_rx, driver) = SwarmDriver::new(
                "0.0.0.0:0"
                    .parse::<SocketAddr>()
                    .expect("0.0.0.0:0 should parse into a valid `SocketAddr`"),
            )?;
            let _handle = tokio::spawn(driver.run());

            let _ = network_events_recievers.insert(net.peer_id, event_rx);
            networks_list.push(net);
        }

        // Check the closest nodes to the following random_data
        let mut rng = thread_rng();
        let random_data = XorName::random(&mut rng);
        let random_data_key = KBucketKey::from(random_data.0.to_vec());

        tokio::time::sleep(Duration::from_secs(5)).await;
        let our_net = networks_list
            .get(0)
            .ok_or_else(|| eyre!("networks_list is not empty"))?;

        // Get the expected list of closest peers by creating a `KBucketsTable` with all the peers
        // inserted inside it.
        // The `KBucketsTable::local_key` is considered to be random since the `local_key` will not
        // be part of the `closest_peers`. Since our implementation of `get_closest_peers` returns
        // `self`, we'd want to insert `our_net` into the table as well.
        let mut table =
            KBucketsTable::<_, ()>::new(KBucketKey::from(PeerId::random()), Duration::from_secs(5));
        let mut key_to_peer_id = HashMap::new();
        for net in networks_list.iter() {
            let key = KBucketKey::from(net.peer_id);
            let _ = key_to_peer_id.insert(key.clone(), net.peer_id);

            if let Entry::Absent(e) = table.entry(&key) {
                match e.insert((), NodeStatus::Connected) {
                    InsertResult::Inserted => {}
                    _ => continue,
                }
            } else {
                return Err(eyre!("Table entry should be absent"));
            }
        }
        let expected_from_table = table
            .closest_keys(&random_data_key)
            .map(|key| {
                key_to_peer_id
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| eyre::eyre!("Key should be present"))
            })
            .take(8)
            .collect::<Result<Vec<_>>>()?;
        info!("Got Closest from table {:?}", expected_from_table.len());

        // Ask the other nodes for the closest_peers.
        let closest = our_net.get_closest_peers(random_data, false).await?;

        assert_lists(closest, expected_from_table);
        Ok(())
    }

    #[tokio::test]
    async fn msg_to_self_should_not_error_out() -> Result<()> {
        init_test_logger();
        let (net, mut event_rx, driver) = SwarmDriver::new(
            "0.0.0.0:0"
                .parse::<SocketAddr>()
                .expect("0.0.0.0:0 should parse into a valid `SocketAddr`"),
        )?;
        let _driver_handle = tokio::spawn(driver.run());

        // Spawn a task to handle the the Request that we recieve.
        // This handles the request and sends a response back.
        let _event_handler = tokio::spawn(async move {
            loop {
                if let Some(NetworkEvent::RequestReceived {
                    channel: MsgResponder::FromSelf(channel),
                    ..
                }) = event_rx.recv().await
                {
                    let res = Response::Cmd(CmdResponse::StoreChunk(Ok(())));
                    assert!(channel.send(Ok(res)).is_ok());
                }
            }
        });

        // Send a request to store a random chunk to `self`.
        let mut random_data = [0u8; 128];
        thread_rng().fill(&mut random_data);
        let req = Request::Cmd(Cmd::StoreChunk(Chunk::new(Bytes::copy_from_slice(
            &random_data,
        ))));
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
                assert_matches!(res, Response::Cmd(CmdResponse::StoreChunk(Ok(()))));
                return Ok(());
            }
        }
    }

    /// Test utility
    fn assert_lists<I, J, K>(a: I, b: J)
    where
        K: fmt::Debug + Eq,
        I: IntoIterator<Item = K>,
        J: IntoIterator<Item = K>,
    {
        let vec1: Vec<_> = a.into_iter().collect();
        let mut vec2: Vec<_> = b.into_iter().collect();

        assert_eq!(vec1.len(), vec2.len());

        for item1 in &vec1 {
            let idx2 = vec2
                .iter()
                .position(|item2| item1 == item2)
                .expect("Item not found in second list");

            let _ = vec2.swap_remove(idx2);
        }

        assert_eq!(vec2.len(), 0);
    }
}
