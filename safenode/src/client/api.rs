// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    Client, ClientEvent, ClientEventsChannel, ClientEventsReceiver, Register, RegisterOffline,
};

use crate::{
    domain::{
        client_transfers::SpendRequest,
        storage::{dbc_address, dbc_name, Chunk, ChunkAddress},
    },
    network::{close_group_majority, NetworkEvent, SwarmDriver},
    protocol::{
        error::Error as ProtocolError,
        messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response, SpendQuery},
    },
};

use sn_dbc::{DbcId, SignedSpend};

use bls::{PublicKey, SecretKey, Signature};
use futures::future::select_all;
use itertools::Itertools;
use libp2p::{kad::RecordKey, Multiaddr, PeerId};
use tokio::task::spawn;
use tracing::trace;
use xor_name::XorName;

impl Client {
    /// Instantiate a new client.
    pub fn new(signer: SecretKey, peers: Option<Vec<(PeerId, Multiaddr)>>) -> Result<Self> {
        info!("Starting Kad swarm in client mode...");
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new_client()?;
        info!("Client constructed network and swarm_driver");
        let events_channel = ClientEventsChannel::default();
        let client = Self {
            network: network.clone(),
            events_channel,
            signer,
        };

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
                                if let Err(err) = network.clone().dial(peer_id, addr.clone()).await
                                {
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

        Ok(client)
    }

    fn handle_network_event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            // Clients do not handle requests.
            NetworkEvent::RequestReceived { .. } => {}
            // We do not listen on sockets.
            NetworkEvent::NewListenAddr(_) => {}
            NetworkEvent::PeerAdded => {
                self.events_channel
                    .broadcast(ClientEvent::ConnectedToNetwork);
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
        let responses = self.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::StoreChunk(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::StoreChunk(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no store chunk errors, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    /// Retrieve a `Chunk` from the kad network.
    pub(super) async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Getting chunk: {address:?}");
        match self
            .network
            .get_provided_data(RecordKey::new(address.name()))
            .await?
        {
            QueryResponse::GetChunk(result) => Ok(result?),
            other => {
                warn!(
                    "On querying chunk {:?} received unexpected response {other:?}",
                    address.name()
                );
                Err(Error::Protocol(ProtocolError::UnexpectedResponses))
            }
        }
    }

    /// This is for network testing only
    pub async fn get_closest(&self, dst: XorName) -> Vec<PeerId> {
        match self.network.client_get_closest_peers(dst).await {
            Ok(peers) => peers,
            Err(err) => {
                warn!("Failed to get_closest of {dst:?} with error {err:?}");
                vec![]
            }
        }
    }

    pub(crate) async fn send_to_closest(&self, request: Request) -> Result<Vec<Result<Response>>> {
        let responses = self
            .network
            .client_send_to_closest(&request)
            .await?
            .into_iter()
            .map(|res| res.map_err(Error::Network))
            .collect_vec();
        Ok(responses)
    }

    pub(crate) async fn expect_closest_majority_ok(&self, spend: SpendRequest) -> Result<()> {
        let dbc_id = spend.signed_spend.dbc_id();
        trace!("Getting the closest peers to {dbc_id:?}.");
        let closest_peers = self
            .network
            .client_get_closest_peers(dbc_name(dbc_id))
            .await?;

        let cmd = Cmd::SpendDbc {
            signed_spend: Box::new(spend.signed_spend),
            parent_tx: Box::new(spend.parent_tx),
            fee_ciphers: spend.fee_ciphers,
        };

        trace!("Sending {:?} to the closest peers.", cmd);

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
                    trace!("Spend Ok response got.");
                    ok_responses += 1;

                    // Return once we got required number of expected responses.
                    if ok_responses >= close_group_majority() {
                        return Ok(());
                    }

                    list_of_futures = remaining_futures;
                }
                (Ok(other), _, remaining_futures) => {
                    trace!("Unexpected response got: {other}.");
                    list_of_futures = remaining_futures;
                }
                (Err(err), _, remaining_futures) => {
                    trace!("Network error: {err:?}.");
                    list_of_futures = remaining_futures;
                }
            }
        }

        Err(Error::CouldNotVerifyTransfer(format!(
            "Not enough close group nodes accepted the spend. Got {}, required: {}.",
            ok_responses,
            close_group_majority()
        )))
    }

    pub(crate) async fn expect_closest_majority_same(&self, dbc_id: &DbcId) -> Result<SignedSpend> {
        trace!("Getting the closest peers to {dbc_id:?}.");
        let address = dbc_address(dbc_id);
        let closest_peers = self
            .network
            .client_get_closest_peers(*address.name())
            .await?;

        let query = Query::Spend(SpendQuery::GetDbcSpend(address));
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
                        trace!("Signed spend got from network.");
                        ok_responses.push(received_spend);
                    }

                    // Return once we got required number of expected responses.
                    if ok_responses.len() >= close_group_majority() {
                        use itertools::*;
                        let majority_agreement = ok_responses
                            .clone()
                            .into_iter()
                            .map(|x| (x, 1))
                            .into_group_map()
                            .into_iter()
                            .filter(|(_, v)| v.len() >= close_group_majority())
                            .max_by_key(|(_, v)| v.len())
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
                    trace!("Unexpected response got: {other}.");
                    list_of_futures = remaining_futures;
                }
                (Err(err), _, remaining_futures) => {
                    trace!("Network error: {err:?}.");
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
