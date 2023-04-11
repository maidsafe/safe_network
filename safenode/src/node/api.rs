// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Node, NodeEvent};

use crate::{
    network::{NetworkEvent, SwarmDriver, CLOSE_GROUP_SIZE},
    protocol::{
        messages::{Cmd, CmdResponse, Event, Query, QueryResponse, Request, Response},
        types::{
            address::{dbc_address, dbc_name, DbcAddress},
            error::Error as ProtocolError,
            register::User,
        },
    },
    storage::DataStorage,
};

use sn_dbc::{SignedSpend, TransactionVerifier};

use futures::future::select_all;
use libp2p::{request_response::ResponseChannel, PeerId};
use std::{
    collections::{BTreeSet, HashSet},
    time::Duration,
};
use tokio::task::spawn;

impl Node {
    /// Write to storage.
    pub async fn write(&self, cmd: &Cmd) -> CmdResponse {
        info!("Write: {cmd:?}");
        self.storage.write(cmd).await
    }

    /// Read from storage.
    pub async fn read(&self, query: &Query) -> QueryResponse {
        self.storage.read(query, User::Anyone).await
    }

    /// Asynchronously runs a new node instance, setting up the swarm driver,
    /// creating a data storage, and handling network events. Returns the
    /// created node and a `NodeEventsChannel` for listening to node-related
    /// events.
    ///
    /// # Returns
    ///
    /// A tuple containing a `Node` instance and a `NodeEventsChannel`.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem initializing the `SwarmDriver`.
    pub async fn run() -> Result<(Self, NodeEventsChannel)> {
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new()?;
        let storage = DataStorage::new();
        let node_events_channel = NodeEventsChannel::default();
        let node = Self {
            network,
            storage,
            events_channel: node_events_channel.clone(),
        };
        let mut node_clone = node.clone();

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            loop {
                let event = match network_event_receiver.recv().await {
                    Some(event) => event,
                    None => {
                        error!("The `NetworkEvent` channel has been closed");
                        continue;
                    }
                };
                if let Err(err) = node_clone.handle_network_event(event).await {
                    warn!("Error handling network event: {err}");
                }
            }
        });

        Ok((node, node_events_channel))
    }

    async fn handle_network_event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await?
            }
            NetworkEvent::PeerAdded => {
                self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
            }
        }

        Ok(())
    }

    async fn handle_request(
        &mut self,
        request: Request,
        response_channel: ResponseChannel<Response>,
    ) -> Result<()> {
        trace!("Handling request: {request:?}");
        let request_dst = request.dst();
        match request {
            Request::Cmd(Cmd::Dbc(signed_spend)) => {
                self.add_if_valid(signed_spend, Some(response_channel))
                    .await?
            }
            Request::Cmd(cmd) => {
                let resp = self.storage.write(&cmd).await;
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
            Request::Query(query) => {
                let resp = self.storage.read(&query, User::Anyone).await;
                self.send_response(Response::Query(resp), response_channel)
                    .await;
            }
            Request::Event(event) => {
                match event {
                    Event::DoubleSpendAttempted(a_spend, b_spend) => {
                        let a_addr = dbc_address(a_spend.dbc_id());
                        let b_addr = dbc_address(b_spend.dbc_id());

                        if a_addr == b_addr {
                            // We carelessly add the two spends here as they will automatically be
                            // marked as double spend attempt.
                            self.try_add_double_if_different_hash(&a_spend, &b_spend)
                                .await?;
                        } else {
                            // We have two of different addresses on the incoming.
                            // If we have a valid spend with different hash, then we
                            // will add that pair to the unspendable list, then we notify
                            // the group of the correctly identified double spend attempt.
                            let existing_a = self.storage.contains_valid(a_spend.dbc_id()).await;
                            let existing_b = self.storage.contains_valid(b_spend.dbc_id()).await;
                            let (one, two) = match (existing_a, existing_b) {
                                (Some(exists_a), Some(exists_b)) => (exists_a, exists_b),
                                (Some(exists_a), None) => (exists_a, *a_spend),
                                (None, Some(exists_b)) => (exists_b, *b_spend),
                                (None, None) => {
                                    // We don't know about either of these spends, and they are not for the same dbc
                                    // so we can't validate them. We can't add them to the unspendable list
                                    // either, because we don't know if they are valid or not.
                                    // We could try validate and add them though?
                                    // (Or else we just ignore them..)
                                    self.add_if_valid(*a_spend, None).await?;
                                    self.add_if_valid(*b_spend, None).await?;

                                    return Ok(());
                                }
                            };

                            // If their hashes are different, we will add them to the unspendable,
                            // then we notify the group of the correctly identified double spend attempt.
                            if self
                                .try_add_double_if_different_hash(&one, &two)
                                .await
                                .is_ok()
                            {
                                // We have two that are equal, we tried to add them.
                                // We populate the address field properly, and then re-route it.
                                // If we are among the closest, we will add them when it comes
                                // back to us.
                                // (This won't loop, as we have populated the address field properly now.)
                                let request = Request::Event(Event::double_spend_attempt(
                                    Box::new(one),
                                    Box::new(two),
                                )?);
                                let _res = self.send_to_closest(&request).await?;
                            }
                        }
                    }
                    Event::InvalidSpendFound(invalid_spend) => {
                        // If we already know this spend is invalid, we can ignore this event.
                        if self.storage.is_unspendable(invalid_spend.dbc_id()).await {
                            return Ok(());
                        }

                        // If we don't know this spend is invalid, we need to check if it is.
                        // If it is, we will mark it as invalid, and broadcast that to every other close node.
                        match self.validate_spend_parents(&invalid_spend).await {
                            // If the parents do not check out as valid
                            // we will mark this child dbc as unspendable,
                            // and broadcast that to every other close node.
                            Err(super::Error::Protocol(
                                ProtocolError::InvalidParentsForSpendFound(parents),
                            )) => {
                                trace!("Could confirm that parent/s for spend attempt of {request_dst:?} are invalid: {parents:?}!");
                                self.storage.mark_as_unspendable(&invalid_spend).await;
                            }
                            Ok(()) => (),
                            res => res?,
                        };
                    }
                };
            }
        }

        Ok(())
    }

    async fn try_add_double_if_different_hash(
        &mut self,
        a_spend: &SignedSpend,
        b_spend: &SignedSpend,
    ) -> Result<()> {
        let a_hash = sn_dbc::Hash::hash(&a_spend.to_bytes());
        let b_hash = sn_dbc::Hash::hash(&b_spend.to_bytes());
        if a_hash != b_hash {
            self.storage.try_add_double(a_spend, b_spend).await?;
        }
        Ok(())
    }

    /// This function will validate the parents of the provided spend,
    /// as well as the actual spend.
    /// A response will be sent if a response channel is provided.
    async fn add_if_valid(
        &mut self,
        signed_spend: SignedSpend,
        response_channel: Option<ResponseChannel<Response>>,
    ) -> Result<()> {
        let dbc_name = dbc_name(signed_spend.dbc_id());

        // First we need to validate the parents of the spend.
        match self.validate_spend_parents(&signed_spend).await {
            // If the parents do not check out as valid
            // we will mark this child dbc as unspendable,
            // and broadcast that to every other close node.
            Err(super::Error::Protocol(ProtocolError::InvalidParentsForSpendFound(
                all_parents,
            ))) => {
                warn!("Invalid parent/s for spend attempt of {dbc_name:?}: {all_parents:?}!");

                // Broadcast this to close groups of every parent spend in all_parents.
                for invalid_parent in all_parents {
                    let request =
                        Request::Event(Event::InvalidSpendFound(Box::new(*invalid_parent)));
                    let _res = self.send_to_closest(&request).await?;
                }

                // Also broadcast this spend as invalid to every peer in this attempted spend's close group.
                let request =
                    Request::Event(Event::InvalidSpendFound(Box::new(signed_spend.clone())));
                let _resp = self.send_to_closest(&request).await?;
            }
            Ok(()) => (),
            res => res?,
        };

        let response = match self.storage.write(&Cmd::Dbc(signed_spend.clone())).await {
            CmdResponse::Spend(Err(ProtocolError::DoubleSpendAttempt { new, existing })) => {
                warn!("Double spend attempted! New: {new:?}. Existing:  {existing:?}");

                let request =
                    Request::Event(Event::double_spend_attempt(new.clone(), existing.clone())?);
                let _resp = self.send_to_closest(&request).await?;

                CmdResponse::Spend(Err(ProtocolError::DoubleSpendAttempt { new, existing }))
            }
            other => other,
        };

        if let Some(response_channel) = response_channel {
            self.send_response(Response::Cmd(response), response_channel)
                .await;
        }

        Ok(())
    }

    async fn validate_spend_parents(&self, signed_spend: &SignedSpend) -> Result<()> {
        // These will be different spends, one for each input that went into
        // creating the above spend passed in to this function.
        let mut all_parent_signed_spends = BTreeSet::new();

        for parent_input in &signed_spend.spend.tx.inputs {
            let parent_address = dbc_address(&parent_input.dbc_id());
            // This call makes sure we get the same spend from all in the close group.
            // If we receive a spend here, it is assumed to be valid. But we will verify
            // that anyway, in the code right after this for loop.
            let parent_spend_by_close_group = self.get_spend(parent_address).await?;
            let _ = all_parent_signed_spends.insert(parent_spend_by_close_group);
        }

        let mut any_invalid = false;

        // Here we verify every retrieved spends' tx given all of the retrieved spends.
        for parent_spend in &all_parent_signed_spends {
            let creation_tx_of_this_spend = &parent_spend.spend.tx;
            if TransactionVerifier::verify(creation_tx_of_this_spend, &all_parent_signed_spends)
                .is_err()
            {
                any_invalid = true;
            }
        }

        if any_invalid {
            let boxed_spends = all_parent_signed_spends.into_iter().map(Box::new).collect();
            return Err(super::Error::Protocol(
                ProtocolError::InvalidParentsForSpendFound(boxed_spends),
            ));
        }

        Ok(())
    }

    /// Retrieve a `Spend` from the closest peers
    async fn get_spend(&self, address: DbcAddress) -> Result<SignedSpend> {
        let request = Request::Query(Query::GetDbcSpend(address));
        info!("Getting the closest peers to {:?}", request.dst());

        let responses = self.send_to_closest(&request).await?;

        // Get all Ok results of the expected response type `GetDbcSpend`.
        let spends: Vec<_> = responses
            .iter()
            .flatten()
            .flat_map(|resp| {
                if let Response::Query(QueryResponse::GetDbcSpend(Ok(signed_spend))) = resp {
                    Some(signed_spend.clone())
                } else {
                    None
                }
            })
            .collect();

        if spends.len() >= CLOSE_GROUP_SIZE {
            // All nodes in the close group returned an Ok response.
            let spends: BTreeSet<_> = spends.into_iter().collect();
            // All nodes in the close group returned
            // the same spend. It is thus valid.
            if spends.len() == 1 {
                return Ok(spends
                    .first()
                    .expect("This will contain a single item, due to the check before this.")
                    .clone());
            }
            // Different spends returned, the parent is not valid.
        }

        // The parent is not recognised by all peers in its close group.
        // Thus, the parent is not valid.
        info!("The spend could not be verified as valid: {address:?}");

        // If not enough spends were gotten, we try error the first
        // error to the expected query returned from nodes.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetDbcSpend(result)) = resp {
                let _ = result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there was none of the above, then we had unexpected responses.
        Err(super::Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    async fn send_response(&mut self, resp: Response, response_channel: ResponseChannel<Response>) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }

    async fn send_to_closest(&self, request: &Request) -> Result<Vec<Result<Response>>> {
        info!("Sending {:?} to the closest peers.", request.dst());
        let closest_peers = self
            .network
            .get_closest_peers(*request.dst().name())
            .await?;

        Ok(self
            .send_and_get_responses(closest_peers, request, true)
            .await)
    }

    // Send a `Request` to the provided set of peers and wait for their responses concurrently.
    // If `get_all_responses` is true, we wait for the responses from all the peers. Will return an
    // error if the request timeouts.
    // If `get_all_responses` is false, we return the first successful response that we get
    async fn send_and_get_responses(
        &self,
        peers: HashSet<PeerId>,
        req: &Request,
        get_all_responses: bool,
    ) -> Vec<Result<Response>> {
        let mut list_of_futures = Vec::new();
        for peer in peers {
            let future = Box::pin(tokio::time::timeout(
                Duration::from_secs(10),
                self.network.send_request(req.clone(), peer),
            ));
            list_of_futures.push(future);
        }

        let mut responses = Vec::new();
        while !list_of_futures.is_empty() {
            match select_all(list_of_futures).await {
                (Ok(res), _, remaining_futures) => {
                    let res = res.map_err(super::Error::Network);
                    info!("Got response for the req: {req:?}, res: {res:?}");
                    // return the first successful response
                    if !get_all_responses && res.is_ok() {
                        return vec![res];
                    }
                    responses.push(res);
                    list_of_futures = remaining_futures;
                }
                (Err(timeout_err), _, remaining_futures) => {
                    responses.push(Err(super::Error::ResponseTimeout(timeout_err)));
                    list_of_futures = remaining_futures;
                }
            }
        }

        responses
    }
}
