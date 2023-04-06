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

pub use self::{error::Error, event::NetworkEvent};

use self::{
    cmd::SwarmCmd,
    error::Result,
    event::NodeBehaviour,
    msg::{MsgCodec, MsgProtocol},
};
use crate::protocol::messages::{Request, Response};
use futures::StreamExt;
use libp2p::{
    core::muxing::StreamMuxerBox,
    identity,
    kad::{record::store::MemoryStore, Kademlia, KademliaConfig, QueryId},
    mdns,
    request_response::{self, ProtocolSupport, RequestId, ResponseChannel},
    swarm::{Swarm, SwarmBuilder},
    Multiaddr, PeerId, Transport,
};
use rand::Rng;
use std::{
    collections::{HashMap, HashSet},
    env, iter,
    process::{self, Command, Stdio},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;
use xor_name::XorName;

/// The main event loop recieves `SwarmEvents` from the network, `SwarmCmd` from the upper layers and
/// emmits back `NetworkEvent` to the upper layers.
/// Also keeps track of the pending queries/requests and their channels. Once we recieve an event
/// that is the outcome of a previously executed cmd, send a response to them via the stored channel.
pub struct NetworkSwarmLoop {
    swarm: Swarm<NodeBehaviour>,
    cmd_receiver: mpsc::Receiver<SwarmCmd>,
    event_sender: mpsc::Sender<NetworkEvent>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
    pending_get_closest_nodes: HashMap<QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_requests: HashMap<RequestId, oneshot::Sender<Result<Response>>>,
}

impl NetworkSwarmLoop {
    /// Creates the network components
    /// - The `Network` to interact with the network layer from anywhere
    ///   within your application.
    ///
    /// - The `NetworkEvent` receiver to get the events from the network layer.
    ///
    /// - The `NetworkSwarmLoop` that drives the network.
    pub fn new() -> Result<(Network, mpsc::Receiver<NetworkEvent>, NetworkSwarmLoop)> {
        // Create a random key for ourselves.
        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());

        info!("Local peer id: {:?}", local_peer_id);

        // QUIC configuration
        let quic_config = libp2p_quic::Config::new(&keypair);
        let transport = libp2p_quic::tokio::Transport::new(quic_config);
        let transport = transport
            .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
            .boxed();
        // Create a Kademlia instance and connect to the network address.
        // Create a swarm to manage peers and events.
        let swarm = {
            // Create a Kademlia behaviour.
            let mut cfg = KademliaConfig::default();
            let _ = cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let kademlia =
                Kademlia::with_config(local_peer_id, MemoryStore::new(local_peer_id), cfg);
            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
            let behaviour = NodeBehaviour {
                request_response: request_response::Behaviour::new(
                    MsgCodec(),
                    iter::once((MsgProtocol(), ProtocolSupport::Full)),
                    Default::default(),
                ),
                kademlia,
                mdns,
            };

            let mut swarm =
                SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

            // Listen on all interfaces and whatever port the OS assigns.
            let addr = "/ip4/0.0.0.0/udp/0/quic-v1"
                .parse()
                .expect("Failed to parse the address");
            let _listener_id = swarm
                .listen_on(addr)
                .expect("Failed to listen on the provided address");

            swarm
        };

        let (swarm_cmd_sender, swarm_cmd_receiver) = mpsc::channel(100);
        let (event_sender, event_receiver) = mpsc::channel(100);
        let event_loop = Self {
            swarm,
            cmd_receiver: swarm_cmd_receiver,
            event_sender,
            pending_dial: Default::default(),
            pending_get_closest_nodes: Default::default(),
            pending_requests: Default::default(),
        };

        Ok((Network { swarm_cmd_sender }, event_receiver, event_loop))
    }

    /// Drive the network
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                some_event = self.swarm.next() => {
                    // TODO: currently disabled to provide a stable network.
                    // restart_at_random(self.swarm.local_peer_id());
                    if let Err(err) = self.handle_swarm_events(some_event.expect("Swarm stream to be infinite!")).await {
                        warn!("Error while handling event: {err}");
                    }
                }  ,
                some_cmd = self.cmd_receiver.recv() => match some_cmd {
                    Some(cmd) => {
                        if let Err(err) = self.handle_cmd(cmd) {
                            warn!("Error while handling cmd: {err}");
                        }
                    },
                    // Cmd channel closed, thus shutting down the network event loop.
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

    /// Find the providers for the given piece of data; The XorName is used to locate the nodes
    /// that hold the data
    pub async fn get_closest_nodes(&self, xor_name: XorName) -> Result<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestNodes { xor_name, sender })
            .await?;
        let closest_nodes = receiver.await?;
        trace!("Got the closest_nodes to the given XorName-{xor_name}, nodes: {closest_nodes:?}");
        Ok(closest_nodes)
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

    // helper to send SwarmCmd
    async fn send_swarm_cmd(&self, cmd: SwarmCmd) -> Result<()> {
        let swarm_cmd_sender = self.swarm_cmd_sender.clone();
        swarm_cmd_sender.send(cmd).await?;
        Ok(())
    }
}
