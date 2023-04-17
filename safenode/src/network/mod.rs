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

use crate::protocol::messages::{Request, Response};

pub use self::{error::Error, event::NetworkEvent};

use self::{
    cmd::SwarmCmd,
    error::Result,
    event::NodeBehaviour,
    msg::{MsgCodec, MsgProtocol},
};

use futures::StreamExt;
use libp2p::{
    core::muxing::StreamMuxerBox,
    identity,
    kad::{record::store::MemoryStore, KBucketKey, Kademlia, KademliaConfig, QueryId},
    mdns,
    multiaddr::Protocol,
    request_response::{self, ProtocolSupport, RequestId, ResponseChannel},
    swarm::{Swarm, SwarmBuilder},
    Multiaddr, PeerId, Transport,
};
use rand::Rng;
use std::{
    collections::{HashMap, HashSet},
    env, iter,
    net::SocketAddr,
    process::{self, Command, Stdio},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;
use xor_name::XorName;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
pub(crate) const CLOSE_GROUP_SIZE: usize = 8;

/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    CLOSE_GROUP_SIZE / 2 + 1
}

type PendingGetClosest =
    HashMap<QueryId, (oneshot::Sender<(PeerId, HashSet<PeerId>)>, HashSet<PeerId>)>;

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
}

impl SwarmDriver {
    /// Creates a new `SwarmDriver` instance, along with a `Network` handle
    /// for sending commands and an `mpsc::Receiver<NetworkEvent>` for receiving
    /// network events. It initializes the swarm, sets up the transport, and
    /// configures the Kademlia and mDNS behaviors for peer discovery.
    ///
    /// # Returns
    ///
    /// A tuple containing a `Network` handle, an `mpsc::Receiver<NetworkEvent>`,
    /// and a `SwarmDriver` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the mDNS behavior.
    pub fn new(addr: SocketAddr) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        let mut cfg = KademliaConfig::default();
        let _ = cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let _ = cfg.set_connection_idle_timeout(Duration::from_secs(10 * 60));

        let request_response = request_response::Behaviour::new(
            MsgCodec(),
            iter::once((MsgProtocol(), ProtocolSupport::Full)),
            Default::default(),
        );

        let (network, events_receiver, mut swarm_driver) = Self::with(cfg, request_response)?;

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
        let cfg = KademliaConfig::default(); // default query timeout is 60 secs
        let request_response = request_response::Behaviour::new(
            MsgCodec(),
            iter::once((MsgProtocol(), ProtocolSupport::Outbound)),
            Default::default(),
        );

        Self::with(cfg, request_response)
    }

    // Private helper to create the network components with the provided config and req/res behaviour
    fn with(
        cfg: KademliaConfig,
        request_response: request_response::Behaviour<MsgCodec>,
    ) -> Result<(Network, mpsc::Receiver<NetworkEvent>, SwarmDriver)> {
        // Create a random key for ourself.
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        info!("Peer id: {:?}", peer_id);

        // QUIC configuration
        let quic_config = libp2p_quic::Config::new(&keypair);
        let transport = libp2p_quic::tokio::Transport::new(quic_config);
        let transport = transport
            .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
            .boxed();

        // Create a Kademlia behaviour for client mode, i.e. set req/resp protocol
        // to outbound-only mode and don't listen on any address
        let kademlia = Kademlia::with_config(peer_id, MemoryStore::new(peer_id), cfg);
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
        let behaviour = NodeBehaviour {
            request_response,
            kademlia,
            mdns,
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
                some_event = self.swarm.next() => {
                    // TODO: currently disabled to provide a stable network.
                    // restart_at_random(self.swarm.local_peer_id());
                    if let Err(err) = self.handle_swarm_events(some_event.expect("Swarm stream to be infinite!")).await {
                        warn!("Error while handling event: {err}");
                    }
                },
                some_cmd = self.cmd_receiver.recv() => match some_cmd {
                    Some(cmd) => {
                        if let Err(err) = self.handle_cmd(cmd) {
                            warn!("Error while handling cmd: {err}");
                        }
                    },
                    None =>  return,
                },
            }
        }
    }
}

/// Restarts the whole program.
/// It does this at random, one in X times called.
///
/// This provides a way to test the network layer's ability to recover from
/// unexpected shutdowns.
#[allow(dead_code)]
fn restart_at_random(peer_id: &PeerId) {
    let mut rng = rand::thread_rng();
    let random_num = rng.gen_range(0..500);

    if random_num == 0 {
        warn!("Restarting {peer_id:?} at random!");

        let ten_millis = std::time::Duration::from_millis(10);
        std::thread::sleep(ten_millis);

        // Get the current executable's path
        let executable = env::current_exe().expect("Failed to get current executable path");

        info!("Spawned executable: {executable:?}");

        // Spawn a new process to restart the binary with the same arguments and environment
        let _ = Command::new(executable)
            .args(env::args().skip(1))
            .envs(env::vars())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to restart the app.");

        debug!("New exec called.");
        // exit the current process now that we've spawned a new one
        process::exit(0);
    }
}

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct Network {
    pub(super) swarm_cmd_sender: mpsc::Sender<SwarmCmd>,
    #[allow(dead_code)]
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

    /// Returns the closest peers to the given `XorName`, sorted by their distance to the xor_name.
    /// If `client` is false, then include `self` among the `closest_peers`
    async fn get_closest_peers(&self, xor_name: XorName, client: bool) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestPeers { xor_name, sender })
            .await?;
        let (our_id, k_bucket_peers) = receiver.await?;

        // Count self in if among the CLOSE_GROUP_SIZE closest and sort the result
        let mut closest_peers: Vec<_> = k_bucket_peers.into_iter().collect();
        if !client {
            closest_peers.push(our_id);
        }
        let target = KBucketKey::new(xor_name.0.to_vec());
        closest_peers.sort_by(|a, b| {
            let a = KBucketKey::new(a.to_bytes());
            let b = KBucketKey::new(b.to_bytes());
            target.distance(&a).cmp(&target.distance(&b))
        });
        let closest_peers: Vec<PeerId> = closest_peers
            .iter()
            .take(CLOSE_GROUP_SIZE)
            .cloned()
            .collect();

        if CLOSE_GROUP_SIZE > closest_peers.len() {
            warn!("Not enough peers in the k-bucket to satisfy the request");
            return Err(Error::NotEnoughPeers);
        }

        trace!(
            "Got the {} closest_peers to the given XorName-{xor_name}, nodes: {closest_peers:?}",
            closest_peers.len()
        );

        Ok(closest_peers)
    }

    /// Send `Request` to the the given `PeerId`
    pub async fn send_request(&self, req: Request, peer: PeerId) -> Result<Response> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::SendRequest { req, peer, sender })
            .await?;
        receiver.await?
    }

    /// Send a `Response` through the channel opened by the requester.
    pub async fn send_response(
        &self,
        resp: Response,
        channel: ResponseChannel<Response>,
    ) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SendResponse { resp, channel })
            .await
    }

    // Helper to send SwarmCmd
    async fn send_swarm_cmd(&self, cmd: SwarmCmd) -> Result<()> {
        let swarm_cmd_sender = self.swarm_cmd_sender.clone();
        swarm_cmd_sender.send(cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SwarmDriver;
    use crate::log::init_node_logging;
    use eyre::{eyre, Result};
    use libp2p::{
        kad::{
            kbucket::{Entry, InsertResult, KBucketsTable, NodeStatus},
            KBucketKey,
        },
        PeerId,
    };
    use rand::thread_rng;
    use std::{
        collections::{BTreeMap, HashMap},
        fmt,
        net::SocketAddr,
        time::Duration,
    };
    use xor_name::XorName;

    #[tokio::test(flavor = "multi_thread")]
    async fn closest() -> Result<()> {
        let _ = init_node_logging(&None)?;
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
