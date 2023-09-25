// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    Client, ClientEvent, ClientEventsChannel, ClientEventsReceiver, ClientRegister, WalletClient,
};
use bls::{PublicKey, SecretKey, Signature};
use indicatif::ProgressBar;
use libp2p::{
    identity::Keypair,
    kad::{Quorum, Record},
    Multiaddr,
};
#[cfg(feature = "open-metrics")]
use prometheus_client::registry::Registry;
use sn_networking::{multiaddr_is_global, NetworkBuilder, NetworkEvent, CLOSE_GROUP_SIZE};
use sn_protocol::{
    error::Error as ProtocolError,
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, RecordHeader,
        RecordKind, RegisterAddress, SpendAddress,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::{MainPubkey, NanoTokens, SignedSpend, UniquePubkey};

use sn_registers::SignedRegister;
use sn_transfers::{transfers::SpendRequest, wallet::Transfer};
use std::time::Duration;
use tokio::{sync::OwnedSemaphorePermit, task::spawn};
use tracing::trace;
use xor_name::XorName;

// Maximum number of concurrency to be allowed at any time
// eg, concurrent uploads/downloads of chunks, managed by a semaphore
pub const DEFAULT_CLIENT_CONCURRENCY: usize = 5;

/// The timeout duration for the client to receive any response from the network.
const INACTIVITY_TIMEOUT: std::time::Duration = tokio::time::Duration::from_secs(30);

impl Client {
    /// Instantiate a new client.
    pub async fn new(
        signer: SecretKey,
        peers: Option<Vec<Multiaddr>>,
        req_response_timeout: Option<Duration>,
        custom_concurrency_limit: Option<usize>,
    ) -> Result<Self> {
        // If any of our contact peers has a global address, we'll assume we're in a global network.
        let local = match peers {
            Some(ref peers) => !peers.iter().any(multiaddr_is_global),
            None => true,
        };

        info!("Startup a client with peers {peers:?} and local {local:?} flag");
        info!("Starting Kad swarm in client mode...");

        let mut network_builder =
            NetworkBuilder::new(Keypair::generate_ed25519(), local, std::env::temp_dir());

        if let Some(request_timeout) = req_response_timeout {
            network_builder.request_timeout(request_timeout);
        }
        network_builder
            .concurrency_limit(custom_concurrency_limit.unwrap_or(DEFAULT_CLIENT_CONCURRENCY));

        #[cfg(feature = "open-metrics")]
        network_builder.metrics_registry(Registry::default());

        let (network, mut network_event_receiver, swarm_driver) = network_builder.build_client()?;
        info!("Client constructed network and swarm_driver");
        let events_channel = ClientEventsChannel::default();

        let client = Self {
            network: network.clone(),
            events_channel,
            signer,
            peers_added: 0,
            progress: Some(Self::setup_connection_progress()),
        };

        // subscribe to our events channel first, so we don't have intermittent
        // errors if it does not exist and we cannot send to it.
        // (eg, if PeerAdded happens faster than our events channel is created)
        let mut client_events_rx = client.events_channel();

        let _swarm_driver = spawn({
            trace!("Starting up client swarm_driver");
            swarm_driver.run()
        });

        // spawn task to dial to the given peers
        let network_clone = network.clone();
        let _handle = spawn(async move {
            if let Some(peers) = peers {
                for addr in peers {
                    trace!(%addr, "dialing initial peer");

                    if let Err(err) = network_clone.dial(addr.clone()).await {
                        tracing::error!(%addr, "Failed to dial: {err:?}");
                    };
                }
            }
        });

        // spawn task to wait for NetworkEvent and check for inactivity
        let mut client_clone = client.clone();
        let _event_handler = spawn(async move {
            loop {
                match tokio::time::timeout(INACTIVITY_TIMEOUT, network_event_receiver.recv()).await
                {
                    Ok(event) => {
                        let the_event = match event {
                            Some(the_event) => the_event,
                            None => {
                                error!("The `NetworkEvent` channel has been closed");
                                continue;
                            }
                        };
                        if let Err(err) = client_clone.handle_network_event(the_event) {
                            warn!("Error handling network event: {err}");
                        }
                    }
                    Err(_elapse_err) => {
                        debug!("Client inactivity... waiting for a network event");
                        if let Err(error) = client_clone
                            .events_channel
                            .broadcast(ClientEvent::InactiveClient(INACTIVITY_TIMEOUT))
                        {
                            error!("Error broadcasting inactive client event: {error}");
                        }
                    }
                }
            }
        });

        // loop to connect to the network
        let mut is_connected = false;
        loop {
            match client_events_rx.recv().await {
                Ok(ClientEvent::ConnectedToNetwork) => {
                    is_connected = true;
                    info!("Client connected to the Network {is_connected:?}.");
                    break;
                }
                Ok(ClientEvent::InactiveClient(timeout)) => {
                    if is_connected {
                        info!("The client was inactive for {timeout:?}.");
                    } else {
                        info!("The client still does not know enough network nodes.");
                    }

                    continue;
                }
                Ok(ClientEvent::GossipsubMsg { .. }) => {}
                Err(err) => {
                    error!("Unexpected error during client startup {err:?}");
                    println!("Unexpected error during client startup {err:?}");
                    return Err(err);
                }
            }
        }

        // The above loop breaks if `ConnectedToNetwork` is received, but we might need the
        // receiver to still be active for us to not get any error if any other event is sent
        let mut client_events_rx = client.events_channel();
        spawn(async move {
            loop {
                let _ = client_events_rx.recv().await;
            }
        });
        Ok(client)
    }

    /// Get the client's network concurrency permit
    ///
    /// This allows us to grab a permit early if we're dealing with large data (chunks)
    /// and want to hold off on loading more until other operations are complete.
    pub async fn get_network_concurrency_permit(&self) -> Result<OwnedSemaphorePermit> {
        if let Some(limiter) = self.network.concurrency_limiter() {
            Ok(limiter.acquire_owned().await?)
        } else {
            Err(Error::NoNetworkConcurrencyLimiterFound)
        }
    }

    /// Set up our initial progress bar for network connectivity
    fn setup_connection_progress() -> ProgressBar {
        // Network connection progress bar
        let progress = ProgressBar::new_spinner();
        progress.enable_steady_tick(Duration::from_millis(120));
        progress.set_message("Connecting to The SAFE Network...");
        let new_style = progress.style().tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈🔗");
        progress.set_style(new_style);

        progress.set_message("Connecting to The SAFE Network...");

        progress
    }

    fn handle_network_event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::PeerAdded(peer_id) => {
                self.peers_added += 1;
                debug!("PeerAdded: {peer_id}");

                // In case client running in non-local-discovery mode,
                // it may take some time to fill up the RT.
                // To avoid such delay may fail the query with RecordNotFound,
                // wait till certain amount of peers populated into RT
                if self.peers_added >= CLOSE_GROUP_SIZE {
                    if let Some(progress) = &self.progress {
                        progress.finish_with_message("Connected to the Network");
                        // Remove the progress bar
                        self.progress = None;
                    }

                    self.events_channel
                        .broadcast(ClientEvent::ConnectedToNetwork)?;
                } else {
                    debug!(
                        "{}/{} initial peers found.",
                        self.peers_added, CLOSE_GROUP_SIZE
                    );

                    if let Some(progress) = &self.progress {
                        progress.set_message(format!(
                            "{}/{} initial peers found.",
                            self.peers_added, CLOSE_GROUP_SIZE
                        ));
                    }
                }
            }
            NetworkEvent::GossipsubMsg { topic, msg } => {
                self.events_channel
                    .broadcast(ClientEvent::GossipsubMsg { topic, msg })?;
            }
            _other => {}
        }

        Ok(())
    }

    /// Get the client events channel.
    pub fn events_channel(&self) -> ClientEventsReceiver {
        self.events_channel.subscribe()
    }

    /// Sign the given data
    pub fn sign<T: AsRef<[u8]>>(&self, data: T) -> Signature {
        self.signer.sign(data)
    }

    /// Return the public key of the data signing key
    pub fn signer_pk(&self) -> PublicKey {
        self.signer.public_key()
    }

    /// Get a register from network
    pub async fn get_signed_register_from_network(
        &self,
        address: RegisterAddress,
    ) -> Result<SignedRegister> {
        let key = NetworkAddress::from_register_address(address).to_record_key();

        let record = self
            .network
            .get_record_from_network(key, None, false)
            .await
            .map_err(|_| ProtocolError::RegisterNotFound(Box::new(address)))?;
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(record.key.clone())
        );
        let header = RecordHeader::from_record(&record)
            .map_err(|_| ProtocolError::RegisterNotFound(Box::new(address)))?;

        if let RecordKind::Register = header.kind {
            let register = try_deserialize_record::<SignedRegister>(&record)
                .map_err(|_| ProtocolError::RegisterNotFound(Box::new(address)))?;
            Ok(register)
        } else {
            error!("RecordKind mismatch while trying to retrieve a signed register");
            Err(Error::Protocol(ProtocolError::RecordKindMismatch(
                RecordKind::Register,
            )))
        }
    }

    /// Retrieve a Register from the network.
    pub async fn get_register(&self, address: RegisterAddress) -> Result<ClientRegister> {
        info!("Retrieving a Register replica at {address}");
        ClientRegister::retrieve(self.clone(), address).await
    }

    /// Create a new Register on the Network.
    pub async fn create_register(
        &self,
        meta: XorName,
        wallet_client: &mut WalletClient,
        verify_store: bool,
    ) -> Result<ClientRegister> {
        info!("Instantiating a new Register replica with meta {meta:?}");
        ClientRegister::create_online(self.clone(), meta, wallet_client, verify_store).await
    }

    /// Store `Chunk` as a record.
    pub(super) async fn store_chunk(
        &self,
        chunk: Chunk,
        payment: Vec<Transfer>,
        verify_store: bool,
        optional_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<()> {
        info!("Store chunk: {:?}", chunk.address());
        let key = chunk.network_address().to_record_key();

        let record = Record {
            key,
            value: try_serialize_record(&(payment, chunk), RecordKind::ChunkWithPayment)?,
            publisher: None,
            expires: None,
        };

        let record_to_verify = if verify_store {
            Some(record.clone())
        } else {
            None
        };

        Ok(self
            .network
            .put_record(record, record_to_verify, optional_permit, Quorum::One)
            .await?)
    }

    /// Retrieve a `Chunk` from the kad network.
    pub async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Getting chunk: {address:?}");
        let key = NetworkAddress::from_chunk_address(address).to_record_key();
        let record = self
            .network
            .get_record_from_network(key, None, false)
            .await?;
        let header = RecordHeader::from_record(&record)?;
        if let RecordKind::Chunk = header.kind {
            let chunk: Chunk = try_deserialize_record(&record)?;
            Ok(chunk)
        } else {
            Err(ProtocolError::RecordKindMismatch(RecordKind::Chunk).into())
        }
    }

    /// Send a `SpendCashNote` request to the network
    pub(crate) async fn network_store_spend(
        &self,
        spend: SpendRequest,
        verify_store: bool,
    ) -> Result<()> {
        let unique_pubkey = *spend.signed_spend.unique_pubkey();
        let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);

        trace!("Sending spend {unique_pubkey:?} to the network via put_record, with addr of {cash_note_addr:?}");
        let key = NetworkAddress::from_cash_note_address(cash_note_addr).to_record_key();
        let record = Record {
            key,
            value: try_serialize_record(&[spend.signed_spend], RecordKind::Spend)?,
            publisher: None,
            expires: None,
        };

        let record_to_verify = if verify_store {
            Some(record.clone())
        } else {
            None
        };

        Ok(self
            .network
            .put_record(record, record_to_verify, None, Quorum::All)
            .await?)
    }

    /// Get a cash_note spend from network
    pub async fn get_spend_from_network(
        &self,
        unique_pubkey: &UniquePubkey,
    ) -> Result<SignedSpend> {
        let address = SpendAddress::from_unique_pubkey(unique_pubkey);
        let key = NetworkAddress::from_cash_note_address(address).to_record_key();

        trace!(
            "Getting spend {unique_pubkey:?} with record_key {:?}",
            PrettyPrintRecordKey::from(key.clone())
        );
        let record = self
            .network
            .get_record_from_network(key.clone(), None, true)
            .await
            .map_err(|err| {
                Error::CouldNotVerifyTransfer(format!(
                    "unique_pubkey {unique_pubkey:?} errored: {err:?}"
                ))
            })?;
        debug!(
            "For spend {unique_pubkey:?} got record from the network, {:?}",
            PrettyPrintRecordKey::from(record.key.clone())
        );

        let header = RecordHeader::from_record(&record).map_err(|err| {
            Error::CouldNotVerifyTransfer(format!(
                "Can't parse RecordHeader for the unique_pubkey {unique_pubkey:?} with error {err:?}"
            ))
        })?;

        if let RecordKind::Spend = header.kind {
            let mut deserialized_record = try_deserialize_record::<Vec<SignedSpend>>(&record)
                .map_err(|err| {
                    Error::CouldNotVerifyTransfer(format!(
                        "Can't deserialize record for the unique_pubkey {unique_pubkey:?} with error {err:?}"
                    ))
                })?;

            match deserialized_record.len() {
                0 => {
                    trace!("Found no spend for {address:?}");
                    Err(Error::CouldNotVerifyTransfer(format!(
                        "Fetched record shows no spend for cash_note {unique_pubkey:?}."
                    )))
                }
                1 => {
                    let signed_spend = deserialized_record.remove(0);
                    trace!("Spend get for address: {address:?} successful");
                    if unique_pubkey == signed_spend.unique_pubkey() {
                        match signed_spend.verify(signed_spend.spent_tx_hash()) {
                            Ok(_) => {
                                trace!(
                                    "Verified signed spend got from networkfor {unique_pubkey:?}"
                                );
                                Ok(signed_spend)
                            }
                            Err(err) => {
                                warn!("Invalid signed spend got from network for {unique_pubkey:?}: {err:?}.");
                                Err(Error::CouldNotVerifyTransfer(format!(
                                "Spend failed verifiation for the unique_pubkey {unique_pubkey:?} with error {err:?}")))
                            }
                        }
                    } else {
                        warn!("Signed spend ({:?}) got from network mismatched the expected one {unique_pubkey:?}.", signed_spend.unique_pubkey());
                        Err(Error::CouldNotVerifyTransfer(format!(
                                "Signed spend ({:?}) got from network mismatched the expected one {unique_pubkey:?}.", signed_spend.unique_pubkey())))
                    }
                }
                _ => {
                    // each one is 0 as it shifts remaining elements
                    let one = deserialized_record.remove(0);
                    let two = deserialized_record.remove(0);
                    error!("Found double spend for {address:?}");
                    Err(Error::CouldNotVerifyTransfer(format!(
                "Found double spend for the unique_pubkey {unique_pubkey:?} - {:?}: spend_one {:?} and spend_two {:?}",
                PrettyPrintRecordKey::from(key), one.derived_key_sig, two.derived_key_sig
            )))
                }
            }
        } else {
            error!("RecordKind mismatch while trying to retrieve a cash_note spend");
            Err(ProtocolError::RecordKindMismatch(RecordKind::Spend).into())
        }
    }

    /// Get the store cost at a given address
    pub async fn get_store_costs_at_address(
        &self,
        address: &NetworkAddress,
    ) -> Result<Vec<(MainPubkey, NanoTokens)>> {
        let tolerance = 1.5;
        trace!("Getting store cost at {address:?}, with tolerance of {tolerance} times the cost");

        // Get the store costs from the network and map each token to `tolerance` * the token itself
        let costs = self
            .network
            .get_store_costs_from_network(address.clone())
            .await?;
        let adjusted_costs: Vec<(MainPubkey, NanoTokens)> = costs
            .into_iter()
            .map(|(address, token)| {
                (
                    address,
                    NanoTokens::from((token.as_nano() as f64 * tolerance) as u64),
                )
            })
            .collect();

        Ok(adjusted_costs)
    }

    /// Subscribe to given gossipsub topic
    pub fn subscribe_to_topic(&self, topic_id: String) -> Result<()> {
        info!("Subscribing to topic id: {topic_id}");
        self.network.subscribe_to_topic(topic_id)?;
        Ok(())
    }

    /// Publish message on given topic
    pub fn publish_on_topic(&self, topic_id: String, msg: Vec<u8>) -> Result<()> {
        info!("Publishing msg on topic id: {topic_id}");
        self.network.publish_on_topic(topic_id, msg)?;
        Ok(())
    }
}
