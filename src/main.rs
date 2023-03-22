mod comms;
mod stableset;

use bytes::Bytes;
use net::connection::Connection;
use net::endpoint::Endpoint;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Receiver;

use crate::comms::{Comm, NetworkNode, NetworkMsg, MsgId};
use crate::stableset::{run_stable_set, StableSetMsg};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::{env, fs, net::SocketAddr};

const PEERS_CONFIG_FILE: &str = "peers.json";

mod net;

#[derive(Clone)]
struct MyComm {
    inner: Endpoint,
    connections: Arc<RwLock<BTreeMap<SocketAddr, Arc<Connection>>>>,
}
impl MyComm {
    fn new(endpoint: Endpoint) -> Self {
        Self {
            inner: endpoint,
            connections: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// This adds an already established connection to our map.
    ///
    /// Streams initiated by a peer will be passed through channels.
    async fn add_connection(&self, connection: Connection) -> (Receiver<quinn::RecvStream>, Receiver<(quinn::SendStream, quinn::RecvStream)>) {
        let addr = connection.remote_address();
        let connection = Arc::new(connection);
        self.connections.write().await.insert(addr, connection.clone());

        let rx_uni = accept_uni(self.clone(), connection.clone(), addr);
        let rx_bi = accept_bi(self.clone(), connection.clone(), addr);

        (rx_uni, rx_bi)
    }

    /// Create a new connection to a peer and add it to our map.
    async fn establish_connection(&self, addr: SocketAddr) {
        if self.connections.read().await.contains_key(&addr) {
            return;
        }

        // TODO: Handle unhappy path
        let connection = self.inner.connect(&addr).await.unwrap();
        self.connections.write().await.insert(addr, Arc::new(connection));
    }

    async fn send_bi(&self, addr: SocketAddr, data: Bytes) -> Option<Vec<u8>> {
        let connection = match self.connections.read().await.get(&addr) {
            Some(c) => Arc::clone(c),
            None => return None,
        };

        // TODO: Abort because connection is probably broken.
        let (mut send, recv) = connection.inner.open_bi().await.unwrap();

        // TODO: Handle unhappy paths.
        let _ = send.write_all(&data).await;
        let _ = send.finish().await;
        let data = recv.read_to_end(5000).await.unwrap();

        Some(data)
    }
}

fn accept_bi(comm: MyComm, connection: Arc<Connection>, addr: SocketAddr) -> Receiver<(quinn::SendStream, quinn::RecvStream)> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    tokio::spawn(async move {
        loop {
            match connection.accept_bi().await {
                Ok(streams) => { let _ = tx.send(streams).await; },
                // Any error we receive here is unrecoverable. E.g, the connection is closed or protocol error occurred.
                Err(_err) => {
                    comm.connections.write().await.remove(&addr);
                    break;
                }
            };
        }
    });

    rx
}
fn accept_uni(comm: MyComm, connection: Arc<Connection>, addr: SocketAddr) -> Receiver<quinn::RecvStream> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    tokio::spawn(async move {
        loop {
            match connection.accept_uni().await {
                Ok(stream) => { let _ = tx.send(stream).await; },
                // Any error we receive here is unrecoverable. E.g, the connection is closed or protocol error occurred.
                Err(_err) => {
                    comm.connections.write().await.remove(&addr);
                    break;
                }
            };
        }
    });

    rx
}

async fn start(addr: SocketAddr) -> MyComm {
    let endpoint = net::endpoint::Endpoint::builder()
        .addr(addr)
        .server()
        .unwrap();
    let endpoint_clone = endpoint.clone();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    // Task to process incoming QUIC connections.
    tokio::spawn(async move {
        while let Some(connecting) = endpoint_clone.accept().await {
            tracing::trace!("QUIC connection incoming");
            let tx = tx.clone();

            // Task that establishes QUIC connection.
            tokio::spawn(async move {
                let connection = match connecting.await {
                    Ok(c) => c,
                    Err(err) => {
                        tracing::error!("incoming connection erred: {err:?}");
                        return;
                    }
                };

                // Send the connection over the channel, ignoring if the receiver is dropped/closed.
                let _ = tx.send(Connection { inner: connection }).await;
            });
        }
    });

    let comm = MyComm::new(endpoint);
    let comm_clone = comm.clone();

    tokio::spawn(async move {
        while let Some(connection) = rx.recv().await {
            let (_rx_uni, mut rx_bi) = comm_clone.add_connection(connection).await;

            tokio::spawn(async move {
                while let Some((mut send, recv)) = rx_bi.recv().await {
                    let data = recv.read_to_end(5000).await.unwrap();
                    let msg = NetworkMsg::<StableSetMsg>::from_bytes(Bytes::from(data)).unwrap();

                    if msg.payload != StableSetMsg::Ping {
                        tracing::error!("received non ping message: {msg:?}");
                        continue;
                    }

                    tracing::info!("ponging this message: {msg:?}");

                    let msg = NetworkMsg::<StableSetMsg> {
                        id: MsgId::new(),
                        payload: StableSetMsg::Pong,
                    };

                    // TODO: Handle unhappy path.
                    let _ = send.write_all(&msg.to_bytes().unwrap()).await;
                    let _ = send.finish().await;
                }
            });
        }
    });

    comm
}


/// Read my addr from env var and peers addr from config file
fn get_config() -> (SocketAddr, BTreeSet<SocketAddr>) {
    let my_addr_str: String = env::var("NODE_ADDR").expect("Failed to read NODE_ADDR from env");
    let my_addr = my_addr_str.parse().expect("Unable to parse socket address");
    let peers_json =
        fs::read_to_string(PEERS_CONFIG_FILE).expect("Unable to read peers config file");
    let peers_ip_str: Vec<String> =
        serde_json::from_str(&peers_json).expect("Unable to parse peers config file");
    let peers_addr: BTreeSet<SocketAddr> = peers_ip_str
        .iter()
        .filter(|p| *p != &my_addr_str)
        .map(|p| p.parse().expect("Unable to parse socket address"))
        .collect();
    println!("Read Peers from config: {:?}", peers_addr);
    (my_addr, peers_addr)
}

// /// start node and no_return unless fatal error
// async fn start_node(my_addr: SocketAddr, peers_addrs: BTreeSet<SocketAddr>) {
//     println!("Starting comms for node {my_addr:?}");
//     let peers = peers_addrs
//         .into_iter()
//         .map(|p| NetworkNode { addr: p })
//         .collect();

//     let (sender, receiver) = Comm::new::<StableSetMsg>(my_addr).expect("Comms Failed");

//     println!("Run stable set with peers {peers:?}");
//     run_stable_set(sender, receiver, peers).await
// }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (addr, peers) = get_config();

    let comm = start(addr).await;

    for p in peers {
        tracing::trace!("establishing connection to `{p}`");
        comm.establish_connection(p).await;

        tracing::trace!("sending `Ping` to `{p}`");
        let msg = NetworkMsg::<StableSetMsg> {
            id: MsgId::new(),
            payload: StableSetMsg::Ping,
        };
        let abc = comm.send_bi(p, msg.to_bytes().unwrap()).await;


        if let Some(data) = abc {
            let data = NetworkMsg::<StableSetMsg>::from_bytes(Bytes::from(data)).unwrap();
            if data.payload == StableSetMsg::Pong {
                tracing::info!("successfully received `Pong` from {p}");
            } else {
                tracing::error!("expected `Pong` from {p} but received: {data:?}");
            }
        } else {
            tracing::error!("error while receiving response");
        }
    }
}
