// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    Client, ClientEvent, ClientEventsBroadcaster, ClientEventsReceiver, ClientRegister,
    WalletClient,
};
use bls::{PublicKey, SecretKey, Signature};
use libp2p::{
    identity::Keypair,
    kad::{Quorum, Record},
    Multiaddr, PeerId,
};
use rand::{thread_rng, Rng};
use sn_networking::{
    get_signed_spend_from_record, multiaddr_is_global,
    target_arch::{interval, spawn, timeout, Instant},
    GetRecordCfg, NetworkBuilder, NetworkError, NetworkEvent, PutRecordCfg, VerificationKind,
};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::ChunkProof,
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, RecordHeader,
        RecordKind, RegisterAddress, RetryStrategy, SpendAddress,
    },
    NetworkAddress, PrettyPrintRecordKey, CLOSE_GROUP_SIZE,
};
use sn_registers::{Permissions, SignedRegister};
use sn_transfers::{
    CashNote, CashNoteRedemption, MainPubkey, NanoTokens, Payment, SignedSpend, TransferError,
    GENESIS_SPEND_UNIQUE_KEY,
};
#[cfg(target_arch = "wasm32")]
use std::path::PathBuf;
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::Arc,
};
use tokio::time::Duration;
use tracing::trace;
use xor_name::XorName;

/// The maximum duration the client will wait for a connection to the network before timing out.
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// The timeout duration for the client to receive any response from the network.
const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);

// Init during compilation, instead of runtime error that should never happen
// Option<T>::expect will be stabilised as const in the future (https://github.com/rust-lang/rust/issues/67441)
const QUORUM_N_IS_2: NonZeroUsize = match NonZeroUsize::new(2) {
    Some(v) => v,
    None => panic!("2 is not zero"),
};

impl Client {
    /// A quick client with a random secret key and some peers.
    pub async fn quick_start(peers: Option<Vec<Multiaddr>>) -> Result<Self> {
        Self::new(SecretKey::random(), peers, None, None).await
    }

    /// Instantiate a new client.
    ///
    /// Optionally specify the duration for the connection timeout.
    ///
    /// Defaults to [`CONNECTION_TIMEOUT`].
    ///
    /// # Arguments
    /// * 'signer' - [SecretKey]
    /// * 'peers' - [Option]<[Vec]<[Multiaddr]>>
    /// * 'connection_timeout' - [Option]<[Duration]> : Specification for client connection timeout set via Optional
    /// * 'client_event_broadcaster' - [Option]<[ClientEventsBroadcaster]>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        signer: SecretKey,
        peers: Option<Vec<Multiaddr>>,
        connection_timeout: Option<Duration>,
        client_event_broadcaster: Option<ClientEventsBroadcaster>,
    ) -> Result<Self> {
        // If any of our contact peers has a global address, we'll assume we're in a global network.
        let local = match peers {
            Some(ref peers) => !peers.iter().any(multiaddr_is_global),
            None => true,
        };

        info!("Startup a client with peers {peers:?} and local {local:?} flag");
        info!("Starting Kad swarm in client mode...");

        #[cfg(target_arch = "wasm32")]
        let root_dir = PathBuf::from("dummy path, wasm32/browser environments will not use this");
        #[cfg(not(target_arch = "wasm32"))]
        let root_dir = std::env::temp_dir();
        trace!("Starting Kad swarm in client mode..{root_dir:?}.");

        let network_builder = NetworkBuilder::new(Keypair::generate_ed25519(), local, root_dir);

        let (network, mut network_event_receiver, swarm_driver) = network_builder.build_client()?;
        info!("Client constructed network and swarm_driver");

        // If the events broadcaster is not provided by the caller, then we create a new one.
        // This is not optional as we wait on certain events to connect to the network and return from this function.
        let events_broadcaster = client_event_broadcaster.unwrap_or_default();

        let client = Self {
            network: network.clone(),
            events_broadcaster,
            signer: Arc::new(signer),
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
            let mut peers_added: usize = 0;
            loop {
                match timeout(INACTIVITY_TIMEOUT, network_event_receiver.recv()).await {
                    Ok(event) => {
                        let the_event = match event {
                            Some(the_event) => the_event,
                            None => {
                                error!("The `NetworkEvent` channel has been closed");
                                continue;
                            }
                        };

                        let start = Instant::now();
                        let event_string = format!("{the_event:?}");
                        if let Err(err) =
                            client_clone.handle_network_event(the_event, &mut peers_added)
                        {
                            warn!("Error handling network event: {err}");
                        }
                        trace!(
                            "Handled network event in {:?}: {:?}",
                            start.elapsed(),
                            event_string
                        );
                    }
                    Err(_elapse_err) => {
                        debug!("Client inactivity... waiting for a network event");
                        client_clone
                            .events_broadcaster
                            .broadcast(ClientEvent::InactiveClient(INACTIVITY_TIMEOUT));
                    }
                }
            }
        });

        // loop to connect to the network
        let mut is_connected = false;
        let connection_timeout = connection_timeout.unwrap_or(CONNECTION_TIMEOUT);
        let mut unsupported_protocol_tracker: Option<(String, String)> = None;

        debug!("Client connection timeout: {connection_timeout:?}");
        let mut connection_timeout_interval = interval(connection_timeout);
        // first tick completes immediately
        connection_timeout_interval.tick().await;

        loop {
            tokio::select! {
            _ = connection_timeout_interval.tick() => {
                if !is_connected {
                    if let Some((our_protocol, their_protocols)) = unsupported_protocol_tracker {
                        error!("Timeout: Client could not connect to the network as it does not support the protocol");
                        break Err(Error::UnsupportedProtocol(our_protocol, their_protocols));
                    }
                    error!("Timeout: Client failed to connect to the network within {connection_timeout:?}");
                    break Err(Error::ConnectionTimeout(connection_timeout));
                }
            }
            event = client_events_rx.recv() => {
                match event {
                    // we do not error out directly as we might still connect if the other initial peers are from
                    // the correct network.
                    Ok(ClientEvent::PeerWithUnsupportedProtocol { our_protocol, their_protocol }) => {
                        warn!(%our_protocol, %their_protocol, "Client tried to connect to a peer with an unsupported protocol. Tracking the latest one");
                        unsupported_protocol_tracker = Some((our_protocol, their_protocol));
                    }
                    Ok(ClientEvent::ConnectedToNetwork) => {
                        is_connected = true;
                        info!("Client connected to the Network {is_connected:?}.");
                        break Ok(());
                    }
                    Ok(ClientEvent::InactiveClient(timeout)) => {
                        if is_connected {
                            info!("The client was inactive for {timeout:?}.");
                        } else {
                            info!("The client still does not know enough network nodes.");
                        }
                    }
                    Err(err) => {
                        error!("Unexpected error during client startup {err:?}");
                        println!("Unexpected error during client startup {err:?}");
                        break Err(err.into());
                    }
                    _ => {}
                }
            }}
        }?;

        Ok(client)
    }

    fn handle_network_event(&mut self, event: NetworkEvent, peers_added: &mut usize) -> Result<()> {
        match event {
            NetworkEvent::PeerAdded(peer_id, _connected_peer) => {
                debug!("PeerAdded: {peer_id}");
                *peers_added += 1;

                // notify the listeners that we are waiting on CLOSE_GROUP_SIZE peers before emitting ConnectedToNetwork
                self.events_broadcaster.broadcast(ClientEvent::PeerAdded {
                    max_peers_to_connect: CLOSE_GROUP_SIZE,
                });
                // In case client running in non-local mode,
                // it may take some time to fill up the RT.
                // To avoid such delay may fail the query with RecordNotFound,
                // wait till certain amount of peers populated into RT
                if *peers_added >= CLOSE_GROUP_SIZE {
                    self.events_broadcaster
                        .broadcast(ClientEvent::ConnectedToNetwork);
                } else {
                    debug!("{peers_added}/{CLOSE_GROUP_SIZE} initial peers found.",);
                }
            }
            NetworkEvent::PeerWithUnsupportedProtocol {
                our_protocol,
                their_protocol,
            } => {
                self.events_broadcaster
                    .broadcast(ClientEvent::PeerWithUnsupportedProtocol {
                        our_protocol,
                        their_protocol,
                    });
            }
            _other => {}
        }

        Ok(())
    }

    /// Get the client events channel.
    ///
    /// Return Type:
    ///
    /// [ClientEventsReceiver]
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error, ClientEvent};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Using client.events_channel() to publish messages
    /// let mut events_channel = client.events_channel();
    /// while let Ok(event) = events_channel.recv().await {
    /// // Handle the event
    ///  }
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn events_channel(&self) -> ClientEventsReceiver {
        self.events_broadcaster.subscribe()
    }

    /// Sign the given data.
    ///
    /// # Arguments
    /// * 'data' - bytes; i.e bytes of an sn_registers::Register instance
    ///
    /// Return Type:
    ///
    /// [Signature]
    ///
    pub fn sign<T: AsRef<[u8]>>(&self, data: T) -> Signature {
        self.signer.sign(data)
    }

    /// Return a reference to the signer secret key.
    ///
    /// Return Type:
    ///
    /// [SecretKey]
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let secret_key_reference = client.signer();
    /// # Ok(())
    /// # }
    /// ```
    pub fn signer(&self) -> &SecretKey {
        &self.signer
    }

    /// Return the public key of the data signing key.
    ///
    /// Return Type:
    ///
    /// [PublicKey]
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let public_key_reference = client.signer_pk();
    /// # Ok(())
    /// # }
    /// ```
    pub fn signer_pk(&self) -> PublicKey {
        self.signer.public_key()
    }

    /// Set the signing key for this client.
    ///
    /// # Arguments
    /// * 'sk' - [SecretKey]
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let mut client = Client::new(SecretKey::random(), None, None, None).await?;
    /// client.set_signer_key(SecretKey::random());
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_signer_key(&mut self, sk: SecretKey) {
        self.signer = Arc::new(sk);
    }

    /// Get a register from network
    ///
    /// # Arguments
    /// * 'address' - [RegisterAddress]
    ///
    /// Return Type:
    ///
    /// Result<[SignedRegister]>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// use xor_name::XorName;
    /// use sn_registers::RegisterAddress;
    /// // Set up a client
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Set up an address
    /// let mut rng = rand::thread_rng();
    /// let owner = SecretKey::random().public_key();
    /// let xorname = XorName::random(&mut rng);
    /// let address = RegisterAddress::new(xorname, owner);
    /// // Get a signed register
    /// let signed_register = client.get_signed_register_from_network(address);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_signed_register_from_network(
        &self,
        address: RegisterAddress,
    ) -> Result<SignedRegister> {
        let key = NetworkAddress::from_register_address(address).to_record_key();

        let maybe_records = self.network.get_register_record_from_network(key).await?;
        merge_register_records(address, &maybe_records)
    }

    /// Retrieve a Register from the network.
    ///
    /// # Arguments
    /// * 'address' - [RegisterAddress]
    ///
    /// Return Type:
    ///
    /// Result<[ClientRegister]>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// use xor_name::XorName;
    /// use sn_registers::RegisterAddress;
    /// // Set up a client
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Set up an address
    /// let mut rng = rand::thread_rng();
    /// let owner = SecretKey::random().public_key();
    /// let xorname = XorName::random(&mut rng);
    /// let address = RegisterAddress::new(xorname, owner);
    /// // Get the register
    /// let retrieved_register = client.get_register(address);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_register(&self, address: RegisterAddress) -> Result<ClientRegister> {
        info!("Retrieving a Register replica at {address}");
        ClientRegister::retrieve(self.clone(), address).await
    }

    /// Create a new Register on the Network.
    /// Tops up payments and retries if necessary and verification failed
    ///
    /// # Arguments
    /// * 'address' - [XorName]
    /// * 'wallet_client' - [WalletClient]
    /// * 'verify_store' - Boolean
    /// * 'perms' - [Permissions]
    ///
    /// Return Type:
    ///
    /// Result<([ClientRegister], [NanoTokens], [NanoTokens])>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, WalletClient, Error};
    /// use tempfile::TempDir;
    /// use bls::SecretKey;
    /// use sn_transfers::{MainSecretKey};
    /// use xor_name::XorName;
    /// use sn_registers::RegisterAddress;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// // Set up Client, Wallet, etc
    /// use sn_registers::Permissions;
    /// use sn_transfers::HotWallet;
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let tmp_path = TempDir::new()?.path().to_owned();
    /// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// // Set up an address
    /// let mut rng = rand::thread_rng();
    /// let owner = SecretKey::random().public_key();
    /// let xorname = XorName::random(&mut rng);
    /// let address = RegisterAddress::new(xorname, owner);
    /// // Example:
    ///     let (mut client_register, _storage_cost, _royalties_fees) = client
    ///         .create_and_pay_for_register(
    ///             xorname,
    ///             &mut wallet_client,
    ///             true,
    ///             Permissions::default(),
    ///         )
    ///         .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_and_pay_for_register(
        &self,
        address: XorName,
        wallet_client: &mut WalletClient,
        verify_store: bool,
        perms: Permissions,
    ) -> Result<(ClientRegister, NanoTokens, NanoTokens)> {
        info!("Instantiating a new Register replica with address {address:?}");
        let (reg, mut total_cost, mut total_royalties) = ClientRegister::create_online(
            self.clone(),
            address,
            wallet_client,
            false,
            perms.clone(),
        )
        .await?;

        debug!("{address:?} Created in theory");
        let reg_address = reg.address();
        if verify_store {
            debug!("We should verify stored at {address:?}");
            let mut stored = self.verify_register_stored(*reg_address).await.is_ok();

            while !stored {
                info!("Register not completely stored on the network yet. Retrying...");
                // this verify store call here ensures we get the record from Quorum::all
                let (reg, top_up_cost, royalties_top_up) = ClientRegister::create_online(
                    self.clone(),
                    address,
                    wallet_client,
                    true,
                    perms.clone(),
                )
                .await?;
                let reg_address = reg.address();

                total_cost = total_cost
                    .checked_add(top_up_cost)
                    .ok_or(Error::TotalPriceTooHigh)?;
                total_royalties = total_cost
                    .checked_add(royalties_top_up)
                    .ok_or(Error::Wallet(sn_transfers::WalletError::from(
                        sn_transfers::TransferError::ExcessiveNanoValue,
                    )))?;
                stored = self.verify_register_stored(*reg_address).await.is_ok();
            }
        }

        Ok((reg, total_cost, total_royalties))
    }

    /// Store `Chunk` as a record. Protected method.
    ///
    /// # Arguments
    /// * 'chunk' - [Chunk]
    /// * 'payee' - [PeerId]
    /// * 'payment' - [Payment]
    /// * 'verify_store' - Boolean
    /// * 'retry_strategy' - [Option]<[RetryStrategy]> : Uses Quick by default
    ///
    pub(super) async fn store_chunk(
        &self,
        chunk: Chunk,
        payee: PeerId,
        payment: Payment,
        verify_store: bool,
        retry_strategy: Option<RetryStrategy>,
    ) -> Result<()> {
        info!("Store chunk: {:?}", chunk.address());
        let key = chunk.network_address().to_record_key();
        let retry_strategy = Some(retry_strategy.unwrap_or(RetryStrategy::Quick));

        let record_kind = RecordKind::ChunkWithPayment;
        let record = Record {
            key: key.clone(),
            value: try_serialize_record(&(payment, chunk.clone()), record_kind)?.to_vec(),
            publisher: None,
            expires: None,
        };

        let verification = if verify_store {
            let verification_cfg = GetRecordCfg {
                get_quorum: Quorum::N(QUORUM_N_IS_2),
                retry_strategy,
                target_record: None, // Not used since we use ChunkProof
                expected_holders: Default::default(),
                is_register: false,
            };
            // The `ChunkWithPayment` is only used to send out via PutRecord.
            // The holders shall only hold the `Chunk` copies.
            // Hence the fetched copies shall only be a `Chunk`

            let stored_on_node = try_serialize_record(&chunk, RecordKind::Chunk)?.to_vec();
            let random_nonce = thread_rng().gen::<u64>();
            let expected_proof = ChunkProof::new(&stored_on_node, random_nonce);

            Some((
                VerificationKind::ChunkProof {
                    expected_proof,
                    nonce: random_nonce,
                },
                verification_cfg,
            ))
        } else {
            None
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::One,
            retry_strategy,
            use_put_record_to: Some(vec![payee]),
            verification,
        };
        Ok(self.network.put_record(record, &put_cfg).await?)
    }

    /// Get chunk from chunk address.
    ///
    /// # Arguments
    /// * 'address' - [ChunkAddress]
    /// * 'show_holders' - Boolean
    /// * 'retry_strategy' - [Option]<[RetryStrategy]> : Uses Quick by default
    ///
    /// Return Type:
    ///
    /// Result<[Chunk]>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// use xor_name::XorName;
    /// use sn_protocol::storage::ChunkAddress;
    /// // client
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // chunk address
    /// let mut rng = rand::thread_rng();
    /// let xorname = XorName::random(&mut rng);
    /// let chunk_address = ChunkAddress::new(xorname);
    /// // get chunk
    /// let chunk = client.get_chunk(chunk_address,true, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_chunk(
        &self,
        address: ChunkAddress,
        show_holders: bool,
        retry_strategy: Option<RetryStrategy>,
    ) -> Result<Chunk> {
        info!("Getting chunk: {address:?}");
        let key = NetworkAddress::from_chunk_address(address).to_record_key();

        let expected_holders = if show_holders {
            let result: HashSet<_> = self
                .network
                .get_closest_peers(&NetworkAddress::from_chunk_address(address), true)
                .await?
                .iter()
                .cloned()
                .collect();
            result
        } else {
            Default::default()
        };

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: Some(retry_strategy.unwrap_or(RetryStrategy::Quick)),
            target_record: None,
            expected_holders,
            is_register: false,
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

    /// Verify if a `Chunk` is stored by expected nodes on the network.
    /// Single local use. Marked Private.
    pub async fn verify_chunk_stored(&self, chunk: &Chunk) -> Result<()> {
        let address = chunk.network_address();
        info!("Verifying chunk: {address:?}");
        let random_nonce = thread_rng().gen::<u64>();
        let record_value = try_serialize_record(&chunk, RecordKind::Chunk)?;
        let expected_proof = ChunkProof::new(record_value.as_ref(), random_nonce);

        if let Err(err) = self
            .network
            .verify_chunk_existence(
                address.clone(),
                random_nonce,
                expected_proof,
                Quorum::N(QUORUM_N_IS_2),
                None,
            )
            .await
        {
            error!("Failed to verify the existence of chunk {address:?} with err {err:?}");
            return Err(err.into());
        }

        Ok(())
    }

    /// Verify if a `Register` is stored by expected nodes on the network.
    ///
    /// # Arguments
    /// * 'address' - [RegisterAddress]
    ///
    /// Return Type:
    ///
    /// Result<[SignedRegister]>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// use xor_name::XorName;
    /// use sn_registers::RegisterAddress;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// // Set up Client
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Set up an address
    /// let mut rng = rand::thread_rng();
    /// let owner = SecretKey::random().public_key();
    /// let xorname = XorName::random(&mut rng);
    /// let address = RegisterAddress::new(xorname, owner);
    /// // Verify address is stored
    /// let is_stored = client.verify_register_stored(address).await.is_ok();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn verify_register_stored(&self, address: RegisterAddress) -> Result<SignedRegister> {
        info!("Verifying register: {address:?}");
        self.get_signed_register_from_network(address).await
    }

    /// Quickly checks if a `Register` is stored by expected nodes on the network.
    ///
    /// To be used for initial register put checks eg, if we expect the data _not_
    /// to exist, we can use it and essentially use the RetryStrategy::Quick under the hood
    ///
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// use xor_name::XorName;
    /// use sn_registers::RegisterAddress;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// // Set up Client
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Set up an address
    /// let mut rng = rand::thread_rng();
    /// let owner = SecretKey::random().public_key();
    /// let xorname = XorName::random(&mut rng);
    /// let address = RegisterAddress::new(xorname, owner);
    /// // Verify address is stored
    /// let is_stored = client.verify_register_stored(address).await.is_ok();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn quickly_check_if_register_stored(
        &self,
        address: RegisterAddress,
    ) -> Result<SignedRegister> {
        info!("Quickly checking for existing register : {address:?}");
        self.get_signed_register_from_network(address).await
    }

    /// Send a `SpendCashNote` request to the network. Protected method.
    ///
    /// # Arguments
    /// * 'spend' - [SignedSpend]
    /// * 'verify_store' - Boolean
    ///
    pub(crate) async fn network_store_spend(
        &self,
        spend: SignedSpend,
        verify_store: bool,
    ) -> Result<()> {
        let unique_pubkey = *spend.unique_pubkey();
        let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);
        let network_address = NetworkAddress::from_spend_address(cash_note_addr);

        let key = network_address.to_record_key();

        let record_kind = RecordKind::Spend;
        let record = Record {
            key: key.clone(),
            value: try_serialize_record(&[spend], record_kind)?.to_vec(),
            publisher: None,
            expires: None,
        };

        let pretty_key = PrettyPrintRecordKey::from(&key);
        info!("Sending spend {unique_pubkey:?} to the network via put_record, with addr of {cash_note_addr:?} - {pretty_key:?}, size of {}",
            record.value.len());

        let (record_to_verify, expected_holders) = if verify_store {
            let expected_holders: HashSet<_> = self
                .network
                .get_closest_peers(&network_address, true)
                .await?
                .iter()
                .cloned()
                .collect();
            (Some(record.clone()), expected_holders)
        } else {
            (None, Default::default())
        };

        // When there is retry on Put side, no need to have a retry on Get
        let verification_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: None,
            target_record: record_to_verify,
            expected_holders,
            is_register: false,
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::Majority,
            retry_strategy: Some(RetryStrategy::Persistent),
            use_put_record_to: None,
            verification: Some((VerificationKind::Network, verification_cfg)),
        };
        Ok(self.network.put_record(record, &put_cfg).await?)
    }

    /// Get a spend from network.
    ///
    /// # Arguments
    /// * 'address' - [SpendAddress]
    ///
    /// Return Type:
    ///
    /// Result<[SignedSpend]>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// use xor_name::XorName;
    /// use sn_transfers::SpendAddress;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Create a SpendAddress
    /// let mut rng = rand::thread_rng();
    /// let xorname = XorName::random(&mut rng);
    /// let spend_address = SpendAddress::new(xorname);
    /// // Here we get the spend address
    /// let spend = client.get_spend_from_network(spend_address).await?;
    /// // Example: We can use the spend to get its unique public key:
    /// let unique_pubkey = spend.unique_pubkey();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_spend_from_network(&self, address: SpendAddress) -> Result<SignedSpend> {
        self.try_fetch_spend_from_network(
            address,
            GetRecordCfg {
                get_quorum: Quorum::Majority,
                retry_strategy: Some(RetryStrategy::Balanced),
                target_record: None,
                expected_holders: Default::default(),
                is_register: false,
            },
        )
        .await
    }

    /// Try to peek a spend by just fetching one copy of it.
    /// Useful to help decide whether a re-put is necessary, or a spend exists already
    /// (client side verification).
    pub async fn peek_a_spend(&self, address: SpendAddress) -> Result<SignedSpend> {
        self.try_fetch_spend_from_network(
            address,
            GetRecordCfg {
                get_quorum: Quorum::One,
                retry_strategy: None,
                target_record: None,
                expected_holders: Default::default(),
                is_register: false,
            },
        )
        .await
    }

    /// This is a similar funcation to `get_spend_from_network` to get a spend from network.
    /// Just using different `RetryStrategy` to improve the performance during crawling.
    pub async fn crawl_spend_from_network(&self, address: SpendAddress) -> Result<SignedSpend> {
        self.try_fetch_spend_from_network(
            address,
            GetRecordCfg {
                get_quorum: Quorum::Majority,
                retry_strategy: None,
                target_record: None,
                expected_holders: Default::default(),
                is_register: false,
            },
        )
        .await
    }

    /// Try to confirm the Genesis spend doesn't present in the network yet.
    /// It shall be quick, and any signle returned copy shall consider as error.
    pub async fn is_genesis_spend_present(&self) -> bool {
        let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_SPEND_UNIQUE_KEY);
        self.peek_a_spend(genesis_addr).await.is_ok()
    }

    async fn try_fetch_spend_from_network(
        &self,
        address: SpendAddress,
        get_cfg: GetRecordCfg,
    ) -> Result<SignedSpend> {
        let key = NetworkAddress::from_spend_address(address).to_record_key();

        info!(
            "Getting spend at {address:?} with record_key {:?}",
            PrettyPrintRecordKey::from(&key)
        );
        let record = self
            .network
            .get_record_from_network(key.clone(), &get_cfg)
            .await?;
        info!(
            "For spend at {address:?} got record from the network, {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );

        let signed_spend = get_signed_spend_from_record(&address, &record)?;

        // check addr
        let spend_addr = SpendAddress::from_unique_pubkey(signed_spend.unique_pubkey());
        if address != spend_addr {
            let s = format!("Spend got from the Network at {address:?} contains different spend address: {spend_addr:?}");
            warn!("{s}");
            return Err(Error::Transfer(TransferError::InvalidSpendValue(
                *signed_spend.unique_pubkey(),
            )));
        }

        // check spend
        match signed_spend.verify() {
            Ok(()) => {
                trace!("Verified signed spend got from network for {address:?}");
                Ok(signed_spend.clone())
            }
            Err(err) => {
                warn!("Invalid signed spend got from network for {address:?}: {err:?}.");
                Err(Error::CouldNotVerifyTransfer(format!(
                    "Verification failed for spent at {address:?} with error {err:?}"
                )))
            }
        }
    }

    /// This function is used to receive a Vector of CashNoteRedemptions and turn them back into spendable CashNotes.
    /// For this we need a network connection.
    /// Verify CashNoteRedemptions and rebuild spendable currency from them.
    /// Returns an `Error::InvalidTransfer` if any CashNoteRedemption is not valid
    /// Else returns a list of CashNotes that can be spent by the owner.
    ///
    /// # Arguments
    /// * 'main_pubkey' - [MainPubkey]
    /// * 'cashnote_redemptions' - [CashNoteRedemption]
    ///
    /// Return Type:
    ///
    /// Result<[Vec]<[CashNote]>>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, Error};
    /// use bls::SecretKey;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// use sn_transfers::{CashNote, CashNoteRedemption, MainPubkey};
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Create a main public key
    /// let pk = SecretKey::random().public_key();
    /// let main_pub_key = MainPubkey::new(pk);
    /// // Create a Cash Note Redemption Vector
    /// let cash_note = CashNote::from_hex("&hex").unwrap();
    /// let cashNoteRedemption = CashNoteRedemption::from_cash_note(&cash_note);
    /// let vector = vec![cashNoteRedemption.clone(), cashNoteRedemption.clone()];
    /// // Verify the cash note redemptions
    /// let cash_notes = client.verify_cash_notes_redemptions(main_pub_key,&vector);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn verify_cash_notes_redemptions(
        &self,
        main_pubkey: MainPubkey,
        cashnote_redemptions: &[CashNoteRedemption],
    ) -> Result<Vec<CashNote>> {
        let cash_notes = self
            .network
            .verify_cash_notes_redemptions(main_pubkey, cashnote_redemptions)
            .await?;
        Ok(cash_notes)
    }
}

fn get_register_from_record(record: &Record) -> Result<SignedRegister> {
    let header = RecordHeader::from_record(record)?;

    if let RecordKind::Register = header.kind {
        let register = try_deserialize_record::<SignedRegister>(record)?;
        Ok(register)
    } else {
        error!("RecordKind mismatch while trying to retrieve a signed register");
        Err(NetworkError::RecordKindMismatch(RecordKind::Register).into())
    }
}

/// if multiple register records where found for a given key, merge them into a single register
fn merge_register_records(
    address: RegisterAddress,
    map: &HashMap<XorName, Record>,
) -> Result<SignedRegister> {
    let key = NetworkAddress::from_register_address(address).to_record_key();
    let pretty_key = PrettyPrintRecordKey::from(&key);
    info!(
        "Got {} register records from the network for key: {pretty_key:?}",
        map.len()
    );
    let mut all_registers = vec![];
    for record in map.values() {
        match get_register_from_record(record) {
            Ok(r) => all_registers.push(r),
            Err(e) => {
                warn!("Ignoring invalid register record {pretty_key:?} with error {e:?}");
                continue;
            }
        }
    }

    // get the first valid register
    let one_valid_reg = if let Some(r) = all_registers.clone().iter().find(|r| r.verify().is_ok()) {
        r.clone()
    } else {
        error!("No valid register records found for {key:?}");
        return Err(Error::Protocol(ProtocolError::RegisterNotFound(Box::new(
            address,
        ))));
    };

    // merge it with the others if they are valid
    let register: SignedRegister = all_registers.into_iter().fold(one_valid_reg, |mut acc, r| {
        if acc.verified_merge(&r).is_err() {
            warn!("Skipping register that failed to merge. Entry found for {key:?}");
        }
        acc
    });

    Ok(register)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sn_registers::{Register, RegisterCrdt, RegisterOp};

    use super::*;

    fn write_atop(
        signed_reg: &mut SignedRegister,
        crdt_reg: &mut RegisterCrdt,
        entry: &[u8],
        owner: &SecretKey,
    ) -> eyre::Result<()> {
        let children: BTreeSet<_> = crdt_reg.read().iter().map(|(hash, _)| *hash).collect();

        let (_hash, address, crdt_op) = crdt_reg.write(entry.to_vec(), &children)?;

        let op = RegisterOp::new(address, crdt_op, owner);

        signed_reg.add_op(op)?;

        Ok(())
    }

    #[test]
    fn test_merge_register_records() -> eyre::Result<()> {
        let mut rng = rand::thread_rng();
        let meta = XorName::random(&mut rng);
        let owner_sk = SecretKey::random();
        let owner_pk = owner_sk.public_key();
        let address = RegisterAddress::new(meta, owner_pk);

        let base_register = Register::new(owner_pk, meta, Default::default());
        let signature = owner_sk.sign(base_register.bytes()?);

        // prepare registers
        let mut register_root = SignedRegister::new(base_register, signature, BTreeSet::new());
        let mut crdt_reg_root = RegisterCrdt::new(address);

        write_atop(
            &mut register_root,
            &mut crdt_reg_root,
            b"root_entry",
            &owner_sk,
        )?;

        let mut signed_register1 = register_root.clone();
        let mut crdt_reg1 = crdt_reg_root.clone();
        write_atop(&mut signed_register1, &mut crdt_reg1, b"entry1", &owner_sk)?;

        let mut signed_register2 = register_root.clone();
        let mut crdt_reg2 = crdt_reg_root.clone();
        write_atop(&mut signed_register2, &mut crdt_reg2, b"entry2", &owner_sk)?;

        let base_register_bad = Register::new(owner_pk, meta, Default::default());
        let bad_sk = SecretKey::random();
        let signature_bad = bad_sk.sign(base_register_bad.bytes()?);
        let signed_register_bad =
            SignedRegister::new(base_register_bad, signature_bad, BTreeSet::new());

        // prepare records
        let record1 = Record {
            key: NetworkAddress::from_register_address(address).to_record_key(),
            value: try_serialize_record(&signed_register1, RecordKind::Register)?.to_vec(),
            publisher: None,
            expires: None,
        };
        let xorname1 = XorName::from_content(&record1.value);
        let record2 = Record {
            key: NetworkAddress::from_register_address(address).to_record_key(),
            value: try_serialize_record(&signed_register2, RecordKind::Register)?.to_vec(),
            publisher: None,
            expires: None,
        };
        let xorname2 = XorName::from_content(&record2.value);
        let record_bad = Record {
            key: NetworkAddress::from_register_address(address).to_record_key(),
            value: try_serialize_record(&signed_register_bad, RecordKind::Register)?.to_vec(),
            publisher: None,
            expires: None,
        };
        let xorname_bad = XorName::from_content(&record_bad.value);

        // test with 2 valid records: should return the two merged
        let mut expected_merge = signed_register1.clone();
        expected_merge.merge(&signed_register2)?;
        let map = HashMap::from_iter(vec![(xorname1, record1.clone()), (xorname2, record2)]);
        let reg = merge_register_records(address, &map)?; // Ok
        assert_eq!(reg, expected_merge);

        // test with 1 valid record and 1 invalid record: should return the valid one
        let map = HashMap::from_iter(vec![(xorname1, record1), (xorname2, record_bad.clone())]);
        let reg = merge_register_records(address, &map)?; // Ok
        assert_eq!(reg, signed_register1);

        // test with 2 invalid records: should error out
        let map = HashMap::from_iter(vec![
            (xorname_bad, record_bad.clone()),
            (xorname_bad, record_bad),
        ]);
        let res = merge_register_records(address, &map); // Err
        assert!(res.is_err());

        Ok(())
    }
}
