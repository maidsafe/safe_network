pub use client::Client;
/// (Re-export for convenience.)
pub use libp2p::{multiaddr, Multiaddr};

mod client_wallet;
mod secrets;
mod self_encryption;

const VERIFY_STORE: bool = true;

mod client {
    use std::{
        collections::{BTreeMap, HashSet},
        time::Duration,
    };

    use bytes::Bytes;
    use libp2p::{identity::Keypair, Multiaddr};
    use sn_client::{
        networking::{
            multiaddr_is_global, version::IDENTIFY_PROTOCOL_STR, Network, NetworkBuilder,
            NetworkEvent, CLOSE_GROUP_SIZE,
        },
        transfers::{HotWallet, MainPubkey, NanoTokens, PaymentQuote},
        StoragePaymentResult,
    };
    use sn_protocol::{storage::ChunkAddress, NetworkAddress};
    use tokio::{
        sync::mpsc::Receiver,
        task::{JoinError, JoinSet},
        time::interval,
    };
    use xor_name::XorName;

    use crate::{client_wallet::SendSpendsError, self_encryption::encrypt};

    #[derive(Debug, thiserror::Error)]
    #[error(transparent)]
    pub struct PutError(#[from] crate::self_encryption::Error);

    #[derive(Debug, thiserror::Error)]
    pub enum PayError {
        #[error("Could not get store costs: {0:?}")]
        CouldNotGetStoreCosts(sn_client::networking::NetworkError),
        #[error("Could not simultaneously fetch store costs: {0:?}")]
        JoinError(JoinError),
        #[error("Hot wallet error: {0:?}")]
        WalletError(#[from] sn_transfers::WalletError),
        #[error("Hot wallet error: {0:?}")]
        SendSpendsError(#[from] SendSpendsError),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum ConnectError {
        #[error("Could not connect to peers due to incompatible protocol: {0:?}")]
        TimedOutWithIncompatibleProtocol(HashSet<String>, String),
        #[error("Could not connect to enough peers in time.")]
        TimedOut,
    }

    #[derive(Clone)]
    pub struct Client {
        pub(crate) network: Network,
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

        pub async fn put(&mut self, data: Bytes, wallet: &mut HotWallet) -> Result<(), PutError> {
            let (map, chunks) = encrypt(data)?;

            let mut xor_names = vec![];
            xor_names.push(*map.address().xorname());
            for chunk in chunks {
                xor_names.push(*chunk.address().xorname());
            }

            self.pay(xor_names.into_iter(), wallet)
                .await
                .expect("TODO: handle error");

            Ok(())
        }

        async fn pay(
            &mut self,
            content_addrs: impl Iterator<Item = XorName>,
            wallet: &mut HotWallet,
        ) -> Result<StoragePaymentResult, PayError> {
            let mut tasks = JoinSet::new();
            for content_addr in content_addrs {
                let network = self.network.clone();
                tasks.spawn(async move {
                    let cost = network
                        .get_store_costs_from_network(
                            NetworkAddress::from_chunk_address(ChunkAddress::new(content_addr)),
                            vec![],
                        )
                        .await
                        .map_err(|error| PayError::CouldNotGetStoreCosts(error));

                    tracing::debug!("Storecosts retrieved for {content_addr:?} {cost:?}");
                    (content_addr, cost)
                });
            }
            tracing::debug!("Pending store cost tasks: {:?}", tasks.len());

            // collect store costs
            let mut cost_map = BTreeMap::default();
            let mut skipped_chunks = vec![];
            while let Some(res) = tasks.join_next().await {
                match res {
                    Ok((content_addr, Ok(cost))) => {
                        if cost.2.cost == NanoTokens::zero() {
                            skipped_chunks.push(content_addr);
                            tracing::debug!("Skipped existing chunk {content_addr:?}");
                        } else {
                            tracing::debug!(
                                "Storecost inserted into payment map for {content_addr:?}"
                            );
                            let _ =
                                cost_map.insert(content_addr, (cost.1, cost.2, cost.0.to_bytes()));
                        }
                    }
                    Ok((content_addr, Err(err))) => {
                        tracing::warn!(
                            "Cannot get store cost for {content_addr:?} with error {err:?}"
                        );
                        return Err(err);
                    }
                    Err(e) => {
                        return Err(PayError::JoinError(e));
                    }
                }
            }

            let (storage_cost, royalty_fees) = self.pay_for_records(&cost_map, wallet).await?;
            let res = StoragePaymentResult {
                storage_cost,
                royalty_fees,
                skipped_chunks,
            };
            Ok(res)
        }

        pub async fn pay_for_records(
            &mut self,
            cost_map: &BTreeMap<XorName, (MainPubkey, PaymentQuote, Vec<u8>)>,
            wallet: &mut HotWallet,
        ) -> Result<(NanoTokens, NanoTokens), PayError> {
            // Before wallet progress, there shall be no `unconfirmed_spend_requests`
            self.resend_pending_transactions(wallet).await;

            let total_cost = wallet.local_send_storage_payment(cost_map)?;

            tracing::trace!(
                "local_send_storage_payment of {} chunks completed",
                cost_map.len(),
            );

            // send to network
            tracing::trace!("Sending storage payment transfer to the network");
            let spend_attempt_result = self
                .send_spends(wallet.unconfirmed_spend_requests().iter())
                .await;

            tracing::trace!("send_spends of {} chunks completed", cost_map.len(),);

            // Here is bit risky that for the whole bunch of spends to the chunks' store_costs and royalty_fee
            // they will get re-paid again for ALL, if any one of the payment failed to be put.
            if let Err(error) = spend_attempt_result {
                tracing::warn!("The storage payment transfer was not successfully registered in the network: {error:?}. It will be retried later.");

                // if we have a DoubleSpend error, lets remove the CashNote from the wallet
                if let SendSpendsError::DoubleSpendAttemptedForCashNotes(spent_cash_notes) = &error
                {
                    for cash_note_key in spent_cash_notes {
                        tracing::warn!(
                            "Removing double spends CashNote from wallet: {cash_note_key:?}"
                        );
                        wallet.mark_notes_as_spent([cash_note_key]);
                        wallet.clear_specific_spend_request(*cash_note_key);
                    }
                }

                wallet.store_unconfirmed_spend_requests()?;

                return Err(PayError::SendSpendsError(error));
            } else {
                tracing::info!("Spend has completed: {:?}", spend_attempt_result);
                wallet.clear_confirmed_spend_requests();
            }
            tracing::trace!("clear up spends of {} chunks completed", cost_map.len(),);

            Ok(total_cost)
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

            let _client = Client::connect(&peers).await.unwrap();
            // client.put(b"Hello, world!".to_vec().into()).await.unwrap();
        }
    }
}
