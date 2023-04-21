// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    event::NodeEventsChannel,
    replication::send_out_data_addrs,
    Node, NodeEvent, NodeId,
};
use crate::{
    domain::{
        node_transfers::{Error as TransferError, Transfers},
        storage::{
            dbc_address, register::User, ChunkStorage, DbcAddress, Error as StorageError,
            RegisterStorage,
        },
    },
    network::{close_group_majority, NetworkEvent, SwarmDriver},
    node::replication::ask_peers_for_data,
    protocol::{
        error::Error as ProtocolError,
        messages::{
            Cmd, CmdResponse, DataRequest, DataResponse, Event, Query, QueryResponse, RegisterCmd,
            Request, Response, SpendQuery,
        },
    },
};
use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use sn_dbc::{DbcTransaction, MainKey, SignedSpend};
use std::{collections::BTreeSet, net::SocketAddr};
use tokio::task::spawn;
use xor_name::XorName;

impl Node {
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
    pub async fn run(
        addr: SocketAddr,
        initial_peers: Vec<(PeerId, Multiaddr)>,
    ) -> Result<(NodeId, NodeEventsChannel)> {
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new(addr)?;
        let node_events_channel = NodeEventsChannel::default();
        let node_id = NodeId::from(network.peer_id);
        let root_dir = get_root_dir().await?;

        let mut node = Self {
            network: network.clone(),
            chunks: ChunkStorage::new(&root_dir),
            registers: RegisterStorage::new(&root_dir),
            transfers: Transfers::new(node_id, MainKey::random(), &root_dir),
            events_channel: node_events_channel.clone(),
            initial_peers,
        };

        let _swarm_handle = spawn(swarm_driver.run());
        let _replication_handle = spawn(send_out_data_addrs(
            network,
            node.chunks.clone(),
            node.registers.clone(),
        ));
        let _events_handle = spawn(async move {
            loop {
                let event = match network_event_receiver.recv().await {
                    Some(event) => event,
                    None => {
                        error!("The `NetworkEvent` channel has been closed");
                        continue;
                    }
                };
                node.handle_network_event(event).await;
            }
        });

        Ok((node_id, node_events_channel))
    }

    async fn handle_network_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await
            }
            NetworkEvent::PeerAdded => {
                self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
                let target = {
                    let mut rng = rand::thread_rng();
                    XorName::random(&mut rng)
                };
                let network = self.network.clone();
                let _handle = spawn(async move {
                    trace!("Getting closest peers for target {target:?}");
                    let result = network.node_get_closest_peers(target).await;
                    trace!("For target {target:?}, get closest peers {result:?}");
                });
            }
            NetworkEvent::NewListenAddr(_) => {
                let network = self.network.clone();
                let peers = self.initial_peers.clone();
                let _handle = spawn(async move {
                    for (peer_id, addr) in &peers {
                        if let Err(err) = network.dial(*peer_id, addr.clone()).await {
                            tracing::error!("Failed to dial {peer_id}: {err:?}");
                        };
                    }
                });
            }
        }
    }

    // Handles a request from a remote peer. Should always send a `Response` back as the sender will
    // be waiting for it.
    async fn handle_request(
        &mut self,
        request: Request,
        response_channel: ResponseChannel<Response>,
    ) {
        trace!("Handling request: {request:?}");
        let response = match request {
            Request::Data(DataRequest::Cmd(cmd)) => {
                Response::Data(DataResponse::Cmd(self.handle_cmd(cmd).await))
            }
            Request::Data(DataRequest::Query(query)) => {
                Response::Data(DataResponse::Query(self.handle_query(query).await))
            }
            Request::Event(event) => match event {
                Event::DoubleSpendAttempted(a_spend, b_spend) => {
                    match self
                        .transfers
                        .try_add_double(a_spend.as_ref(), b_spend.as_ref())
                        .await
                    {
                        Ok(_) => Response::EventAck(Ok(())),
                        Err(err) => Response::EventAck(Err(ProtocolError::Transfers(err))),
                    }
                }
                Event::ReplicateData(data_addresses) => {
                    let _handle = spawn(ask_peers_for_data(
                        self.network.clone(),
                        self.chunks.clone(),
                        self.registers.clone(),
                        data_addresses,
                    ));
                    Response::EventAck(Ok(()))
                }
            },
        };
        self.send_response(response, response_channel).await;
    }

    async fn handle_query(&mut self, query: Query) -> QueryResponse {
        match query {
            Query::Register(query) => self.registers.read(&query, User::Anyone).await,
            Query::GetChunk(address) => {
                let resp = self
                    .chunks
                    .get(&address)
                    .await
                    .map_err(ProtocolError::Storage);
                QueryResponse::GetChunk(resp)
            }
            Query::Spend(query) => {
                match query {
                    SpendQuery::GetFees { dbc_id, priority } => {
                        // The client is asking for the fee to spend a specific dbc, and including the id of that dbc.
                        // The required fee content is encrypted to that dbc id, and so only the holder of the dbc secret
                        // key can unlock the contents.
                        let required_fee = self.transfers.get_required_fee(dbc_id, priority);
                        QueryResponse::GetFees(Ok(required_fee))
                    }
                    SpendQuery::GetDbcSpend(address) => {
                        let res = self
                            .transfers
                            .get(address)
                            .await
                            .map_err(ProtocolError::Transfers);
                        QueryResponse::GetDbcSpend(res)
                    }
                }
            }
        }
    }

    async fn handle_cmd(&mut self, cmd: Cmd) -> CmdResponse {
        // check if we're among the closest node to the data
        let mut were_closest = false;
        if let Ok(closest) = self.network.node_get_closest_peers(*cmd.dst().name()).await {
            if !closest.contains(&self.network.peer_id) {
                were_closest = true;
            }
        }
        if !were_closest {
            match cmd {
                Cmd::StoreChunk(_) => {
                    return CmdResponse::StoreChunk(Err(ProtocolError::NotClosest))
                }
                Cmd::Register(cmd) => match cmd {
                    RegisterCmd::Create(_) => {
                        return CmdResponse::CreateRegister(Err(ProtocolError::NotClosest))
                    }
                    RegisterCmd::Edit(_) => {
                        return CmdResponse::EditRegister(Err(ProtocolError::NotClosest))
                    }
                },
                Cmd::SpendDbc { .. } => return CmdResponse::Spend(Err(ProtocolError::NotClosest)),
            }
        }
        match cmd {
            Cmd::StoreChunk(chunk) => {
                let resp = self
                    .chunks
                    .store(&chunk)
                    .await
                    .map_err(ProtocolError::Storage);
                CmdResponse::StoreChunk(resp)
            }
            Cmd::Register(cmd) => {
                let result = self
                    .registers
                    .write(&cmd)
                    .await
                    .map_err(ProtocolError::Storage);
                match cmd {
                    RegisterCmd::Create(_) => CmdResponse::CreateRegister(result),
                    RegisterCmd::Edit(_) => CmdResponse::EditRegister(result),
                }
            }
            Cmd::SpendDbc {
                signed_spend,
                parent_tx,
                fee_ciphers,
            } => {
                // First we fetch all parent spends from the network.
                // They shall naturally all exist as valid spends for this current
                // spend attempt to be valid.
                let parent_spends = match self.get_parent_spends(parent_tx.as_ref()).await {
                    Ok(parent_spends) => parent_spends,
                    Err(Error::Protocol(err)) => return CmdResponse::Spend(Err(err)),
                    Err(error) => {
                        return CmdResponse::Spend(Err(ProtocolError::Transfers(
                            TransferError::SpendParentCloseGroupIssue(error.to_string()),
                        )))
                    }
                };

                // Then we try to add the spend to the transfers.
                // This will validate all the necessary components of the spend.
                let res = match self
                    .transfers
                    .try_add(signed_spend, parent_tx, fee_ciphers, parent_spends)
                    .await
                {
                    Err(TransferError::Storage(StorageError::DoubleSpendAttempt {
                        new,
                        existing,
                    })) => {
                        warn!("Double spend attempted! New: {new:?}. Existing:  {existing:?}");
                        if let Ok(event) =
                            Event::double_spend_attempt(new.clone(), existing.clone())
                        {
                            match self
                                .network
                                .node_send_to_closest(&Request::Event(event))
                                .await
                            {
                                Ok(_) => {}
                                Err(err) => {
                                    warn!("Failed to send double spend event to closest peers: {err:?}");
                                }
                            }
                        }

                        Err(ProtocolError::Transfers(TransferError::Storage(
                            StorageError::DoubleSpendAttempt { new, existing },
                        )))
                    }
                    other => other.map_err(ProtocolError::Transfers),
                };

                CmdResponse::Spend(res)
            }
        }
    }

    // This call makes sure we get the same spend from all in the close group.
    // If we receive a spend here, it is assumed to be valid. But we will verify
    // that anyway, in the code right after this for loop.
    async fn get_parent_spends(&self, parent_tx: &DbcTransaction) -> Result<BTreeSet<SignedSpend>> {
        // These will be different spends, one for each input that went into
        // creating the above spend passed in to this function.
        let mut all_parent_spends = BTreeSet::new();

        // First we fetch all parent spends from the network.
        // They shall naturally all exist as valid spends for this current
        // spend attempt to be valid.
        for parent_input in &parent_tx.inputs {
            let parent_address = dbc_address(&parent_input.dbc_id());
            // This call makes sure we get the same spend from all in the close group.
            // If we receive a spend here, it is assumed to be valid. But we will verify
            // that anyway, in the code right after this for loop.
            let parent_spend = self.get_spend(parent_address).await?;
            let _ = all_parent_spends.insert(parent_spend);
        }

        Ok(all_parent_spends)
    }

    /// Retrieve a `Spend` from the closest peers
    async fn get_spend(&self, address: DbcAddress) -> Result<SignedSpend> {
        let request = Request::Data(DataRequest::Query(Query::Spend(SpendQuery::GetDbcSpend(
            address,
        ))));

        let responses = self.network.node_send_to_closest(&request).await?;

        // Get all Ok results of the expected response type `GetDbcSpend`.
        let spends: Vec<_> = responses
            .iter()
            .flatten()
            .flat_map(|resp| {
                if let Response::Data(DataResponse::Query(QueryResponse::GetDbcSpend(Ok(
                    signed_spend,
                )))) = resp
                {
                    Some(signed_spend.clone())
                } else {
                    None
                }
            })
            .collect();

        // As to not have a single rogue node deliver a bogus spend,
        // and thereby have us fail the check here
        // (we would have more than 1 spend in the BTreeSet), we must
        // look for a majority of the same responses, and ignore any other responses.
        if spends.len() >= close_group_majority() {
            // Majority of nodes in the close group returned an Ok response.
            use itertools::*;
            if let Some(spend) = spends
                .into_iter()
                .map(|x| (x, 1))
                .into_group_map()
                .into_iter()
                .filter(|(_, v)| v.len() >= close_group_majority())
                .max_by_key(|(_, v)| v.len())
                .map(|(k, _)| k)
            {
                // Majority of nodes in the close group returned the same spend.
                return Ok(spend);
            }
        }

        // The parent is not recognised by all peers in its close group.
        // Thus, the parent is not valid.
        info!("The spend could not be verified as valid: {address:?}");

        // If not enough spends were gotten, we try error the first
        // error to the expected query returned from nodes.
        for resp in responses.iter().flatten() {
            if let Response::Data(DataResponse::Query(QueryResponse::GetDbcSpend(result))) = resp {
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

    async fn send_response(&self, resp: Response, response_channel: ResponseChannel<Response>) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }
}

async fn get_root_dir() -> Result<std::path::PathBuf> {
    use crate::protocol::error::Error as StorageError;
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("node");
    tokio::fs::create_dir_all(home_dirs.as_path())
        .await
        .map_err(|err| StorageError::Storage(err.into()))?;
    Ok(home_dirs)
}
