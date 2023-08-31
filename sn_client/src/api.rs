// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    Client, ClientEvent, ClientEventsChannel, ClientEventsReceiver, ClientRegister,
};

use bls::{PublicKey, SecretKey, Signature};
use indicatif::ProgressBar;
use libp2p::{kad::Record, Multiaddr};
use sn_dbc::{Dbc, DbcId, PublicAddress, SignedSpend, Token};
use sn_networking::{multiaddr_is_global, NetworkEvent, SwarmDriver, CLOSE_GROUP_SIZE};
use sn_protocol::{
    error::Error as ProtocolError,
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, ChunkWithPayment,
        DbcAddress, RecordHeader, RecordKind, RegisterAddress,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;
use sn_transfers::client_transfers::SpendRequest;
use std::{sync::Arc, time::Duration};
use tokio::{sync::Semaphore, task::spawn};
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

        let (network, mut network_event_receiver, swarm_driver) =
            SwarmDriver::new_client(local, req_response_timeout)?;
        info!("Client constructed network and swarm_driver");
        let events_channel = ClientEventsChannel::default();

        // use passed concurrency limit or default
        let concurrency_limit = custom_concurrency_limit.unwrap_or(DEFAULT_CLIENT_CONCURRENCY);
        let concurrency_limiter = Arc::new(Semaphore::new(concurrency_limit));

        let client = Self {
            network: network.clone(),
            events_channel,
            signer,
            peers_added: 0,
            progress: Some(Self::setup_connection_progress()),
            concurrency_limiter,
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

    /// Get the client's concurrency limiter
    pub fn concurrency_limiter(&self) -> Arc<Semaphore> {
        self.concurrency_limiter.clone()
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
        if let NetworkEvent::PeerAdded(peer_id) = event {
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
        verify_store: bool,
    ) -> Result<ClientRegister> {
        info!("Instantiating a new Register replica with meta {meta:?}");
        ClientRegister::create_online(self.clone(), meta, verify_store).await
    }

    /// Store `Chunk` as a record.
    pub(super) async fn store_chunk(
        &self,
        chunk: Chunk,
        payment: Vec<Dbc>,
        verify_store: bool,
    ) -> Result<()> {
        info!("Store chunk: {:?}", chunk.address());
        let key = chunk.network_address().to_record_key();
        let chunk_with_payment = ChunkWithPayment { chunk, payment };

        let record = Record {
            key,
            value: try_serialize_record(&chunk_with_payment, RecordKind::Chunk)?,
            publisher: None,
            expires: None,
        };

        Ok(self.network.put_record(record, verify_store).await?)
    }

    /// Retrieve a `Chunk` from the kad network.
    pub(super) async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Getting chunk: {address:?}");
        let key = NetworkAddress::from_chunk_address(address).to_record_key();
        let record = self
            .network
            .get_record_from_network(key, None, false)
            .await?;
        let header = RecordHeader::from_record(&record)?;
        if let RecordKind::Chunk = header.kind {
            let chunk_with_payment: ChunkWithPayment = try_deserialize_record(&record)?;
            Ok(chunk_with_payment.chunk)
        } else {
            Err(ProtocolError::RecordKindMismatch(RecordKind::Chunk).into())
        }
    }

    /// Send a `SpendDbc` request to the network
    pub(crate) async fn network_store_spend(
        &self,
        spend: SpendRequest,
        verify_store: bool,
    ) -> Result<()> {
        let dbc_id = *spend.signed_spend.dbc_id();
        let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);

        trace!("Sending spend {dbc_id:?} to the network via put_record, with addr of {dbc_addr:?}");
        let key = NetworkAddress::from_dbc_address(dbc_addr).to_record_key();
        let record = Record {
            key,
            value: try_serialize_record(&[spend.signed_spend], RecordKind::DbcSpend)?,
            publisher: None,
            expires: None,
        };
        Ok(self.network.put_record(record, verify_store).await?)
    }

    /// Get a dbc spend from network
    pub async fn get_spend_from_network(&self, dbc_id: &DbcId) -> Result<SignedSpend> {
        let address = DbcAddress::from_dbc_id(dbc_id);
        let key = NetworkAddress::from_dbc_address(address).to_record_key();

        trace!(
            "Getting spend {dbc_id:?} with record_key {:?}",
            PrettyPrintRecordKey::from(key.clone())
        );
        let record = self
            .network
            .get_record_from_network(key.clone(), None, true)
            .await
            .map_err(|err| {
                Error::CouldNotVerifyTransfer(format!("dbc_id {dbc_id:?} errored: {err:?}"))
            })?;
        debug!(
            "For spend {dbc_id:?} got record from the network, {:?}",
            PrettyPrintRecordKey::from(record.key.clone())
        );

        let header = RecordHeader::from_record(&record).map_err(|err| {
            Error::CouldNotVerifyTransfer(format!(
                "Can't parse RecordHeader for the dbc_id {dbc_id:?} with error {err:?}"
            ))
        })?;

        if let RecordKind::DbcSpend = header.kind {
            let mut deserialized_record = try_deserialize_record::<Vec<SignedSpend>>(&record)
                .map_err(|err| {
                    Error::CouldNotVerifyTransfer(format!(
                        "Can't deserialize record for the dbc_id {dbc_id:?} with error {err:?}"
                    ))
                })?;

            match deserialized_record.len() {
                0 => {
                    trace!("Found no spend for {address:?}");
                    Err(Error::CouldNotVerifyTransfer(format!(
                        "Fetched record shows no spend for dbc {dbc_id:?}."
                    )))
                }
                1 => {
                    let signed_spend = deserialized_record.remove(0);
                    trace!("Spend get for address: {address:?} successful");
                    if dbc_id == signed_spend.dbc_id() {
                        match signed_spend.verify(signed_spend.spent_tx_hash()) {
                            Ok(_) => {
                                trace!("Verified signed spend got from networkfor {dbc_id:?}");
                                Ok(signed_spend)
                            }
                            Err(err) => {
                                warn!("Invalid signed spend got from network for {dbc_id:?}: {err:?}.");
                                Err(Error::CouldNotVerifyTransfer(format!(
                                "Spend failed verifiation for the dbc_id {dbc_id:?} with error {err:?}")))
                            }
                        }
                    } else {
                        warn!("Signed spend ({:?}) got from network mismatched the expected one {dbc_id:?}.", signed_spend.dbc_id());
                        Err(Error::CouldNotVerifyTransfer(format!(
                                "Signed spend ({:?}) got from network mismatched the expected one {dbc_id:?}.", signed_spend.dbc_id())))
                    }
                }
                _ => {
                    // each one is 0 as it shifts remaining elements
                    let one = deserialized_record.remove(0);
                    let two = deserialized_record.remove(0);
                    error!("Found double spend for {address:?}");
                    Err(Error::CouldNotVerifyTransfer(format!(
                "Found double spend for the dbc_id {dbc_id:?} - {:?}: spend_one {:?} and spend_two {:?}",
                PrettyPrintRecordKey::from(key), one.derived_key_sig, two.derived_key_sig
            )))
                }
            }
        } else {
            error!("RecordKind mismatch while trying to retrieve a dbc spend");
            Err(ProtocolError::RecordKindMismatch(RecordKind::DbcSpend).into())
        }
    }

    /// Get the store cost at a given address
    pub async fn get_store_costs_at_address(
        &self,
        address: &NetworkAddress,
    ) -> Result<Vec<(PublicAddress, Token)>> {
        trace!("Getting store cost at {address:?}");

        Ok(self
            .network
            .get_store_costs_from_network(address.clone())
            .await?)
    }
}
