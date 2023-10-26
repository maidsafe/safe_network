// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use eyre::Result;
use libp2p::{Multiaddr, PeerId};
use safenode_proto::{
    safe_node_client::SafeNodeClient, GossipsubPublishRequest, GossipsubSubscribeRequest,
    GossipsubUnsubscribeRequest, NetworkInfoRequest, NodeEventsRequest, NodeInfoRequest,
    RecordAddressesRequest, RestartRequest, StopRequest, UpdateRequest,
};
use sn_logging::LogBuilder;
use sn_node::NodeEvent;
use sn_protocol::storage::SpendAddress;
use sn_transfers::Transfer;
use std::{fs, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};
use tokio_stream::StreamExt;
use tonic::Request;
use tracing_core::Level;
use tracing::{warn, info};

// this includes code generated from .proto files
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

#[derive(Parser, Debug)]
#[clap(name = "safenode RPC client")]
struct Opt {
    /// Address of the node's RPC service, e.g. 127.0.0.1:12001.
    addr: SocketAddr,
    /// subcommands
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Debug)]
enum Cmd {
    /// Retrieve information about the node itself
    #[clap(name = "info")]
    Info,
    /// Retrieve information about the node's connections to the network
    #[clap(name = "netinfo")]
    Netinfo,
    /// Start listening for node events.
    /// Note this blocks the app and it will print events as they are broadcasted by the node
    #[clap(name = "events")]
    Events,
    /// Start listening for transfers events.
    /// Note this blocks the app and it will print events as they are broadcasted by the node
    #[clap(name = "transfers")]
    TransfersEvents {
        /// Path where to store CashNotes received.
        /// Each CashNote is written to a separate file in respective
        /// recipient public address dir in the created cash_notes dir.
        /// Each file is named after the CashNote id.
        #[clap(name = "log-cash-notes")]
        log_cash_notes: Option<PathBuf>,
    },
    /// Subscribe to a given Gossipsub topic
    #[clap(name = "subscribe")]
    Subscribe {
        /// Name of the topic
        topic: String,
    },
    /// Unsubscribe from a given Gossipsub topic
    #[clap(name = "unsubscribe")]
    Unsubscribe {
        /// Name of the topic
        topic: String,
    },
    /// Publish a msg on a given Gossipsub topic
    #[clap(name = "publish")]
    Publish {
        /// Name of the topic
        topic: String,
        /// Message to publish
        msg: String,
    },
    /// Restart the node after the specified delay
    #[clap(name = "restart")]
    Restart {
        /// Delay in milliseconds before restartng the node
        #[clap(default_value = "0")]
        delay_millis: u64,
    },
    /// Stop the node after the specified delay
    #[clap(name = "stop")]
    Stop {
        /// Delay in milliseconds before stopping the node
        #[clap(default_value = "0")]
        delay_millis: u64,
    },
    /// Update to latest `safenode` released version, and restart it
    #[clap(name = "update")]
    Update {
        /// Delay in milliseconds before updating and restarting the node
        #[clap(default_value = "0")]
        delay_millis: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // For client, default to log to std::out
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard = LogBuilder::new(logging_targets).initialize()?;

    let opt = Opt::parse();
    let addr = opt.addr;

    match opt.cmd {
        Cmd::Info => node_info(addr).await,
        Cmd::Netinfo => network_info(addr).await,
        Cmd::Events => node_events(addr, false, None).await,
        Cmd::TransfersEvents { log_cash_notes } => node_events(addr, true, log_cash_notes).await,
        Cmd::Subscribe { topic } => gossipsub_subscribe(addr, topic).await,
        Cmd::Unsubscribe { topic } => gossipsub_unsubscribe(addr, topic).await,
        Cmd::Publish { topic, msg } => gossipsub_publish(addr, topic, msg).await,
        Cmd::Restart { delay_millis } => node_restart(addr, delay_millis).await,
        Cmd::Stop { delay_millis } => node_stop(addr, delay_millis).await,
        Cmd::Update { delay_millis } => node_update(addr, delay_millis).await,
    }
}

pub async fn node_info(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint.clone()).await?;
    let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
    let node_info = response.get_ref();
    let peer_id = PeerId::from_bytes(&node_info.peer_id)?;

    println!("Node info:");
    println!("==========");
    println!("RPC endpoint: {endpoint}");
    println!("Peer Id: {peer_id}");
    println!("Logs dir: {}", node_info.log_dir);
    println!("PID: {}", node_info.pid);
    println!("Binary version: {}", node_info.bin_version);
    println!(
        "Time since last restart: {:?}",
        Duration::from_secs(node_info.uptime_secs)
    );

    Ok(())
}

pub async fn network_info(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let response = client
        .network_info(Request::new(NetworkInfoRequest {}))
        .await?;
    let network_info = response.get_ref();

    println!("Node's connections to the Network:");
    println!();
    println!("Connected peers:");
    for bytes in network_info.connected_peers.iter() {
        let peer_id = PeerId::from_bytes(bytes)?;
        println!("Peer: {peer_id}");
    }

    println!();
    println!("Node's listeners:");
    for multiaddr_str in network_info.listeners.iter() {
        let multiaddr = Multiaddr::from_str(multiaddr_str)?;
        println!("Listener: {multiaddr}");
    }

    Ok(())
}

pub async fn node_events(
    addr: SocketAddr,
    only_transfers: bool,
    log_cash_notes: Option<PathBuf>,
) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let response = client
        .node_events(Request::new(NodeEventsRequest {}))
        .await?;

    if only_transfers {
        println!("Listening to transfers notifications... (press Ctrl+C to exit)");
        if let Some(ref path) = log_cash_notes {
            // create cash_notes dir
            fs::create_dir_all(path)?;
            println!("Writing cash notes to: {}", path.display());
        }
    } else {
        println!("Listening to node events... (press Ctrl+C to exit)");
    }
    println!();
    
    let mut stream = response.into_inner();
    while let Some(Ok(e)) = stream.next().await {
        match NodeEvent::from_bytes(&e.event) {
            Ok(NodeEvent::TransferNotif { key, transfers }) if only_transfers => {
                println!(
                    "New transfer notification received for {key:?}, containing {} transfer/s.",
                    transfers.len()
                );

                let mut cash_notes = vec![];
                for transfer in transfers {
                    match transfer {
                        Transfer::Encrypted(_) => match client
                            .network
                            .verify_and_unpack_transfer(transfer, wallet)
                            .await
                        {
                            // transfer not for us
                            Err(sn_protocol::Error::FailedToDecypherTransfer) => continue,
                            // transfer invalid
                            Err(e) => return Err(e),
                            // transfer ok
                            Ok(cns) => cash_notes = cns,
                        },
                        Transfer::NetworkRoyalties(_) => {
                            // we should always send transfers as they are lighter weight.
                            warn!("Unencrypted NetworkRoyalty received via TransferNotification. Ignoring it.");
                        }
                    }
                }

                // for cn in transfers {
                //     println!(
                //         "CashNote received with {:?}, value: {}",
                //         cn.unique_pubkey(),
                //         cn.value()?
                //     );

                //     if let Some(ref path) = log_cash_notes {
                //         // create cash_notes dir
                //         let unique_pubkey_name =
                //             *SpendAddress::from_unique_pubkey(&cn.unique_pubkey()).xorname();
                //         let unique_pubkey_file_name =
                //             format!("{}.cash_note", hex::encode(unique_pubkey_name));

                //         let cash_note_file_path = path.join(unique_pubkey_file_name);
                //         println!("Writing cash note to: {}", cash_note_file_path.display());

                //         let hex = cn.to_hex()?;
                //         fs::write(cash_note_file_path, &hex)?;
                //     }
                // }
                println!();
            }
            Ok(_) if only_transfers => continue,
            Ok(event) => println!("New event received: {event:?}"),
            Err(_) => {
                println!("Error while parsing received NodeEvent");
            }
        }
    }

    Ok(())
}

pub async fn record_addresses(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let response = client
        .record_addresses(Request::new(RecordAddressesRequest {}))
        .await?;

    println!("Records held by the node:");
    for bytes in response.get_ref().addresses.iter() {
        let key = libp2p::kad::RecordKey::from(bytes.clone());
        println!("Key: {key:?}");
    }

    Ok(())
}

pub async fn gossipsub_subscribe(addr: SocketAddr, topic: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .subscribe_to_topic(Request::new(GossipsubSubscribeRequest {
            topic: topic.clone(),
        }))
        .await?;
    println!("Node successfully received the request to subscribe to topic '{topic}'");
    Ok(())
}

pub async fn gossipsub_unsubscribe(addr: SocketAddr, topic: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .unsubscribe_from_topic(Request::new(GossipsubUnsubscribeRequest {
            topic: topic.clone(),
        }))
        .await?;
    println!("Node successfully received the request to unsubscribe from topic '{topic}'");
    Ok(())
}

pub async fn gossipsub_publish(addr: SocketAddr, topic: String, msg: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .publish_on_topic(Request::new(GossipsubPublishRequest {
            topic: topic.clone(),
            msg: msg.into(),
        }))
        .await?;
    println!("Node successfully received the request to publish on topic '{topic}'");
    Ok(())
}

pub async fn node_restart(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .restart(Request::new(RestartRequest { delay_millis }))
        .await?;
    println!(
        "Node successfully received the request to restart in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

pub async fn node_stop(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .stop(Request::new(StopRequest { delay_millis }))
        .await?;
    println!(
        "Node successfully received the request to stop in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

pub async fn node_update(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .update(Request::new(UpdateRequest { delay_millis }))
        .await?;
    println!(
        "Node successfully received the request to try to update in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}
