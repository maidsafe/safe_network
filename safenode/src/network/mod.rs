// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod api;
mod command;
mod error;
mod event;
mod safe_msg;

pub use self::{
    event::NetworkEvent,
    safe_msg::{SafeRequest, SafeResponse},
};

use self::{
    api::NetworkApi,
    command::CmdToSwarm,
    error::Result,
    event::SafeNodeBehaviour,
    safe_msg::{SafeMsgCodec, SafeMsgProtocol},
};
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use libp2p::{
    core::muxing::StreamMuxerBox,
    identity,
    kad::{record::store::MemoryStore, Kademlia, KademliaConfig, QueryId},
    mdns,
    request_response::{self, ProtocolSupport, RequestId},
    swarm::{Swarm, SwarmBuilder},
    PeerId, Transport,
};
use std::{
    collections::{HashMap, HashSet},
    iter,
    time::Duration,
};
use tracing::warn;

pub struct EventLoop {
    swarm: Swarm<SafeNodeBehaviour>,
    command_receiver: mpsc::Receiver<CmdToSwarm>,
    event_sender: mpsc::Sender<NetworkEvent>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
    pending_start_providing: HashMap<QueryId, oneshot::Sender<Result<()>>>,
    pending_get_providers: HashMap<QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_safe_requests: HashMap<RequestId, oneshot::Sender<Result<SafeResponse>>>,
}

impl EventLoop {
    /// Creates the network components, namely:
    ///
    /// - The network client to interact with the network layer from anywhere
    ///   within your application.
    ///
    /// - The network event stream, e.g. for incoming requests.
    ///
    /// - The network task driving the network itself.
    pub fn new(// secret_key_seed: Option<u8>,
    ) -> Result<(NetworkApi, impl Stream<Item = NetworkEvent>, EventLoop)> {
        // Create a random key for ourselves.
        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());

        // QUIC configuration
        let quic_config = libp2p_quic::Config::new(&keypair);
        let transport = libp2p_quic::async_std::Transport::new(quic_config);
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
            let mdns = mdns::async_io::Behaviour::new(mdns::Config::default(), local_peer_id)?;
            let behaviour = SafeNodeBehaviour {
                request_response: request_response::Behaviour::new(
                    SafeMsgCodec(),
                    iter::once((SafeMsgProtocol(), ProtocolSupport::Full)),
                    Default::default(),
                ),
                kademlia,
                mdns,
            };

            let mut swarm =
                SwarmBuilder::with_async_std_executor(transport, behaviour, local_peer_id).build();

            // // Listen on all interfaces and whatever port the OS assigns.
            let addr = "/ip4/0.0.0.0/udp/0/quic-v1".parse().expect("addr okay");
            let _listener_id = swarm.listen_on(addr).expect("listening failed");

            swarm
        };

        let (command_sender, command_receiver) = mpsc::channel(0);
        let (event_sender, event_receiver) = mpsc::channel(0);
        let event_loop = Self {
            swarm,
            command_receiver,
            event_sender,
            pending_dial: Default::default(),
            pending_start_providing: Default::default(),
            pending_get_providers: Default::default(),
            pending_safe_requests: Default::default(),
        };

        Ok((
            NetworkApi {
                sender: command_sender,
            },
            event_receiver,
            event_loop,
        ))
    }

    pub async fn run(mut self) {
        loop {
            futures::select! {
                event = self.swarm.next() => {
                    if let Err(err) = self.handle_event(event.expect("Swarm stream to be infinite!")).await {
                        warn!("Error while handling event: {err}");
                    }
                }  ,
                command = self.command_receiver.next() => match command {
                    Some(cmd) => {
                        if let Err(err) = self.handle_command(cmd) {
                            warn!("Error while handling cmd: {err}");
                        }
                    },
                    // Command channel closed, thus shutting down the network event loop.
                    None=>  return,
                },
            }
        }
    }
}
