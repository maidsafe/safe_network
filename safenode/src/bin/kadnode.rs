mod log;
mod protocol;

use log::init_node_logging;
use protocol::{SafeCodec, SafeProtocol, SafeRequest, SafeResponse};

use bytes::Bytes;
use eyre::{Error, Result};
use futures::{select, FutureExt, StreamExt};
use libp2p::{
    core::muxing::StreamMuxerBox,
    identity,
    kad::{
        record::store::MemoryStore, GetProvidersOk, Kademlia, KademliaConfig, KademliaEvent,
        QueryId, QueryResult,
    },
    mdns,
    request_response::{self, Event, Message},
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    PeerId, Transport,
};
use std::{path::PathBuf, time::Duration};
use tracing::{debug, error, info, trace};
use xor_name::XorName;

// We create a custom network behaviour that combines Kademlia and mDNS.
// mDNS is for local discovery only
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "SafeNetBehaviour")]
struct MyBehaviour {
    kademlia: Kademlia<MemoryStore>,
    req_resp: request_response::Behaviour<SafeCodec>,
    mdns: mdns::async_io::Behaviour,
}

impl MyBehaviour {
    fn get_closest_peers_to_xorname(&mut self, addr: XorName) {
        self.kademlia.get_closest_peers(addr.to_vec());
    }

    fn send_request(&mut self, query_id: &QueryId, closest_peer: Option<&PeerId>) {
        if let Some(peer_id) = closest_peer {
            info!("Found provider for query {query_id:?}: {peer_id:?}");
            let req = SafeRequest(Bytes::from("hello").to_vec());
            let req_id = self.req_resp.send_request(peer_id, req);
            info!("Request sent to {peer_id:?}: {req_id:?}");
        } else {
            trace!("No closest peer reported for query {query_id:?} yet");
        }
    }

    fn send_response(
        &mut self,
        channel: request_response::ResponseChannel<SafeResponse>,
        resp: SafeResponse,
    ) -> Result<(), SafeResponse> {
        self.req_resp.send_response(channel, resp)
    }
}

#[allow(clippy::large_enum_variant)]
enum SafeNetBehaviour {
    Kademlia(KademliaEvent),
    Mdns(mdns::Event),
    ReqResp(request_response::Event<SafeRequest, SafeResponse>),
}

impl From<KademliaEvent> for SafeNetBehaviour {
    fn from(event: KademliaEvent) -> Self {
        SafeNetBehaviour::Kademlia(event)
    }
}

impl From<mdns::Event> for SafeNetBehaviour {
    fn from(event: mdns::Event) -> Self {
        SafeNetBehaviour::Mdns(event)
    }
}

impl From<request_response::Event<SafeRequest, SafeResponse>> for SafeNetBehaviour {
    fn from(event: request_response::Event<SafeRequest, SafeResponse>) -> Self {
        SafeNetBehaviour::ReqResp(event)
    }
}

#[derive(Debug)]
enum SwarmCmd {
    Search(XorName),
    Get(XorName),
}

/// Channel to send Cmds to the swarm
type CmdChannel = tokio::sync::mpsc::Sender<SwarmCmd>;

fn run_swarm() -> CmdChannel {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<SwarmCmd>(1);

    let _handle: tokio::task::JoinHandle<Result<(), Error>> = tokio::spawn(async move {
        debug!("Starting swarm");
        // Create a random key for ourselves.
        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());
        info!("My PeerId is {local_peer_id}");

        // QUIC configuration
        let quic_config = libp2p_quic::Config::new(&keypair);
        let transport = libp2p_quic::async_std::Transport::new(quic_config);
        let transport = transport
            .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
            .boxed();

        // Create a Kademlia instance and connect to the network address.
        // Create a swarm to manage peers and events.
        let mut swarm = {
            // Create a Kademlia behaviour.
            let mut cfg = KademliaConfig::default();
            cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let store = MemoryStore::new(local_peer_id);
            let kademlia = Kademlia::new(local_peer_id, store);
            let mdns = mdns::async_io::Behaviour::new(mdns::Config::default(), local_peer_id)?;

            let protocols =
                std::iter::once((SafeProtocol(), request_response::ProtocolSupport::Full));
            let cfg = request_response::Config::default();
            let req_resp = request_response::Behaviour::new(SafeCodec(), protocols, cfg);

            let behaviour = MyBehaviour {
                kademlia,
                req_resp,
                mdns,
            };

            let mut swarm =
                SwarmBuilder::with_async_std_executor(transport, behaviour, local_peer_id).build();

            // // Listen on all interfaces and whatever port the OS assigns.
            let addr = "/ip4/0.0.0.0/udp/0/quic-v1".parse().expect("addr okay");
            swarm.listen_on(addr).expect("listening failed");

            swarm
        };

        let net_info = swarm.network_info();

        debug!("network info: {net_info:?}");
        // Kick it off.
        loop {
            select! {
                cmd = receiver.recv().fuse() => {
                    debug!("Cmd in: {cmd:?}");
                    match cmd {
                        Some(SwarmCmd::Search(xor_name)) => swarm.behaviour_mut().get_closest_peers_to_xorname(xor_name),
                        Some(SwarmCmd::Get(xor_name)) => {
                            let key = xor_name.to_vec().into();
                            // TODO: double check if we need to also try to get_closest_peers_to_xorname
                            let query_id = swarm.behaviour_mut().kademlia.get_providers(key);
                            info!("Request sent to get providers: {query_id:?}");
                        }
                        None => {}
                    }
                }

                event = swarm.select_next_some() => match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening in {address:?}");
                    },
                    SwarmEvent::Behaviour(SafeNetBehaviour::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            info!("Node discovered: {multiaddr:?}");
                            swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                        }
                    }
                    SwarmEvent::Behaviour(SafeNetBehaviour::Kademlia(KademliaEvent::OutboundQueryProgressed {
                        result: QueryResult::GetClosestPeers(result),
                        ..
                    })) => {
                        info!("Result for closest peers is in! {result:?}");
                    }
                    SwarmEvent::Behaviour(SafeNetBehaviour::Kademlia(KademliaEvent::OutboundQueryProgressed{
                        id,
                        result: QueryResult::GetProviders(Ok(
                            GetProvidersOk::FoundProviders { providers, .. }
                        )),
                        ..
                    })) => {
                        swarm.behaviour_mut().send_request(&id, providers.iter().next());
                    }
                    SwarmEvent::Behaviour(SafeNetBehaviour::Kademlia(KademliaEvent::OutboundQueryProgressed{
                        id,
                        result: QueryResult::GetProviders(Ok(
                            GetProvidersOk::FinishedWithNoAdditionalRecord { closest_peers, .. }
                        )),
                        ..
                    })) => {
                        swarm.behaviour_mut().send_request(&id, closest_peers.first());
                    }

                    // Request/Response events handling

                    SwarmEvent::Behaviour(SafeNetBehaviour::ReqResp(Event::Message {
                        message: Message::Request { request, channel, .. },
                        ..
                    })) => {
                         info!("Request received: {request:?}");
                         let resp = SafeResponse(Bytes::from("world!").to_vec());
                         if let Err(resp) = swarm.behaviour_mut().send_response(channel, resp) {
                             error!("Faied to send response: {resp:?}");
                         }
                    }
                    SwarmEvent::Behaviour(SafeNetBehaviour::ReqResp(Event::Message {
                        message: Message::Response { request_id, response, .. },
                        ..
                    })) => {
                         info!("Response for request {request_id:?}: {response:?}");

                         // TODO: send response back to the client...
                         // ...or have the client to be another peer in the Kad

                    }
                    // SwarmEvent::Behaviour(SafeNetBehaviour::Kademlia(KademliaEvent::RoutingUpdated{addresses, ..})) => {
                    //     trace!("Kad routing updated: {addresses:?}");
                    // }
                    // SwarmEvent::Behaviour(SafeNetBehaviour::Kademlia(KademliaEvent::OutboundQueryProgressed { result, ..})) => {
                    //     match result {
                    //         // QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders { key, providers, .. })) => {
                    //         //     for peer in providers {
                    //         //         println!(
                    //         //             "Peer {peer:?} provides key {:?}",
                    //         //             std::str::from_utf8(key.as_ref()).unwrap()
                    //         //         );
                    //         //     }
                    //         // }
                    //         // QueryResult::GetProviders(Err(err)) => {
                    //         //     eprintln!("Failed to get providers: {err:?}");
                    //         // }
                    //         // QueryResult::GetRecord(Ok(
                    //         //     GetRecordOk::FoundRecord(PeerRecord {
                    //         //         record: Record { key, value, .. },
                    //         //         ..
                    //         //     })
                    //         // )) => {
                    //         //     println!(
                    //         //         "Got record {:?} {:?}",
                    //         //         std::str::from_utf8(key.as_ref()).unwrap(),
                    //         //         std::str::from_utf8(&value).unwrap(),
                    //         //     );
                    //         // }
                    //         // QueryResult::GetRecord(Ok(_)) => {}
                    //         // QueryResult::GetRecord(Err(err)) => {
                    //         //     eprintln!("Failed to get record: {err:?}");
                    //         // }
                    //         // QueryResult::PutRecord(Ok(PutRecordOk { key })) => {
                    //         //     println!(
                    //         //         "Successfully put record {:?}",
                    //         //         std::str::from_utf8(key.as_ref()).unwrap()
                    //         //     );
                    //         // }
                    //         // QueryResult::PutRecord(Err(err)) => {
                    //         //     eprintln!("Failed to put record: {err:?}");
                    //         // }
                    //         // QueryResult::StartProviding(Ok(AddProviderOk { key })) => {
                    //         //     println!(
                    //         //         "Successfully put provider record {:?}",
                    //         //         std::str::from_utf8(key.as_ref()).unwrap()
                    //         //     );
                    //         // }
                    //         // QueryResult::StartProviding(Err(err)) => {
                    //         //     eprintln!("Failed to put provider record: {err:?}");
                    //         // }
                    //         _ => {
                    //             //
                    //         }
                    //     }
                    // }
                    _ => { /* trace!("Other type of SwarmEvent we are not handling!") */ }
                }

            }
        }
    });

    sender
}

#[tokio::main]
async fn main() -> Result<()> {
    let log_dir = grab_log_dir();
    let _log_appender_guard = init_node_logging(&log_dir)?;

    info!("start");
    let channel = run_swarm();

    let x = xor_name::XorName::from_content(b"some random content here for you");

    if let Err(e) = channel.send(SwarmCmd::Search(x)).await {
        debug!("Error while sending SwarmCmd: {e}");
    }

    tokio::time::sleep(Duration::from_secs(5)).await;

    if let Err(e) = channel.send(SwarmCmd::Search(x)).await {
        debug!("Error while sending SwarmCmd: {e}");
    }

    if let Err(e) = channel.send(SwarmCmd::Get(x)).await {
        debug!("Error while sending SwarmCmd::Get: {e}");
    }

    loop {
        tokio::time::sleep(Duration::from_millis(5000)).await;
    }
}

/// Grabs the log dir arg if passed in
fn grab_log_dir() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1); // Skip the first argument (the program name)

    let mut log_dir = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--log-dir" => {
                log_dir = args.next();
            }
            _ => {
                println!("Unknown argument: {}", arg);
            }
        }
    }
    log_dir.map(PathBuf::from)
}
