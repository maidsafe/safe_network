#![allow(dead_code)]

pub use bytes::Bytes;
pub use client::Client;
/// (Re-export for convenience.)
pub use libp2p::{multiaddr, Multiaddr};

mod client_wallet;
mod secrets;
mod self_encryption;
mod wallet;

const VERIFY_STORE: bool = true;

mod client {
    use std::{
        collections::{BTreeMap, HashSet},
        time::Duration,
    };

    use bytes::Bytes;
    use libp2p::{
        identity::Keypair,
        kad::{Quorum, Record},
        Multiaddr, PeerId,
    };
    use self_encryption::{decrypt_full_set, DataMap, EncryptedChunk};
    use sn_client::{
        networking::{
            multiaddr_is_global, version::IDENTIFY_PROTOCOL_STR, GetRecordCfg, Network,
            NetworkBuilder, NetworkError, NetworkEvent, PutRecordCfg, CLOSE_GROUP_SIZE,
        },
        transfers::{HotWallet, MainPubkey, NanoTokens, PaymentQuote},
        StoragePaymentResult,
    };
    use sn_protocol::{
        storage::{
            try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, RecordHeader,
            RecordKind,
        },
        NetworkAddress,
    };
    use sn_transfers::{Payment, SpendReason, Transfer};
    use tokio::{
        sync::mpsc::Receiver,
        task::{JoinError, JoinSet},
        time::interval,
    };
    use xor_name::XorName;

    use crate::wallet::MemWallet;
    use crate::{
        client_wallet::SendSpendsError,
        self_encryption::{encrypt, DataMapLevel},
    };

    #[derive(Debug, thiserror::Error)]
    pub enum PutError {
        #[error("Self encryption error: {0:?}")]
        SelfEncryption(#[from] crate::self_encryption::Error),
        #[error("Error serializing data")]
        Serialization,
        #[error("General networking error: {0:?}")]
        Network(#[from] NetworkError),
        #[error("General wallet error: {0:?}")]
        Wallet(#[from] sn_transfers::WalletError),
    }

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
    pub enum GetError {
        #[error("General networking error: {0:?}")]
        Network(#[from] sn_client::networking::NetworkError),
        #[error("General protocol error: {0:?}")]
        Protocol(#[from] sn_client::protocol::Error),
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

        /// Creates a `Transfer` that can be received by the receiver.
        /// Once received, it will be turned into a `CashNote` that the receiver can spend.
        pub async fn send(
            &mut self,
            to: MainPubkey,
            amount_in_nano: NanoTokens,
            reason: Option<SpendReason>,
            wallet: &mut MemWallet,
        ) -> eyre::Result<Transfer> {
            let offline_transfer =
                wallet.create_offline_transfer(vec![(amount_in_nano, to)], reason)?;

            // return the first CashNote (assuming there is only one because we only sent to one recipient)
            let cash_note_for_recipient = match &offline_transfer.cash_notes_for_recipient[..] {
                [cash_note] => Ok(cash_note),
                [_multiple, ..] => Err(SendSpendsError::CouldNotSendMoney(
                    "Multiple CashNotes were returned from the transaction when only one was expected."
                        .into(),
                )),
                [] => Err(SendSpendsError::CouldNotSendMoney(
                    "No CashNotes were returned from the wallet.".into(),
                )),
            }?;

            let transfer = Transfer::transfer_from_cash_note(cash_note_for_recipient)?;

            self.send_spends(offline_transfer.all_spend_requests.iter())
                .await?;

            wallet.process_offline_transfer(offline_transfer.clone());

            for spend in &offline_transfer.all_spend_requests {
                wallet.add_pending_spend(spend.clone());
            }

            Ok(transfer)
        }

        async fn fetch_from_data_map(&self, data_map: &DataMap) -> Result<Bytes, GetError> {
            let mut encrypted_chunks = vec![];
            for info in data_map.infos() {
                let chunk = self.fetch_chunk(info.dst_hash).await?;
                let chunk = EncryptedChunk {
                    index: info.index,
                    content: chunk.value,
                };
                encrypted_chunks.push(chunk);
            }

            let data = decrypt_full_set(data_map, &encrypted_chunks).expect("TODO");

            Ok(data)
        }

        async fn fetch_from_data_map_chunk(
            &self,
            data_map_bytes: &Bytes,
        ) -> Result<Bytes, GetError> {
            let mut data_map_level: DataMapLevel =
                rmp_serde::from_slice(data_map_bytes).expect("TODO");

            loop {
                let data_map = match &data_map_level {
                    DataMapLevel::First(map) => map,
                    DataMapLevel::Additional(map) => map,
                };

                let data = self.fetch_from_data_map(data_map).await?;

                match &data_map_level {
                    DataMapLevel::First(_) => break Ok(data),
                    DataMapLevel::Additional(_) => {
                        data_map_level = rmp_serde::from_slice(&data).expect("TODO");
                        continue;
                    }
                };
            }
        }

        /// Fetch a file based on the DataMap XorName.
        pub async fn get(&self, addr: XorName) -> Result<Bytes, GetError> {
            let data_map_chunk = self.fetch_chunk(addr).await?;
            let data = self
                .fetch_from_data_map_chunk(data_map_chunk.value())
                .await?;

            Ok(data)
        }

        pub async fn fetch_chunk(&self, addr: XorName) -> Result<Chunk, GetError> {
            tracing::info!("Getting chunk: {addr:?}");
            let key = NetworkAddress::from_chunk_address(ChunkAddress::new(addr)).to_record_key();

            let get_cfg = GetRecordCfg {
                get_quorum: Quorum::One,
                retry_strategy: None,
                target_record: None,
                expected_holders: HashSet::new(),
            };
            let record = self.network.get_record_from_network(key, &get_cfg).await?;
            let header = RecordHeader::from_record(&record)?;
            if let RecordKind::Chunk = header.kind {
                let chunk: Chunk = try_deserialize_record(&record)?;
                Ok(chunk)
            } else {
                Err(NetworkError::RecordKindMismatch(RecordKind::Chunk).into())
            }
        }

        pub async fn put(
            &mut self,
            data: Bytes,
            wallet: &mut HotWallet,
        ) -> Result<XorName, PutError> {
            let (map, chunks) = encrypt(data)?;
            let map_xor_name = *map.address().xorname();

            let mut xor_names = vec![];
            xor_names.push(map_xor_name);
            for chunk in &chunks {
                xor_names.push(*chunk.name());
            }

            self.pay(xor_names.into_iter(), wallet)
                .await
                .expect("TODO: handle error");

            self.upload_chunk(map, wallet).await?;
            for chunk in chunks {
                self.upload_chunk(chunk, wallet).await?;
            }

            Ok(map_xor_name)
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
                    // TODO: retry, but where?
                    let cost = network
                        .get_store_costs_from_network(
                            NetworkAddress::from_chunk_address(ChunkAddress::new(content_addr)),
                            vec![],
                        )
                        .await
                        .map_err(PayError::CouldNotGetStoreCosts);

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

        async fn pay_for_records(
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

        /// Directly writes Chunks to the network in the form of immutable self encrypted chunks.
        async fn upload_chunk(&self, chunk: Chunk, wallet: &mut HotWallet) -> Result<(), PutError> {
            let xor_name = *chunk.name();
            let (payment, payee) = self.get_recent_payment_for_addr(&xor_name, wallet)?;

            self.store_chunk(chunk, payee, payment).await?;

            wallet.api().remove_payment_transaction(&xor_name);

            Ok(())
        }

        /// Store `Chunk` as a record. Protected method.
        async fn store_chunk(
            &self,
            chunk: Chunk,
            payee: PeerId,
            payment: Payment,
        ) -> Result<(), PutError> {
            tracing::debug!("Storing chunk: {chunk:?} to {payee:?}");

            let key = chunk.network_address().to_record_key();

            let record_kind = RecordKind::ChunkWithPayment;
            let record = Record {
                key: key.clone(),
                value: try_serialize_record(&(payment, chunk.clone()), record_kind)
                    .map_err(|_| PutError::Serialization)?
                    .to_vec(),
                publisher: None,
                expires: None,
            };

            let put_cfg = PutRecordCfg {
                put_quorum: Quorum::One,
                retry_strategy: None,
                use_put_record_to: Some(vec![payee]),
                verification: None,
            };
            Ok(self.network.put_record(record, &put_cfg).await?)
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
                    match event {
                        NetworkEvent::PeerAdded(_peer_id, peers_len) => {
                            tracing::trace!("Peer added: {peers_len} in routing table");

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
        use rand::Rng;
        use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
        use sn_transfers::get_faucet_data_dir;
        use tokio::time::sleep;

        use super::*;

        #[tokio::test]
        async fn test_client_new() {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .try_init();

            let mut client = Client::connect(&[]).await.unwrap();

            let mut data = vec![0u8; 1024 * 1024 * 20];
            rand::thread_rng().fill(&mut data[..]);
            let data = Bytes::from(data);

            let root_dir = get_faucet_data_dir();
            let mut funded_faucet =
                load_account_wallet_or_create_with_mnemonic(&root_dir, None).unwrap();

            let xor = client.put(data.clone(), &mut funded_faucet).await.unwrap();

            sleep(Duration::from_secs(2)).await;

            let chunk = client.fetch_chunk(xor).await.unwrap();
            assert_eq!(chunk.name(), &xor);

            let data_fetched = client.get(xor).await.unwrap();
            assert_eq!(data, data_fetched);
        }
    }
}
