// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk_codec;
mod command;

use futures::channel::{mpsc, oneshot};
use futures::prelude::*;
use libp2p::{
    core::muxing::StreamMuxerBox,
    identity,
    kad::{
        record::store::MemoryStore, GetProvidersOk, Kademlia, KademliaConfig, KademliaEvent,
        QueryId, QueryResult,
    },
    multiaddr::Protocol,
    request_response::{self, ProtocolSupport, RequestId, ResponseChannel},
    swarm::{NetworkBehaviour, Swarm, SwarmBuilder, SwarmEvent},
    PeerId,
};
use libp2p::{mdns, Multiaddr, Transport};
use std::collections::{HashMap, HashSet};
use std::{error::Error, iter, time::Duration};
use tracing::info;
use xor_name::XorName;

use self::chunk_codec::{ChunkRequest, ChunkResponse, ChunkStorageCodec, ChunkStorageProtocol};
use self::command::CmdToSwarm;

#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<CmdToSwarm>,
}

impl Client {
    //  Listen for incoming connections on the given address.
    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::StartListening { addr, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Dial the given peer at the given address.
    pub async fn dial(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Advertise the local node as the provider of the given file on the DHT.
    pub async fn store_chunk(&mut self, xor_name: XorName) {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::StoreChunk { xor_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.");
    }

    /// Find the providers for the given file on the DHT.
    pub async fn get_chunk_providers(&mut self, xor_name: XorName) -> HashSet<PeerId> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::GetChunkProviders { xor_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    /// Request the content of the given file from the given peer.
    pub async fn request_chunk(
        &mut self,
        peer: PeerId,
        xor_name: XorName,
    ) -> Result<Vec<u8>, Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(CmdToSwarm::RequestChunk {
                xor_name,
                peer,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not be dropped.")
    }

    /// Respond with the provided file content to the given request.
    pub async fn respond_chunk(&mut self, file: Vec<u8>, channel: ResponseChannel<ChunkResponse>) {
        self.sender
            .send(CmdToSwarm::RespondChunk { file, channel })
            .await
            .expect("Command receiver not to be dropped.");
    }
}

pub struct EventLoop {
    swarm: Swarm<SafeNodeBehaviour>,
    command_receiver: mpsc::Receiver<CmdToSwarm>,
    event_sender: mpsc::Sender<Event>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<(), Box<dyn Error + Send>>>>,
    pending_start_providing: HashMap<QueryId, oneshot::Sender<()>>,
    pending_get_providers: HashMap<QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_request_file:
        HashMap<RequestId, oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>>,
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
    pub async fn new(// secret_key_seed: Option<u8>,
    ) -> Result<(Client, impl Stream<Item = Event>, EventLoop), Box<dyn Error>> {
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
            cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let kademlia =
                Kademlia::with_config(local_peer_id, MemoryStore::new(local_peer_id), cfg);
            let mdns = mdns::async_io::Behaviour::new(mdns::Config::default(), local_peer_id)?;
            let behaviour = SafeNodeBehaviour {
                request_response: request_response::Behaviour::new(
                    ChunkStorageCodec(),
                    iter::once((ChunkStorageProtocol(), ProtocolSupport::Full)),
                    Default::default(),
                ),
                kademlia,
                mdns,
            };

            let mut swarm =
                SwarmBuilder::with_async_std_executor(transport, behaviour, local_peer_id).build();

            // // Listen on all interfaces and whatever port the OS assigns.
            let addr = "/ip4/0.0.0.0/udp/0/quic-v1".parse().expect("addr okay");
            swarm.listen_on(addr).expect("listening failed");

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
            pending_request_file: Default::default(),
        };

        Ok((
            Client {
                sender: command_sender,
            },
            event_receiver,
            event_loop,
        ))
    }

    pub async fn run(mut self) {
        loop {
            futures::select! {
                event = self.swarm.next() => self.handle_event(event.expect("Swarm stream to be infinite.")).await  ,
                command = self.command_receiver.next() => match command {
                    Some(c) => self.handle_command(c).await,
                    // Command channel closed, thus shutting down the network event loop.
                    None=>  return,
                },
            }
        }
    }

    async fn handle_event<THandleErr: std::error::Error>(
        &mut self,
        event: SwarmEvent<SafeNodeEvent, THandleErr>,
    ) {
        match event {
            SwarmEvent::Behaviour(SafeNodeEvent::Kademlia(
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result: QueryResult::StartProviding(_),
                    ..
                },
            )) => {
                let sender: oneshot::Sender<()> = self
                    .pending_start_providing
                    .remove(&id)
                    .expect("Completed query to be previously pending.");
                let _ = sender.send(());
            }
            SwarmEvent::Behaviour(SafeNodeEvent::Kademlia(
                KademliaEvent::OutboundQueryProgressed {
                    id,
                    result:
                        QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders {
                            providers, ..
                        })),
                    ..
                },
            )) => {
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    sender.send(providers).expect("Receiver not to be dropped");

                    // Finish the query. We are only interested in the first result.
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .unwrap()
                        .finish();
                }
            }
            SwarmEvent::Behaviour(SafeNodeEvent::Kademlia(
                KademliaEvent::OutboundQueryProgressed {
                    result:
                        QueryResult::GetProviders(Ok(GetProvidersOk::FinishedWithNoAdditionalRecord {
                            ..
                        })),
                    ..
                },
            )) => {}
            SwarmEvent::Behaviour(SafeNodeEvent::Kademlia(_)) => {}
            SwarmEvent::Behaviour(SafeNodeEvent::RequestResponse(
                request_response::Event::Message { message, .. },
            )) => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    self.event_sender
                        .send(Event::InboundChunkRequest {
                            xor_name: request.0,
                            channel,
                        })
                        .await
                        .expect("Event receiver not to be dropped.");
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    let _ = self
                        .pending_request_file
                        .remove(&request_id)
                        .expect("Request to still be pending.")
                        .send(Ok(response.0));
                }
            },
            SwarmEvent::Behaviour(SafeNodeEvent::RequestResponse(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => {
                let _ = self
                    .pending_request_file
                    .remove(&request_id)
                    .expect("Request to still be pending.")
                    .send(Err(Box::new(error)));
            }
            SwarmEvent::Behaviour(SafeNodeEvent::RequestResponse(
                request_response::Event::ResponseSent { .. },
            )) => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                info!(
                    "Local node is listening on {:?}",
                    address.with(Protocol::P2p(local_peer_id.into()))
                );
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                if endpoint.is_dialer() {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(Box::new(error)));
                    }
                }
            }
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::Dialing(peer_id) => info!("Dialing {peer_id}"),
            SwarmEvent::Behaviour(SafeNodeEvent::Mdns(mdns_event)) => match *mdns_event {
                mdns::Event::Discovered(list) => {
                    for (peer_id, multiaddr) in list {
                        info!("Node discovered: {multiaddr:?}");
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, multiaddr);
                    }
                }
                mdns::Event::Expired(_) => todo!(),
            },
            e => panic!("{e:?}"),
        }
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "SafeNodeEvent")]
struct SafeNodeBehaviour {
    request_response: request_response::Behaviour<ChunkStorageCodec>,
    kademlia: Kademlia<MemoryStore>,
    mdns: mdns::async_io::Behaviour,
}

#[derive(Debug)]
enum SafeNodeEvent {
    RequestResponse(request_response::Event<ChunkRequest, ChunkResponse>),
    Kademlia(KademliaEvent),
    Mdns(Box<mdns::Event>),
}

impl From<request_response::Event<ChunkRequest, ChunkResponse>> for SafeNodeEvent {
    fn from(event: request_response::Event<ChunkRequest, ChunkResponse>) -> Self {
        SafeNodeEvent::RequestResponse(event)
    }
}

impl From<KademliaEvent> for SafeNodeEvent {
    fn from(event: KademliaEvent) -> Self {
        SafeNodeEvent::Kademlia(event)
    }
}

impl From<mdns::Event> for SafeNodeEvent {
    fn from(event: mdns::Event) -> Self {
        SafeNodeEvent::Mdns(Box::new(event))
    }
}

#[derive(Debug)]
pub enum Event {
    InboundChunkRequest {
        xor_name: XorName,
        channel: ResponseChannel<ChunkResponse>,
    },
}
