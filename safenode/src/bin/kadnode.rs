mod log;

use bincode::de;
use futures::{select, FutureExt};
use libp2p::Swarm;
use log::init_node_logging;

use futures::StreamExt;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{GetClosestPeersError, Kademlia, KademliaConfig, KademliaEvent, QueryResult};
use libp2p::{
    development_transport, identity, mdns,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    PeerId,
};
// use safenode::error::Result;
use eyre::{Error, Result};
use std::path::PathBuf;
use std::{env, time::Duration};
#[macro_use]
extern crate tracing;

// We create a custom network behaviour that combines Kademlia and mDNS.
// mDNS is for local discovery only
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "SafeNetBehaviour")]
struct MyBehaviour {
    kademlia: Kademlia<MemoryStore>,
    mdns: mdns::async_io::Behaviour,
}

impl MyBehaviour {
    fn get_closest_peers(&mut self, peer:PeerId){
        self.kademlia.get_closest_peers(peer);
    }
}

#[allow(clippy::large_enum_variant)]
enum SafeNetBehaviour {
    Kademlia(KademliaEvent),
    Mdns(mdns::Event),
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

#[derive(Debug)]
enum SwarmCmd {
    Search,
}

/// Channel to send Cmds to the swarm
type CmdChannel = tokio::sync::mpsc::Sender<SwarmCmd>;

fn run_swarm() -> CmdChannel {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<SwarmCmd>(1);

    let _handle = tokio::spawn(async move {
        // Create a random key for ourselves.
        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());

        // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol
        let transport = development_transport(keypair).await?;

        // Create a Kademlia instance and connect to the network address.
        // Create a swarm to manage peers and events.
        let mut swarm = {
            // Create a Kademlia behaviour.
            let mut cfg = KademliaConfig::default();
            cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let store = MemoryStore::new(local_peer_id);
            let kademlia = Kademlia::new(local_peer_id, store);
            let mdns = mdns::async_io::Behaviour::new(mdns::Config::default(), local_peer_id)?;
            let behaviour = MyBehaviour { kademlia, mdns };

            let mut swarm =
                SwarmBuilder::with_async_std_executor(transport, behaviour, local_peer_id).build();

            // Listen on all interfaces and whatever port the OS assigns.
            swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

            swarm
        };

        let net_info = swarm.network_info();

        debug!("network info: {net_info:?}");
        // Kick it off.
        loop {
            select! {
                cmd = receiver.recv().fuse() => {
                    debug!("Cmd innnnnnnnnnnnn: {cmd:?}");
                    if let Some(SwarmCmd::Search) =  cmd {
                        swarm.behaviour_mut().get_closest_peers(PeerId::random());
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
                    _ => {}
                }

            }
        }

        Ok::<(), Error>(())
    });

    sender
}

#[tokio::main]
async fn main() -> Result<()> {
    let log_dir = grab_log_dir();
    let _log_appender_guard = init_node_logging(&log_dir)?;

    let channel = run_swarm();

    channel.send(SwarmCmd::Search).await;
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    channel.send(SwarmCmd::Search).await;
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await
    }
    
    Ok(())
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

    if let Some(log_dir) = log_dir {
        Some(PathBuf::from(log_dir))
    } else {
        None
    }
}
