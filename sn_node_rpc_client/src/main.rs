// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
//

use sn_node_rpc_client::{RpcActions, RpcClient};

use assert_fs::TempDir;
use bls::SecretKey;
use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use libp2p::Multiaddr;
use sn_client::Client;
use sn_logging::LogBuilder;
use sn_node::{NodeEvent, ROYALTY_TRANSFER_NOTIF_TOPIC};
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, GossipsubSubscribeRequest, NodeEventsRequest,
    TransferNotifsFilterRequest,
};
use sn_protocol::storage::SpendAddress;
use sn_transfers::{MainPubkey, WatchOnlyWallet};
use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};
use tokio_stream::StreamExt;
use tonic::Request;
use tracing_core::Level;

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
        /// The hex-encoded BLS secret key to decrypt the transfers received and convert
        /// them into spendable CashNotes.
        sk: String,
        /// Path where to store CashNotes received.
        /// Each CashNote is written to a separate file in respective
        /// recipient public address dir in the created cash_notes dir.
        /// Each file is named after the CashNote id.
        #[clap(name = "log-cash-notes")]
        log_cash_notes: Option<PathBuf>,

        #[command(flatten)]
        peers: PeersArgs,
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
        /// Retain the node's PeerId by reusing the same root dir.
        retain_peer_id: bool,
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
        Cmd::Events => node_events(addr).await,
        Cmd::TransfersEvents {
            sk,
            log_cash_notes,
            peers,
        } => {
            let bootstrap_peers = get_peers_from_args(peers).await?;
            let bootstrap_peers = if bootstrap_peers.is_empty() {
                // empty vec is returned if `local-discovery` flag is provided
                None
            } else {
                Some(bootstrap_peers)
            };

            transfers_events(addr, sk, log_cash_notes, bootstrap_peers).await
        }
        Cmd::Subscribe { topic } => gossipsub_subscribe(addr, topic).await,
        Cmd::Unsubscribe { topic } => gossipsub_unsubscribe(addr, topic).await,
        Cmd::Publish { topic, msg } => gossipsub_publish(addr, topic, msg).await,
        Cmd::Restart {
            delay_millis,
            retain_peer_id,
        } => node_restart(addr, delay_millis, retain_peer_id).await,
        Cmd::Stop { delay_millis } => node_stop(addr, delay_millis).await,
        Cmd::Update { delay_millis } => node_update(addr, delay_millis).await,
    }
}

pub async fn node_info(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    let node_info = client.node_info().await?;

    println!("Node info:");
    println!("==========");
    println!("RPC endpoint: {endpoint}");
    println!("Peer Id: {}", node_info.peer_id);
    println!("Logs dir: {}", node_info.log_path.to_string_lossy());
    println!("PID: {}", node_info.pid);
    println!("Binary version: {}", node_info.version);
    println!("Time since last restart: {:?}", node_info.uptime);

    Ok(())
}

pub async fn network_info(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    let network_info = client.network_info().await?;

    println!("Node's connections to the Network:");
    println!();
    println!("Connected peers:");
    for peer_id in network_info.connected_peers.iter() {
        println!("Peer: {peer_id}");
    }

    println!();
    println!("Node's listeners:");
    for multiaddr_str in network_info.listeners.iter() {
        println!("Listener: {multiaddr_str}");
    }

    Ok(())
}

pub async fn node_events(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let response = client
        .node_events(Request::new(NodeEventsRequest {}))
        .await?;

    println!("Listening to node events... (press Ctrl+C to exit)");

    let mut stream = response.into_inner();
    while let Some(Ok(e)) = stream.next().await {
        match NodeEvent::from_bytes(&e.event) {
            Ok(event) => println!("New event received: {event:?}"),
            Err(_) => {
                println!("Error while parsing received NodeEvent");
            }
        }
    }

    Ok(())
}

pub async fn transfers_events(
    addr: SocketAddr,
    sk: String,
    log_cash_notes: Option<PathBuf>,
    bootstrap_peers: Option<Vec<Multiaddr>>,
) -> Result<()> {
    let (client, mut wallet) = match MainPubkey::from_hex(&sk) {
        Ok(main_pubkey) => {
            let client =
                Client::new(SecretKey::random(), bootstrap_peers, true, None, None).await?;
            let wallet_dir = TempDir::new()?;
            let wallet = WatchOnlyWallet::load_from(&wallet_dir, main_pubkey)?;
            (client, wallet)
        }
        Err(err) => return Err(eyre!("Failed to parse hex-encoded PK: {err:?}")),
    };
    let endpoint = format!("https://{addr}");
    let mut node_client = SafeNodeClient::connect(endpoint).await?;
    let main_pk = wallet.address();
    let pk = main_pk.public_key();
    let _ = node_client
        .transfer_notifs_filter(Request::new(TransferNotifsFilterRequest {
            pk: pk.to_bytes().to_vec(),
        }))
        .await?;

    let _ = node_client
        .subscribe_to_topic(Request::new(GossipsubSubscribeRequest {
            topic: ROYALTY_TRANSFER_NOTIF_TOPIC.to_string(),
        }))
        .await?;

    let response = node_client
        .node_events(Request::new(NodeEventsRequest {}))
        .await?;

    println!("Listening to transfers notifications for {pk:?}... (press Ctrl+C to exit)");
    if let Some(ref path) = log_cash_notes {
        // create cash_notes dir
        fs::create_dir_all(path)?;
        println!("Writing cash notes to: {}", path.display());
    }
    println!();

    let mut stream = response.into_inner();
    while let Some(Ok(e)) = stream.next().await {
        let cash_notes = match NodeEvent::from_bytes(&e.event) {
            Ok(NodeEvent::TransferNotif {
                key,
                cashnote_redemptions,
            }) => {
                println!(
                    "New transfer notification received for {key:?}, containing {} CashNoteRedemption/s.",
                    cashnote_redemptions.len()
                );

                match client
                    .verify_cash_notes_redemptions(main_pk, &cashnote_redemptions)
                    .await
                {
                    Err(err) => {
                        println!(
                            "At least one of the CashNoteRedemptions received is invalid, dropping them: {err:?}"
                        );
                        continue;
                    }
                    Ok(cash_notes) => cash_notes,
                }
            }
            Ok(_) => continue,
            Err(_) => {
                println!("Error while parsing received NodeEvent");
                continue;
            }
        };

        wallet.deposit(&cash_notes)?;

        for cn in cash_notes {
            println!(
                "CashNote received with {:?}, value: {}",
                cn.unique_pubkey(),
                cn.value()?
            );

            if let Some(ref path) = log_cash_notes {
                // create cash_notes dir
                let unique_pubkey_name =
                    *SpendAddress::from_unique_pubkey(&cn.unique_pubkey()).xorname();
                let unique_pubkey_file_name =
                    format!("{}.cash_note", hex::encode(unique_pubkey_name));

                let cash_note_file_path = path.join(unique_pubkey_file_name);
                println!("Writing cash note to: {}", cash_note_file_path.display());

                let hex = cn.to_hex()?;
                fs::write(cash_note_file_path, &hex)?;
            }
        }
        println!(
            "New balance after depositing received CashNote/s: {}",
            wallet.balance()
        );
        println!();
    }

    Ok(())
}

pub async fn record_addresses(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    let record_addresses = client.record_addresses().await?;

    println!("Records held by the node:");
    for address in record_addresses.iter() {
        println!("Key: {:?}", address.key);
    }

    Ok(())
}

pub async fn gossipsub_subscribe(addr: SocketAddr, topic: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    client.gossipsub_subscribe(&topic).await?;
    println!("Node successfully received the request to subscribe to topic '{topic}'");
    Ok(())
}

pub async fn gossipsub_unsubscribe(addr: SocketAddr, topic: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    client.gossipsub_unsubscribe(&topic).await?;
    println!("Node successfully received the request to unsubscribe from topic '{topic}'");
    Ok(())
}

pub async fn gossipsub_publish(addr: SocketAddr, topic: String, msg: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    client.gossipsub_publish(&topic, &msg).await?;
    println!("Node successfully received the request to publish on topic '{topic}'");
    Ok(())
}

pub async fn node_restart(addr: SocketAddr, delay_millis: u64, retain_peer_id: bool) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    client.node_restart(delay_millis, retain_peer_id).await?;
    println!(
        "Node successfully received the request to restart in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

pub async fn node_stop(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    client.node_stop(delay_millis).await?;
    println!(
        "Node successfully received the request to stop in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

pub async fn node_update(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let client = RpcClient::new(&endpoint);
    client.node_update(delay_millis).await?;
    println!(
        "Node successfully received the request to try to update in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}
