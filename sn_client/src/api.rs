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

use sn_dbc::{DbcId, SignedSpend};
use sn_networking::{close_group_majority, multiaddr_is_global, NetworkEvent, SwarmDriver};
use sn_protocol::{
    messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response},
    storage::{Chunk, ChunkAddress, DbcAddress},
    NetworkAddress,
};
use sn_transfers::client_transfers::SpendRequest;

use bls::{PublicKey, SecretKey, Signature};
use futures::future::select_all;
use itertools::Itertools;
use libp2p::{kad::RecordKey, Multiaddr, PeerId};
use tokio::task::spawn;
use tracing::trace;
use xor_name::XorName;

impl Client {
    /// Instantiate a new client.
    pub async fn new(signer: SecretKey, peers: Option<Vec<(PeerId, Multiaddr)>>) -> Result<Self> {
        // If any of our contact peers has a global address, we'll assume we're in a global network.
        let local = !peers
            .clone()
            .unwrap_or(vec![])
            .iter()
            .any(|(_, multiaddr)| multiaddr_is_global(multiaddr));

        info!("Starting Kad swarm in client mode...");
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new_client(local)?;
        info!("Client constructed network and swarm_driver");
        let events_channel = ClientEventsChannel::default();
        let client = Self {
            network: network.clone(),
            events_channel,
            signer,
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
                            trace!("Client dialing network");
                            for (peer_id, addr) in peers {
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
                let event = match network_event_receiver.recv().await {
                    Some(event) => event,
                    None => {
                        error!("The `NetworkEvent` channel has been closed");
                        continue;
                    }
                };
                trace!("Client recevied a network event {event:?}");
                if let Err(err) = client_clone.handle_network_event(event) {
                    warn!("Error handling network event: {err}");
                }
            }
        });

        if let Ok(event) = client_events_rx.recv().await {
            match event {
                ClientEvent::ConnectedToNetwork => {
                    info!("Client connected to the Network.");
                    println!("Client successfully connected to the Network.");
                }
            }
        }

        Ok(client)
    }

    fn handle_network_event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            // Clients do not handle requests.
            NetworkEvent::RequestReceived { .. } => {}
            // We do not listen on sockets.
            NetworkEvent::NewListenAddr(_) => {}
            // We are not doing AutoNAT and don't care about our status.
            NetworkEvent::NatStatusChanged(_) => {}
            NetworkEvent::PeerAdded(peer_id) => {
                self.events_channel
                    .broadcast(ClientEvent::ConnectedToNetwork)?;
                debug!("PeerAdded: {peer_id}");
            }
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

    /// Return the publick key of the data signing key
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
    /// It returns a Rgister instance which can be used to apply operations offline,
    /// and publish them all to the network on a ad hoc basis.
    pub fn create_register_offline(&self, xorname: XorName, tag: u64) -> Result<RegisterOffline> {
        info!("Instantiating a new (offline) Register replica with name {xorname} and tag {tag}");
        RegisterOffline::create(self.clone(), xorname, tag)
    }

    /// Store `Chunk` to its close group.
    pub(super) async fn store_chunk(&self, chunk: Chunk) -> Result<()> {
        info!("Store chunk: {:?}", chunk.address());
        let request = Request::Cmd(Cmd::StoreChunk(chunk));
        let response = self.send_and_wait_till_first_rsp(request).await?;

        if matches!(response, Response::Cmd(CmdResponse::StoreChunk(Ok(())))) {
            return Ok(());
        }

        if let Response::Cmd(CmdResponse::StoreChunk(result)) = response {
            result?;
        };

        // If there were no store chunk errors, then we had unexpected responses.
        Err(Error::UnexpectedResponses)
    }

    /// Retrieve a `Chunk` from the kad network.
    pub(super) async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Getting chunk: {address:?}");
        let xorname = address.name();
        match self
            .network
            .get_provided_data(RecordKey::new(xorname))
            .await?
        {
            Ok(chunk_bytes) => Ok(Chunk::new(chunk_bytes.into())),
            Err(err) => {
                warn!("Local internal error when trying to query chunk {xorname:?}: {err:?}",);
                Err(err.into())
            }
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

    pub(crate) async fn send_and_wait_till_first_rsp(&self, request: Request) -> Result<Response> {
        let mut responses = self
            .network
            .client_send_to_closest(&request, false)
            .await?
            .into_iter()
            .map(|res| res.map_err(Error::Network))
            .collect_vec();
        // The responses will be just one OK response or a vector of error responses.
        // In case of error responses, only need to return one.
        if let Some(response) = responses.pop() {
            response
        } else {
            Err(Error::UnexpectedResponses)
        }
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
                (Ok(Response::Cmd(CmdResponse::Spend(Ok(())))), _, remaining_futures) => {
                    trace!("Spend Ok response got while requesting to spend {dbc_id:?}");
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

        Err(Error::CouldNotVerifyTransfer(format!(
            "Not enough close group nodes accepted the spend for {dbc_id:?}. Got {}, required: {}.",
            ok_responses,
            close_group_majority()
        )))
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
                    Ok(Response::Query(QueryResponse::GetDbcSpend(Ok(received_spend)))),
                    _,
                    remaining_futures,
                ) => {
                    if dbc_id == received_spend.dbc_id() {
                        match received_spend.verify(received_spend.spent_tx_hash()) {
                            Ok(_) => {
                                trace!("Verified signed spend got from network while getting Spend for {dbc_id:?}");
                                ok_responses.push(received_spend);
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
