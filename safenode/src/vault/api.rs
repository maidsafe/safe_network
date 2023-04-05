// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    Vault,
};
use crate::protocol::{
    messages::{Cmd, CmdResponse, Query, QueryResponse, ReplicatedData, Request, Response},
    types::{address::ChunkAddress, chunk::Chunk, errors::Error as ProtocolError},
};
use futures::future::select_all;
use libp2p::PeerId;
use std::{collections::HashSet, time::Duration};
use xor_name::XorName;

impl Vault {
    /// Store `ReplicatedData` to the closest nodes
    pub async fn store_data(&self, data: &ReplicatedData) -> Result<()> {
        info!("Storing data: {:?}", data.name());
        let cmd = match data {
            ReplicatedData::Chunk(chunk) => Cmd::StoreChunk(chunk.clone()),
            ReplicatedData::RegisterWrite(cmd) => Cmd::Register(cmd.clone()),
            ReplicatedData::RegisterLog(_) => todo!(),
        };
        // forward to the other closest nodes if we're seeing the data for the first time
        // return early if we already have the data with us
        match self.storage.store(&cmd).await {
            CmdResponse::StoreChunk(Err(_)) => return Ok(()),
            CmdResponse::CreateRegister(Err(_)) => return Ok(()),
            CmdResponse::EditRegister(Err(_)) => return Ok(()),
            _ => {}
        }
        info!("Forwarding data {:?} to the closest nodes", data.name());
        let closest_nodes = self.network.get_closest_nodes(data.name()).await?;
        let _responses = self
            .send_req_and_get_reponses(closest_nodes, &Request::Cmd(cmd), true)
            .await;

        Ok(())
    }

    /// Retrieve a `Chunk` from the closest nodes
    pub async fn get_chunk(&self, xor_name: XorName) -> Result<Chunk> {
        info!("Get data: {xor_name:?}");
        let closest_nodes = self.network.get_closest_nodes(xor_name).await?;
        let req = Request::Query(Query::GetChunk(ChunkAddress::new(xor_name)));
        let mut response = self
            .send_req_and_get_reponses(closest_nodes, &req, false)
            .await;
        let response = response.remove(0)?;
        if let Response::Query(QueryResponse::GetChunk(chunk)) = response {
            Ok(chunk?)
        } else {
            Err(Error::Protocol(ProtocolError::ChunkNotFound(xor_name)))
        }
    }

    // Send a `Request` to the provided set of nodes and wait for their responses concurretnly.
    // If `get_all_responses` is true, we wait for the responses from all the nodes. Will return an
    // error if the request timesout.
    // If `get_all_responses` is false, we return the first successfull response that we get
    async fn send_req_and_get_reponses(
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
                (Err(timedout_err), _, remaining_futures) => {
                    responses.push(Err(Error::ResponseTimeout(timedout_err)));
                    // return Err(timedout_err.into());
                    list_of_futures = remaining_futures;
                }
            }
        }

        responses
    }
}
