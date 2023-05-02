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
    Network, Node, NodeEvent,
};

use crate::{
    domain::{dbc_genesis::is_genesis_parent_tx, wallet::LocalWallet},
    network::{close_group_majority, MsgResponder, NetworkEvent, SwarmDriver, SwarmLocalState},
    node::{RegisterStorage, Transfers},
    protocol::{
        error::{Error as ProtocolError, StorageError, TransferError},
        messages::{
            Cmd, CmdResponse, Event, FeeCiphers, NodeId, Query, QueryResponse, RegisterCmd,
            Request, Response, SpendQuery,
        },
        storage::{dbc_address, registers::User, DbcAddress},
    },
};

use sn_dbc::{DbcTransaction, SignedSpend};

use libp2p::{
    kad::{Record, RecordKey},
    Multiaddr, PeerId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    path::Path,
};
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    task::spawn,
};
use xor_name::XorName;

#[derive(Debug)]
pub(super) struct TransferAction {
    signed_spend: Box<SignedSpend>,
    parent_tx: Box<DbcTransaction>,
    fee_ciphers: BTreeMap<NodeId, FeeCiphers>,
    parent_spends: BTreeSet<SignedSpend>,
    response_channel: MsgResponder,
}

/// Once a node is started and running, the user obtains
/// a `NodeRunning` object which can be used to interact with it.
pub struct RunningNode {
    network: Network,
    node_events_channel: NodeEventsChannel,
}

impl RunningNode {
    /// Returns this node's `PeerId`
    pub fn peer_id(&self) -> PeerId {
        self.network.peer_id
    }

    /// Returns a `SwarmLocalState` with some information obtained from swarm's local state.
    pub async fn get_swarm_local_state(&self) -> Result<SwarmLocalState> {
        let state = self.network.get_swarm_local_state().await?;
        Ok(state)
    }

    /// Returns the node events channel where to subscribe to receive `NodeEvent`s
    pub fn node_events_channel(&self) -> &NodeEventsChannel {
        &self.node_events_channel
    }
}

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
        root_dir: &Path,
    ) -> Result<RunningNode> {
        let (network, mut network_event_receiver, swarm_driver) = SwarmDriver::new(addr)?;
        let node_events_channel = NodeEventsChannel::default();

        let node_id = NodeId::from(network.peer_id);
        let node_wallet = LocalWallet::load_from(root_dir)
            .await
            .map_err(|e| Error::CouldNotLoadWallet(e.to_string()))?;

        let (transfer_action_sender, mut transfer_action_receiver) = mpsc::channel(100);

        let mut node = Self {
            network: network.clone(),
            registers: RegisterStorage::new(root_dir),
            transfers: Transfers::new(root_dir, node_id, node_wallet),
            events_channel: node_events_channel.clone(),
            initial_peers,
            transfer_actor: transfer_action_sender,
        };

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            loop {
                match network_event_receiver.try_recv() {
                    Ok(event) => {
                        if let Err(err) = node.handle_network_event(event).await {
                            warn!("Error handling network event: {err}");
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        error!("The `NetworkEvent` channel is closed")
                    }
                    Err(_) => {}
                };
                match transfer_action_receiver.try_recv() {
                    Ok(action) => node.handle_transfer_action(action).await,
                    Err(TryRecvError::Disconnected) => {
                        error!("The `TransferAction` channel is closed")
                    }
                    Err(_) => {}
                };
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });

        Ok(RunningNode {
            network,
            node_events_channel,
        })
    }

    // **** Private helpers *****

    async fn handle_network_event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                self.handle_request(req, channel).await?
            }
            NetworkEvent::PeerAdded(peer) => {
                self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
                let target = {
                    let mut rng = rand::thread_rng();
                    XorName::random(&mut rng)
                };

                let network = self.network.clone();
                let _handle = spawn(async move {
                    trace!("On PeerAdded({peer:?}) Getting closest peers for target {target:?}");
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

        Ok(())
    }

    async fn handle_request(
        &mut self,
        request: Request,
        response_channel: MsgResponder,
    ) -> Result<()> {
        trace!("Handling request: {request:?}");
        match request {
            Request::Cmd(cmd) => self.handle_cmd(cmd, response_channel).await,
            Request::Query(query) => self.handle_query(query, response_channel).await,
            Request::Event(event) => {
                match event {
                    Event::ValidSpendReceived {
                        spend,
                        parent_tx,
                        fee_ciphers,
                        parent_spends,
                    } => {
                        self.transfers
                            .try_add(spend, parent_tx, fee_ciphers, parent_spends)
                            .await
                            .map_err(ProtocolError::Transfers)?;
                    }
                    Event::DoubleSpendAttempted { new, existing } => {
                        self.transfers
                            .try_add_double(new.as_ref(), existing.as_ref())
                            .await
                            .map_err(ProtocolError::Transfers)?;
                    }
                };
            }
        }

        Ok(())
    }

    async fn handle_query(&mut self, query: Query, response_channel: MsgResponder) {
        let resp = match query {
            Query::Register(query) => self.registers.read(&query, User::Anyone).await,
            Query::GetChunk(address) => {
                match self
                    .network
                    .get_provided_data(RecordKey::new(address.name()))
                    .await
                {
                    Ok(Ok(response)) => response,
                    Ok(Err(err)) | Err(err) => {
                        error!("Error getting chunk from network: {err}");
                        QueryResponse::GetChunk(Err(StorageError::ChunkNotFound(address).into()))
                    }
                }
            }
            Query::Spend(query) => {
                match query {
                    SpendQuery::GetFees { dbc_id, priority } => {
                        // The client is asking for the fee to spend a specific dbc, and including the id of that dbc.
                        // The required fee content is encrypted to that dbc id, and so only the holder of the dbc secret
                        // key can unlock the contents.
                        let required_fee = self.transfers.get_required_fee(dbc_id, priority);
                        trace!("Sending response back on query GetFees {dbc_id:?}");
                        QueryResponse::GetFees(Ok(required_fee))
                    }
                    SpendQuery::GetDbcSpend(address) => {
                        let res = self
                            .transfers
                            .get(address)
                            .await
                            .map_err(ProtocolError::Transfers);
                        trace!("Sending response back on query DbcSpend {address:?}");
                        QueryResponse::GetDbcSpend(res)
                    }
                }
            }
        };
        self.send_response(Response::Query(resp), response_channel)
            .await;
    }

    async fn handle_cmd(&mut self, cmd: Cmd, response_channel: MsgResponder) {
        match cmd {
            Cmd::StoreChunk(chunk) => {
                let addr = *chunk.address();
                debug!("That's a store chunk in for :{:?}", addr.name());

                // Create a Kademlia record for storage
                let record = Record {
                    key: RecordKey::new(addr.name()),
                    value: chunk.value().to_vec(),
                    publisher: None,
                    expires: None,
                };

                let resp = match self.network.put_data_as_record(record).await {
                    Ok(()) => {
                        self.events_channel.broadcast(NodeEvent::ChunkStored(addr));
                        CmdResponse::StoreChunk(Ok(()))
                    }
                    Err(err) => {
                        error!("Failed to register node as chunk provider: {err:?}");
                        CmdResponse::StoreChunk(Err(
                            StorageError::ChunkNotStored(*addr.name()).into()
                        ))
                    }
                };
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
            Cmd::Register(cmd) => {
                let result = self
                    .registers
                    .write(&cmd)
                    .await
                    .map_err(ProtocolError::Storage);

                let xorname = cmd.dst();
                let resp = match cmd {
                    RegisterCmd::Create(_) => {
                        self.events_channel
                            .broadcast(NodeEvent::RegisterCreated(xorname));
                        CmdResponse::CreateRegister(result)
                    }
                    RegisterCmd::Edit(_) => {
                        self.events_channel
                            .broadcast(NodeEvent::RegisterEdited(xorname));
                        CmdResponse::EditRegister(result)
                    }
                };
                self.send_response(Response::Cmd(resp), response_channel)
                    .await;
            }
            Cmd::SpendDbc {
                signed_spend,
                parent_tx,
                fee_ciphers,
            } => {
                let network = self.network.clone();
                let transfer_actor = self.transfer_actor.clone();

                let _handler = spawn(async move {
                    handle_spend_dbc(
                        network,
                        transfer_actor,
                        response_channel,
                        signed_spend,
                        parent_tx,
                        fee_ciphers,
                    )
                    .await
                });
            }
        }
    }

    async fn handle_transfer_action(&mut self, action: TransferAction) {
        let TransferAction {
            signed_spend,
            parent_tx,
            fee_ciphers,
            parent_spends,
            response_channel,
        } = action;

        let result = self
            .transfers
            .try_add(
                signed_spend.clone(),
                parent_tx.clone(),
                fee_ciphers.clone(),
                parent_spends.clone(),
            )
            .await;

        let network = self.network.clone();
        let events_channel = self.events_channel.clone();

        let _handler = spawn(async move {
            let resp = match result {
                Ok(()) => {
                    let dbc_id = *signed_spend.dbc_id();
                    trace!("Broadcasting valid spend: {dbc_id:?}");

                    events_channel.broadcast(NodeEvent::SpendStored(dbc_id));

                    let event = Event::ValidSpendReceived {
                        spend: signed_spend,
                        parent_tx,
                        fee_ciphers,
                        parent_spends,
                    };
                    match network
                        .fire_and_forget_to_closest(&Request::Event(event))
                        .await
                    {
                        Ok(_) => {}
                        Err(err) => {
                            warn!("Failed to send valid spend event to closest peers: {err:?}");
                        }
                    }

                    Ok(())
                }
                Err(TransferError::Storage(StorageError::DoubleSpendAttempt { new, existing })) => {
                    warn!("Double spend attempted! New: {new:?}. Existing:  {existing:?}");
                    if let Ok(event) = Event::double_spend_attempt(new.clone(), existing.clone()) {
                        match network.node_send_to_closest(&Request::Event(event)).await {
                            Ok(_) => {}
                            Err(err) => {
                                warn!(
                                    "Failed to send double spend event to closest peers: {err:?}"
                                );
                            }
                        }
                    }

                    Err(ProtocolError::Transfers(TransferError::Storage(
                        StorageError::DoubleSpendAttempt { new, existing },
                    )))
                }
                other => other.map_err(ProtocolError::Transfers),
            };

            if let Err(err) = network
                .send_response(Response::Cmd(CmdResponse::Spend(resp)), response_channel)
                .await
            {
                warn!("Error while sending response: {err:?}");
            }
        });
    }

    async fn send_response(&self, resp: Response, response_channel: MsgResponder) {
        if let Err(err) = self.network.send_response(resp, response_channel).await {
            warn!("Error while sending response: {err:?}");
        }
    }
}

// Node handling of SpendDbc request
async fn handle_spend_dbc(
    network: Network,
    transfer_actor: mpsc::Sender<TransferAction>,
    response_channel: MsgResponder,
    signed_spend: Box<SignedSpend>,
    parent_tx: Box<DbcTransaction>,
    fee_ciphers: BTreeMap<NodeId, FeeCiphers>,
) {
    // First we fetch all parent spends from the network.
    // They shall naturally all exist as valid spends for this current
    // spend attempt to be valid.
    let parent_spends = match get_parent_spends(network.clone(), &parent_tx).await {
        Ok(parent_spends) => parent_spends,
        Err(error) => {
            let resp = if let Error::Protocol(err) = &error {
                CmdResponse::Spend(Err(err.clone()))
            } else {
                CmdResponse::Spend(Err(ProtocolError::Transfers(
                    TransferError::SpendParentCloseGroupIssue(error.to_string()),
                )))
            };

            if let Err(err) = network
                .send_response(Response::Cmd(resp), response_channel)
                .await
            {
                warn!("Error while sending response: {err:?}");
            }
            return;
        }
    };

    let transfer_action = TransferAction {
        signed_spend,
        parent_tx,
        fee_ciphers,
        parent_spends,
        response_channel,
    };

    // Then we try to add the spend to the transfers.
    // This will validate all the necessary components of the spend.
    if let Err(err) = transfer_actor.send(transfer_action).await {
        warn!("Failed to send transfer action with {err:?}");
    }
}

// This call makes sure we get the same spend from all in the close group.
// If we receive a spend here, it is assumed to be valid. But we will verify
// that anyway, in the code right after this for loop.
async fn get_parent_spends(
    network: Network,
    parent_tx: &DbcTransaction,
) -> Result<BTreeSet<SignedSpend>> {
    // These will be different spends, one for each input that went into
    // creating the above spend passed in to this function.
    let mut all_parent_spends = BTreeSet::new();

    if is_genesis_parent_tx(parent_tx) {
        return Ok(all_parent_spends);
    }

    // First we fetch all parent spends from the network.
    // They shall naturally all exist as valid spends for this current
    // spend attempt to be valid.
    for parent_input in &parent_tx.inputs {
        let parent_address = dbc_address(&parent_input.dbc_id());
        // This call makes sure we get the same spend from all in the close group.
        // If we receive a spend here, it is assumed to be valid. But we will verify
        // that anyway, in the code right after this for loop.
        let parent_spend = get_spend(network.clone(), parent_address).await?;
        let _ = all_parent_spends.insert(parent_spend);
    }

    Ok(all_parent_spends)
}

/// Retrieve a `Spend` from the closest peers
async fn get_spend(network: Network, address: DbcAddress) -> Result<SignedSpend> {
    let request = Request::Query(Query::Spend(SpendQuery::GetDbcSpend(address)));
    let responses = network.node_send_to_closest(&request).await?;

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

    // The parent is not recognised by majority of peers in its close group.
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
    Err(Error::UnexpectedResponses)
}
