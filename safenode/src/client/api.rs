// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    Client, ClientEvent, ClientEventsChannel, ClientEventsReceiver,
};

use crate::{
    network::{NetworkEvent, SwarmDriver},
    protocol::{
        messages::{
            Cmd, CmdResponse, Query, QueryResponse, RegisterCmd, RegisterQuery, Request, Response,
            SignedRegisterCreate, SignedRegisterEdit,
        },
        types::{
            address::{ChunkAddress, RegisterAddress},
            chunk::Chunk,
            error::Error as ProtocolError,
            register::Register,
        },
    },
};

use futures::future::select_all;
use libp2p::PeerId;
use std::{collections::HashSet, time::Duration};
use tokio::task::spawn;

impl Client {
    /// Instantiate a new client.
    pub fn new() -> Result<Self> {
        info!("Starting Kad swarm in client mode...");
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new_client()?;
        let events_channel = ClientEventsChannel::default();
        let client = Self {
            network,
            events_channel,
        };
        let mut client_clone = client.clone();

        let _swarm_driver = spawn(swarm_driver.run());
        let _event_handler = spawn(async move {
            loop {
                let event = match network_event_receiver.recv().await {
                    Some(event) => event,
                    None => {
                        error!("The `NetworkEvent` channel has been closed");
                        continue;
                    }
                };
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

    /// Store `Chunk` to its close group.
    pub async fn store_chunk(&self, chunk: Chunk) -> Result<()> {
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

    /// Create a `Register` on the network.
    pub async fn create_register(&self, cmd: SignedRegisterCreate) -> Result<()> {
        info!("Create register: {:?}", cmd.dst());
        let request = Request::Cmd(Cmd::Register(RegisterCmd::Create(cmd)));
        let responses = self.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::CreateRegister(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::CreateRegister(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no register errors, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    /// Edit a `Register` in the network.
    pub async fn edit_register(&self, cmd: SignedRegisterEdit) -> Result<()> {
        info!("Create register: {:?}", cmd.dst());
        let request = Request::Cmd(Cmd::Register(RegisterCmd::Edit(cmd)));
        let responses = self.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::EditRegister(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::EditRegister(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no register errors, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    /// Retrieve a `Chunk` from the closest peers.
    pub async fn get_chunk(&self, address: ChunkAddress) -> Result<Chunk> {
        info!("Get chunk: {address:?}");
        let request = Request::Query(Query::GetChunk(address));
        let responses = self.send_to_closest(request).await?;

        // We will return the first chunk we get.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetChunk(Ok(chunk))) = resp {
                return Ok(chunk.clone());
            };
        }

        // If no chunk was found, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetChunk(result)) = resp {
                let _ = result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there was none of the above, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    /// Retrieve a `Register` from the closest peers.
    pub async fn get_register(&self, address: RegisterAddress) -> Result<Register> {
        info!("Get chunk: {address:?}");
        let request = Request::Query(Query::Register(RegisterQuery::Get(address)));
        let responses = self.send_to_closest(request).await?;

        // We will return the first register we get.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetRegister(Ok(register))) = resp {
                return Ok(register.clone());
            };
        }

        // If no register was gotten, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetChunk(result)) = resp {
                let _ = result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there was none of the above, then we had unexpected responses.
        Err(Error::Protocol(ProtocolError::UnexpectedResponses))
    }

    async fn send_to_closest(&self, request: Request) -> Result<Vec<Result<Response>>> {
        info!("Sending {:?} to the closest peers.", request.dst());
        let closest_peers = self
            .network
            .get_closest_peers(*request.dst().name())
            .await?;
        Ok(self
            .send_and_get_responses(closest_peers, &request, true)
            .await)
    }

    // Send a `Request` to the provided set of nodes and wait for their responses concurrently.
    // If `get_all_responses` is true, we wait for the responses from all the nodes. Will return an
    // error if the request timeouts.
    // If `get_all_responses` is false, we return the first successful response that we get.
    async fn send_and_get_responses(
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
                    let res = res.map_err(Error::Network);
                    info!("Got response for the req: {req:?}, res: {res:?}");
                    // return the first successful response
                    if !get_all_responses && res.is_ok() {
                        return vec![res];
                    }
                    responses.push(res);
                    list_of_futures = remaining_futures;
                }
                (Err(timeout_err), _, remaining_futures) => {
                    responses.push(Err(Error::ResponseTimeout(timeout_err)));
                    list_of_futures = remaining_futures;
                }
            }
        }

        responses
    }
}
