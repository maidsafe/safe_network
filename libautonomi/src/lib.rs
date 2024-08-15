pub use client::Client;
/// (Re-export for convenience.)
pub use libp2p::{multiaddr, Multiaddr};

mod secrets;
mod self_encryption;
mod wallet;

mod client {
    use std::{collections::HashSet, time::Duration};

    use bytes::Bytes;
    use libp2p::{identity::Keypair, Multiaddr};
    use sn_client::networking::{
        multiaddr_is_global, version::IDENTIFY_PROTOCOL_STR, Network, NetworkBuilder, NetworkEvent,
        CLOSE_GROUP_SIZE,
    };
    use sn_protocol::NetworkAddress;
    use tokio::{sync::mpsc::Receiver, time::interval};

    use crate::self_encryption::encrypt;

    #[derive(Debug, thiserror::Error)]
    #[error(transparent)]
    pub struct PutError(#[from] crate::self_encryption::Error);

    #[derive(Debug, thiserror::Error)]
    pub enum ConnectError {
        #[error("Could not connect to peers due to incompatible protocol: {0:?}")]
        TimedOutWithIncompatibleProtocol(HashSet<String>, String),
        #[error("Could not connect to enough peers in time.")]
        TimedOut,
    }

    #[derive(Clone)]
    pub struct Client {
        network: Network,
    }

    // /// Type of events broadcasted by the client to the public API.
    // #[derive(Clone, Debug)]
    // pub enum Event {
    //     /// A peer has been added to the Routing table.
    //     /// Also contains the max number of peers to connect to before we receive ClientEvent::ConnectedToNetwork
    //     PeerAdded { max_peers_to_connect: usize },
    //     /// We've encountered a Peer with an unsupported protocol.
    //     PeerWithUnsupportedProtocol {
    //         our_protocol: String,
    //         their_protocol: String,
    //     },
    //     /// The client has been connected to the network
    //     ConnectedToNetwork,
    //     /// No network activity has been received for a given duration
    //     /// we should error out
    //     InactiveClient(tokio::time::Duration),
    // }

    impl Client {
        /// ```no_run
        /// # use libautonomi::Client;
        /// let client = Client::connect(&["/ip4/127.0.0.1/udp/12000/quic".parse().unwrap()]);
        /// ```
        pub async fn connect(peers: &[Multiaddr]) -> Result<Self, ConnectError> {
            // Any global address makes the client non-local
            let local = !peers.iter().any(multiaddr_is_global);

            let (network, event_receiver) = build_client_and_run_swarm(local);

            // Spawn task to dial to the given peers
            let network_clone = network.clone();
            let peers = peers.to_vec();
            let _handle = tokio::spawn(async move {
                for addr in peers {
                    if let Err(err) = network_clone.dial(addr.clone()).await {
                        eprintln!("addr={addr} Failed to dial: {err:?}");
                    };
                }
            });

            let (sender, receiver) = tokio::sync::oneshot::channel();
            tokio::spawn(handle_event_receiver(event_receiver, sender));

            receiver.await.expect("sender should not close")?;

            Ok(Self { network })
        }

        pub async fn put(&self, data: Bytes) -> Result<(), PutError> {
            let (map, _chunks) = encrypt(data)?;

            let addr = NetworkAddress::from_chunk_address(*map.address());

            let cost = self
                .network
                .get_store_costs_from_network(addr, vec![])
                .await
                .expect("get store cost");

            tracing::info!("cost: {cost:?}");

            Ok(())
        }
    }

    fn build_client_and_run_swarm(local: bool) -> (Network, Receiver<NetworkEvent>) {
        // TODO: `root_dir` is only used for nodes. `NetworkBuilder` should not require it.
        let root_dir = std::env::temp_dir();
        let network_builder = NetworkBuilder::new(Keypair::generate_ed25519(), local, root_dir);

        // TODO: Re-export `Receiver<T>` from `sn_networking`. Else users need to keep their `tokio` dependency in sync.
        // TODO: Think about handling the mDNS error here.
        let (network, event_receiver, swarm_driver) =
            network_builder.build_client().expect("mdns to succeed");

        let _swarm_driver = tokio::spawn(swarm_driver.run());

        (network, event_receiver)
    }

    async fn handle_event_receiver(
        mut event_receiver: Receiver<NetworkEvent>,
        sender: tokio::sync::oneshot::Sender<Result<(), ConnectError>>,
    ) {
        // We switch this to `None` when we've sent the oneshot 'connect' result.
        let mut sender = Some(sender);
        let mut unsupported_protocols = vec![];

        let mut timeout_timer = interval(Duration::from_secs(20));
        timeout_timer.tick().await;

        loop {
            tokio::select! {
                _ = timeout_timer.tick() =>  {
                    if let Some(sender) = sender.take() {
                        if unsupported_protocols.len() > 1 {
                            let protocols: HashSet<String> =
                                unsupported_protocols.iter().cloned().collect();
                            sender
                                .send(Err(ConnectError::TimedOutWithIncompatibleProtocol(
                                    protocols,
                                    IDENTIFY_PROTOCOL_STR.to_string(),
                                )))
                                .expect("receiver should not close");
                        } else {
                            sender
                                .send(Err(ConnectError::TimedOut))
                                .expect("receiver should not close");
                        }
                    }
                }
                event = event_receiver.recv() => {
                    let event = event.expect("receiver should not close");
                    tracing::trace!("Handling event: {event:?}");
                    match event {
                        NetworkEvent::PeerAdded(_peer_id, peers_len) => {
                            tracing::debug!("Peer added: {peers_len} in routing table");

                            if peers_len >= CLOSE_GROUP_SIZE {
                                if let Some(sender) = sender.take() {
                                    sender.send(Ok(())).expect("receiver should not close");
                                }
                            }
                        }
                        NetworkEvent::PeerWithUnsupportedProtocol { their_protocol, .. } => {
                            tracing::warn!(their_protocol, "Peer with unsupported protocol");

                            if sender.is_some() {
                                unsupported_protocols.push(their_protocol);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // TODO: Handle closing of network events sender
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_client_new() {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .try_init();

            let peers: Vec<Multiaddr> = vec!["/ip4/127.0.0.1/udp/46910/quic-v1"]
                .into_iter()
                .map(|addr| addr.parse().expect("valid multiaddr"))
                .collect();

            let client = Client::connect(&peers).await.unwrap();

            client.put(b"Hello, world!".to_vec().into()).await.unwrap();

            // while let Some(event) = client.event_receiver.recv().await {
            //     println!("Received event: {event:?}");
            // }
        }
    }
}
