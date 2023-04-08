// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Node, NodeEvent};

use crate::{
    network::{NetworkEvent, SwarmDriver},
    protocol::{
        messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response},
        types::{
            address::{dbc_address, DbcAddress},
            error::Error as ProtocolError,
            register::User,
            spend::Spend,
        },
    },
    storage::DataStorage,
};

use futures::future::select_all;
use libp2p::{request_response::ResponseChannel, PeerId};
use sn_dbc::SignedSpend;
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
        match request {
            Request::Cmd(Cmd::Dbc(spend)) => {
                // First we need to validate the parents of the spend.
                self.validate_spend_parents(&spend).await?;
                let resp = self.storage.write(&Cmd::Dbc(spend)).await;
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
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
        }

        Ok(())
    }

    async fn validate_spend_parents(&self, spend: &Spend) -> Result<()> {
        for input in &spend.signed_spend().spend.tx.inputs {
            // We validate each input.
            // input.verify(msg, blinded_amount)
            // Here is supposedly one and the same spend from its close group.
            // If we receive a spend here, it is assumed to be valid.
            let parent_address = dbc_address(&input.dbc_id());
            let parent_spend_by_close_group = self.get_spend(parent_address).await?;
            // We serialize the transaction.
            let msg = parent_spend_by_close_group.spend.tx.gen_message();

            // We check that the input is the expected one, i.e. it has the
            // same amount as the valid parent spend that we got from the parent spend close group.
            match input.verify(&msg, *parent_spend_by_close_group.blinded_amount()) {
                Ok(_) => continue,
                Err(_) => {
                    return Err(super::Error::Protocol(ProtocolError::InvalidSpendParent(
                        parent_address,
                    )))
                }
            };

            // TODO: Do we need more validation of the input parent??
        }

        Ok(())
    }

    /// Retrieve a `Spend` from the closest peers
    async fn get_spend(&self, address: DbcAddress) -> Result<SignedSpend> {
        let request = Request::Query(Query::GetDbcSpend(address));
        info!("Getting the closest peers to {:?}", request.dst());

        let closest_peers = self
            .network
            .get_closest_peers(*request.dst().name())
            .await?;
        // We must know that this size is always the required/expected one.
        let close_group_size = closest_peers.len();

        let responses = self
            .send_req_and_get_responses(closest_peers, &request, true)
            .await;

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

        if spends.len() >= close_group_size {
            // All nodes in the close group returned a response.
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

    // Send a `Request` to the provided set of nodes and wait for their responses concurrently.
    // If `get_all_responses` is true, we wait for the responses from all the nodes. Will return an
    // error if the request timeouts.
    // If `get_all_responses` is false, we return the first successful response that we get
    pub(super) async fn send_req_and_get_responses(
        &self,
        nodes: HashSet<PeerId>,
        req: &Request,
        get_all_responses: bool,
    ) -> Vec<Result<Response>> {
        let mut list_of_futures = Vec::new();
        for node in nodes {
            let future = Box::pin(tokio::time::timeout(
                Duration::from_secs(10),
                self.network.send_request(req.clone(), node),
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
