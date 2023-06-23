// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::BTreeMap;

use super::{
    error::{Error, Result},
    Client, ClientEvent, ClientEventsChannel, ClientEventsReceiver, Register, RegisterOffline,
};

use bls::{PublicKey, SecretKey, Signature};
use futures::future::select_all;
use indicatif::ProgressBar;
use itertools::Itertools;
use libp2p::{
    kad::{RecordKey, K_VALUE},
    Multiaddr, PeerId,
};
use sn_dbc::{DbcId, SignedSpend};
use sn_networking::{close_group_majority, multiaddr_is_global, NetworkEvent, SwarmDriver};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{Cmd, CmdResponse, PaymentProof, Query, QueryResponse, Request, Response},
    storage::{
        try_deserialize_record, Chunk, ChunkAddress, ChunkWithPayment, DbcAddress, RecordHeader,
        RecordKind,
    },
    NetworkAddress,
};
use sn_transfers::client_transfers::SpendRequest;
use std::num::NonZeroUsize;
use std::time::Duration;
use tokio::task::spawn;
use tracing::trace;
use xor_name::XorName;

/// The timeout duration for the client to receive any response from the network.
const INACTIVITY_TIMEOUT: std::time::Duration = tokio::time::Duration::from_secs(10);

/// The initial rounds of `get_random` allowing client to fill up the RT.
const INITIAL_GET_RANDOM_ROUNDS: usize = 5;

impl Client {
    /// Instantiate a new client.
    pub async fn new(
        signer: SecretKey,
        peers: Option<Vec<(PeerId, Multiaddr)>>,
        req_response_timeout: Option<Duration>,
    ) -> Result<Self> {
        // If any of our contact peers has a global address, we'll assume we're in a global network.
        let local = !peers
            .clone()
            .unwrap_or(vec![])
            .iter()
            .any(|(_, multiaddr)| multiaddr_is_global(multiaddr));

        info!("Starting Kad swarm in client mode...");

        let (network, mut network_event_receiver, swarm_driver) =
            SwarmDriver::new_client(local, req_response_timeout)?;
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
        // errors if it does not exist and we cannot send to it
        // (eg, if PeerAdded happens faster than our events channel is created)
        let mut client_events_rx = client.events_channel();

        let mut must_dial_network = true;

        let mut client_clone = client.clone();

        let _swarm_driver = spawn({
            trace!("Starting up client swarm_driver");
            swarm_driver.run()
        });

        let _event_handler = spawn(async move {
            loop {
                if let Some(peers) = peers.clone() {
                    if must_dial_network {
                        let network = network.clone();
                        let _handle = spawn(async move {
                            for (peer_id, addr) in peers {
                                trace!("Client dialing network peer {peer_id} at {addr:?}");
                                let _ = network.add_to_routing_table(peer_id, addr.clone()).await;
                                if let Err(err) = network.dial(peer_id, addr.clone()).await {
                                    tracing::error!("Failed to dial {peer_id}: {err:?}");
                                };
                            }
                        });

                        must_dial_network = false;
                    }
                }

                info!("Client waiting for a network event");
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
                        trace!("Client received a network event {the_event:?}");
                        if let Err(err) = client_clone.handle_network_event(the_event) {
                            warn!("Error handling network event: {err}");
                        }
                    }
                    Err(_elapse_err) => {
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

        loop {
            let mut rng = rand::thread_rng();
            // Carry out 5 rounds of random get to fill up the RT at the beginning.
            for _ in 0..INITIAL_GET_RANDOM_ROUNDS {
                let random_target = ChunkAddress::new(XorName::random(&mut rng));
                let _ = client.get_chunk(random_target).await;
            }
            match client_events_rx.recv().await {
                Ok(ClientEvent::ConnectedToNetwork) => {
                    info!("Client connected to the Network.");
                    break;
                }
                Ok(ClientEvent::InactiveClient(timeout)) => {
                    let random_target = ChunkAddress::new(XorName::random(&mut rng));
                    debug!("No ClientEvent activity in the past {timeout:?}, performing a random get_chunk query to target: {random_target:?}");
                    println!("The client still does not know enough network nodes.");
                    let _ = client.get_chunk(random_target).await;
                    continue;
                }
                Err(err) => {
                    error!("Unexpected error during client startup {err:?}");
                    println!("Unexpected error during client startup {err:?}");
                    return Err(err);
                }
            }
        }

        Ok(client)
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
            // Clients do not handle requests.
            NetworkEvent::RequestReceived { .. } => {}
            // Clients do not handle responses
            NetworkEvent::ResponseReceived { .. } => {}
            // We do not listen on sockets.
            NetworkEvent::NewListenAddr(_) => {}
            // We are not doing AutoNAT and don't care about our status.
            NetworkEvent::NatStatusChanged(_) => {}
            NetworkEvent::PeerAdded(peer_id) => {
                debug!("PeerAdded: {peer_id}");
                // In case client running in non-local-discovery mode,
                // it may take some time to fill up the RT.
                // To avoid such delay may fail the query with RecordNotFound,
                // wait till certain amount of peers populated into RT
                if let Some(peers_added) = NonZeroUsize::new(self.peers_added) {
                    if peers_added >= K_VALUE {
                        if let Some(progress) = &self.progress {
                            progress.finish_with_message("Connected to the Network");
                            // Remove the progress bar
                            self.progress = None;
                        }

                        self.events_channel
                            .broadcast(ClientEvent::ConnectedToNetwork)?;
                    } else {
                        debug!("{}/{} initial peers found.", self.peers_added, K_VALUE);

                        if let Some(progress) = &self.progress {
                            progress.set_message(format!(
                                "{}/{} initial peers found.",
                                self.peers_added, K_VALUE
                            ));
                        }
                    }
                }
                self.peers_added += 1;
            }
            NetworkEvent::PeerRemoved(_) | NetworkEvent::LostRecordDetected(_) => {}
        }

        Ok(())
    }

    /// Get the client events channel.
    pub fn events_channel(&self) -> ClientEventsReceiver {
        self.events_channel.subscribe()
    }

    /// Sign the given data
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signer.sign(data)
    }

    /// Return the public key of the data signing key
    pub fn signer_pk(&self) -> PublicKey {
        self.signer.public_key()
    }

    /// Retrieve a Register from the network.
    pub async fn get_register(&self, xorname: XorName, tag: u64) -> Result<Register> {
        info!("Retrieving a Register replica with name {xorname} and tag {tag}");
        Register::retrieve(self.clone(), xorname, tag).await
    }

    /// Create a new Register.
    pub async fn create_register(&self, xorname: XorName, tag: u64) -> Result<Register> {
        info!("Instantiating a new Register replica with name {xorname} and tag {tag}");
        Register::create(self.clone(), xorname, tag).await
    }

    /// Create a new offline Register instance.
    /// It returns a Register instance which can be used to apply operations offline,
    /// and publish them all to the network on a ad hoc basis.
    pub fn create_register_offline(&self, xorname: XorName, tag: u64) -> Result<RegisterOffline> {
        info!("Instantiating a new (offline) Register replica with name {xorname} and tag {tag}");
        RegisterOffline::create(self.clone(), xorname, tag)
    }

    /// Store `Chunk` to spcified target.
    pub(super) async fn store_chunk(
        &self,
        chunk: Chunk,
        payment: Option<PaymentProof>,
        closest_peers: Vec<PeerId>,
    ) -> Result<()> {
        let address = *chunk.name();
        info!("Store chunk: {:?}", address);
        let request = Request::Cmd(Cmd::StoreChunk { chunk, payment });

        // Result will be: just one with `StoreChunk(Ok(_))` response;
        // or a vector of error responses, which only take the first into account.
        let mut responses = self
            .network
            .send_and_get_responses(closest_peers, &request, false)
            .await
            .into_iter()
            .map(|res| res.map_err(Error::Network))
            .collect_vec();
        let response = if let Some(response) = responses.pop() {
            response?
        } else {
            return Err(Error::UnexpectedResponses);
        };

        if matches!(response, Response::Cmd(CmdResponse::StoreChunk(Ok(_)))) {
            return Ok(());
        }

        if let Response::Cmd(CmdResponse::StoreChunk(result)) = response {
            result?;
        };

        // If there were no store chunk errors, then we had unexpected responses.
        Err(Error::UnexpectedResponses)
    }

    /// Return all the peers from the local network knowledge.
    pub(super) async fn get_all_local_peers(&self) -> Result<Vec<PeerId>> {
        Ok(self.network.get_all_local_peers().await?)
    }

    /// Returns the closest peers to the given `NetworkAddress`
    /// that is fetched from the local Routing Table.
    /// It is ordered by increasing distance of the peers.
    /// Note self peer_id is not included in the result.
    pub async fn get_closest_local_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        Ok(self.network.get_closest_local_peers(key).await?)
    }

    /// Retrieve a `Chunk` from the kad network.
    pub(super) async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Getting chunk: {address:?}");
        let xorname = address.name();
        let record = self
            .network
            .get_record_from_network(RecordKey::new(xorname))
            .await?;
        let header = RecordHeader::from_record(&record)?;
        if let RecordKind::Chunk = header.kind {
            let chunk_with_payment: ChunkWithPayment = try_deserialize_record(&record)?;
            Ok(chunk_with_payment.chunk)
        } else {
            Err(ProtocolError::RecordKindMismatch(RecordKind::Chunk).into())
        }
    }

    pub(crate) async fn send_to_closest(&self, request: Request) -> Result<Vec<Result<Response>>> {
        let responses = self
            .network
            .client_send_to_closest(&request, true)
            .await?
            .into_iter()
            .map(|res| res.map_err(Error::Network))
            .collect_vec();
        Ok(responses)
    }

    /// Send a `SpendDbc` request to the closest nodes to the dbc_id
    /// Makes sure at least majority of them successfully stored it
    pub(crate) async fn network_store_spend(&self, spend: SpendRequest) -> Result<()> {
        let dbc_id = *spend.signed_spend.dbc_id();
        let network_address = NetworkAddress::from_dbc_address(DbcAddress::from_dbc_id(&dbc_id));

        trace!("Getting the closest peers to the dbc_id {dbc_id:?} / {network_address:?}.");
        let closest_peers = self
            .network
            .client_get_closest_peers(&network_address)
            .await?;

        let cmd = Cmd::SpendDbc(spend.signed_spend);

        trace!(
            "Sending {:?} to the closest peers to store spend for {dbc_id:?}.",
            cmd
        );

        let mut list_of_futures = vec![];
        for peer in closest_peers {
            let request = Request::Cmd(cmd.clone());
            let future = Box::pin(self.network.send_request(request, peer));
            list_of_futures.push(future);
        }

        let mut ok_responses = 0;

        while !list_of_futures.is_empty() {
            match select_all(list_of_futures).await {
                (Ok(Response::Cmd(CmdResponse::Spend(Ok(spend_ok)))), _, remaining_futures) => {
                    trace!(
                        "Spend Ok {spend_ok:?} response got while requesting to spend {dbc_id:?}"
                    );
                    ok_responses += 1;

                    // Return once we got required number of expected responses.
                    if ok_responses >= close_group_majority() {
                        return Ok(());
                    }

                    list_of_futures = remaining_futures;
                }
                (Ok(other), _, remaining_futures) => {
                    trace!("Unexpected response got while requesting to spend {dbc_id:?}: {other}");
                    list_of_futures = remaining_futures;
                }
                (Err(err), _, remaining_futures) => {
                    trace!("Network error while requesting to spend {dbc_id:?}: {err:?}.");
                    list_of_futures = remaining_futures;
                }
            }
        }

        let err = Err(Error::CouldNotVerifyTransfer(format!(
            "Not enough close group nodes accepted the spend for {dbc_id:?}. Got {}, required: {}.",
            ok_responses,
            close_group_majority()
        )));

        warn!("Failed to store spend on the network: {:?}", err);
        err
    }

    pub(crate) async fn expect_closest_majority_same(&self, dbc_id: &DbcId) -> Result<SignedSpend> {
        let address = DbcAddress::from_dbc_id(dbc_id);
        let network_address = NetworkAddress::from_dbc_address(address);
        trace!("Getting the closest peers to {dbc_id:?} / {network_address:?}.");
        let closest_peers = self
            .network
            .client_get_closest_peers(&network_address)
            .await?;

        let query = Query::GetSpend(address);
        trace!("Sending {:?} to the closest peers.", query);

        let mut list_of_futures = vec![];
        for peer in closest_peers {
            let request = Request::Query(query.clone());
            let future = Box::pin(self.network.send_request(request, peer));
            list_of_futures.push(future);
        }

        let mut ok_responses = vec![];

        while !list_of_futures.is_empty() {
            match select_all(list_of_futures).await {
                (
                    Ok(Response::Query(QueryResponse::GetDbcSpend(Ok(signed_spend)))),
                    _,
                    remaining_futures,
                ) => {
                    if dbc_id == signed_spend.dbc_id() {
                        match signed_spend.verify(signed_spend.spent_tx_hash()) {
                            Ok(_) => {
                                trace!("Verified signed spend got from network while getting Spend for {dbc_id:?}");
                                ok_responses.push(signed_spend);
                            }
                            Err(err) => {
                                warn!("Invalid signed spend got from network while getting Spend for {dbc_id:?}: {err:?}.");
                            }
                        }
                    }

                    // Return once we got required number of expected responses.
                    if ok_responses.len() >= close_group_majority() {
                        use itertools::*;
                        let resp_count_by_spend: BTreeMap<SignedSpend, usize> = ok_responses
                            .clone()
                            .into_iter()
                            .map(|x| (x, 1))
                            .into_group_map()
                            .into_iter()
                            .map(|(spend, vec_of_ones)| (spend, vec_of_ones.len()))
                            .collect();

                        if resp_count_by_spend.len() > 1 {
                            return Err(Error::CouldNotVerifyTransfer(format!(
                                "Double spend detected while getting Spend for {dbc_id:?}: {:?}",
                                resp_count_by_spend.keys()
                            )));
                        }

                        let majority_agreement = resp_count_by_spend
                            .into_iter()
                            .max_by_key(|(_, count)| *count)
                            .map(|(k, _)| k);

                        if let Some(agreed_spend) = majority_agreement {
                            // Majority of nodes in the close group returned the same spend of the requested id.
                            // We return the spend, so that it can be compared to the spends we have in the DBC.
                            return Ok(agreed_spend);
                        }
                    }

                    list_of_futures = remaining_futures;
                }
                (Ok(other), _, remaining_futures) => {
                    trace!("Unexpected response while getting Spend for {dbc_id:?}: {other}.");
                    list_of_futures = remaining_futures;
                }
                (Err(err), _, remaining_futures) => {
                    trace!("Network error getting Spend for {dbc_id:?}: {err:?}.");
                    list_of_futures = remaining_futures;
                }
            }
        }

        Err(Error::CouldNotVerifyTransfer(format!(
            "Not enough close group nodes returned the requested spend. Got {}, required: {}.",
            ok_responses.len(),
            close_group_majority()
        )))
    }
}
