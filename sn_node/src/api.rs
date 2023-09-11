// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, event::NodeEventsChannel, Marker, Network, Node, NodeEvent};
use libp2p::{autonat::NatStatus, identity::Keypair, Multiaddr, PeerId};
use rand::{rngs::StdRng, Rng, SeedableRng};
use sn_dbc::MainKey;
use sn_networking::{MsgResponder, NetworkEvent, SwarmDriver, SwarmLocalState, CLOSE_GROUP_SIZE};
use sn_protocol::{
    messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::wallet::LocalWallet;
use std::{
    collections::HashSet,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::spawn;

/// Once a node is started and running, the user obtains
/// a `NodeRunning` object which can be used to interact with it.
#[derive(Clone)]
pub struct RunningNode {
    network: Network,
    node_events_channel: NodeEventsChannel,
}

impl RunningNode {
    /// Returns this node's `PeerId`
    pub fn peer_id(&self) -> PeerId {
        self.network.peer_id
    }

    /// Returns the root directory path for the node.
    ///
    /// This will either be a value defined by the user, or a default location, plus the peer ID
    /// appended. The default location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/node/<peer-id>
    ///  - macOS: $HOME/Library/Application Support/safe/node/<peer-id>
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\node\<peer-id>
    #[allow(rustdoc::invalid_html_tags)]
    pub fn root_dir_path(&self) -> PathBuf {
        self.network.root_dir_path.clone()
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

    /// Returns the list of all the RecordKeys held by the node
    pub async fn get_all_record_addresses(&self) -> Result<HashSet<NetworkAddress>> {
        let addresses = self.network.get_all_local_record_addresses().await?;
        Ok(addresses)
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
    pub fn run(
        keypair: Keypair,
        addr: SocketAddr,
        initial_peers: Vec<Multiaddr>,
        local: bool,
        root_dir: PathBuf,
    ) -> Result<RunningNode> {
        // TODO: Make this key settable, and accessible via API
        let reward_key = MainKey::random();
        let reward_address = reward_key.public_address();

        let wallet = LocalWallet::load_from_main_key(&root_dir, reward_key)?;
        wallet.store()?;

        let (network, mut network_event_receiver, swarm_driver) =
            SwarmDriver::new(keypair, addr, local, root_dir)?;
        let node_events_channel = NodeEventsChannel::default();

        let node = Self {
            network: network.clone(),
            events_channel: node_events_channel.clone(),
            initial_peers,
            reward_address,
        };

        let network_clone = network.clone();
        let node_event_sender = node_events_channel.clone();
        let mut rng = StdRng::from_entropy();

        let peers_connected = Arc::new(AtomicUsize::new(0));

        let _handle = spawn(swarm_driver.run());
        let _handle = spawn(async move {
            // use a random inactivity timeout to ensure that the nodes do not sync when messages
            // are being transmitted.
            let inactivity_timeout: i32 = rng.gen_range(20..40);
            let inactivity_timeout = Duration::from_secs(inactivity_timeout as u64);

            loop {
                let peers_connected = peers_connected.clone();

                tokio::select! {
                    net_event = network_event_receiver.recv() => {
                        trace!("Handling NetworkEvent: {net_event:?}");
                        match net_event {
                            Some(event) => {
                                let stateless_node_copy = node.clone();
                                let _handle =
                                    spawn(async move { stateless_node_copy.handle_network_event(event, peers_connected).await });
                            }
                            None => {
                                error!("The `NetworkEvent` channel is closed");
                                node_event_sender.broadcast(NodeEvent::ChannelClosed);
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(inactivity_timeout) => {
                        trace!("NetworkEvent inactivity timeout hit");

                        let network_clone = network_clone.clone();

                        Marker::NoNetworkActivity( inactivity_timeout ).log();
                        let _handle = spawn ( async move {
                            let random_target = NetworkAddress::from_peer(PeerId::random());
                            debug!("No network activity in the past {inactivity_timeout:?}, performing a random get_closest query to target: {random_target:?}");
                            match network_clone.node_get_closest_peers(&random_target).await {
                                Ok(closest) => debug!("Network inactivity: get_closest returned {closest:?}"),
                                Err(e) => {
                                    warn!("get_closest query failed after network inactivity timeout - check your connection: {}", e);
                                    Marker::OperationFailedAfterNetworkInactivityTimeout.log();
                                }
                            }
                        });
                    }
                }
            }
        });

        Ok(RunningNode {
            network,
            node_events_channel,
        })
    }

    // **** Private helpers *****

    async fn handle_network_event(&self, event: NetworkEvent, peers_connected: Arc<AtomicUsize>) {
        // when the node has not been connected to enough peers, it should not perform activities
        // that might require peers in the RT to succeed.
        let mut log_when_not_enough_peers = true;
        loop {
            if peers_connected.load(Ordering::Relaxed) >= CLOSE_GROUP_SIZE {
                break;
            }
            match &event {
                // these activities requires the node to be connected to some peer to be able to carry
                // out get kad.get_record etc. This happens during replication/PUT. So we should wait
                // until we have enough nodes, else these might fail.
                NetworkEvent::RequestReceived { .. }
                | NetworkEvent::UnverifiedRecord(_)
                | NetworkEvent::ResponseReceived { .. }
                | NetworkEvent::KeysForReplication(_) => {
                    if log_when_not_enough_peers {
                        debug!("Waiting before processing certain NetworkEvent before reaching {CLOSE_GROUP_SIZE} peers");
                    }
                    log_when_not_enough_peers = false;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                // These events do not need to wait until there are enough peers
                NetworkEvent::PeerAdded(_)
                | NetworkEvent::PeerRemoved(_)
                | NetworkEvent::NewListenAddr(_)
                | NetworkEvent::NatStatusChanged(_) => break,
            }
        }
        trace!("Handling network event {event:?}");

        match event {
            NetworkEvent::RequestReceived { req, channel } => {
                trace!("RequestReceived: {req:?}");
                self.handle_request(req, channel).await;
            }
            NetworkEvent::ResponseReceived { res } => {
                trace!("NetworkEvent::ResponseReceived {res:?}");
                if let Err(err) = self.handle_response(res) {
                    error!("Error while handling NetworkEvent::ResponseReceived {err:?}");
                }
            }
            NetworkEvent::PeerAdded(peer_id) => {
                // increment peers_connected and send ConnectedToNetwork event if have connected to K_VALUE peers
                let _ = peers_connected.fetch_add(1, Ordering::SeqCst);
                if peers_connected.load(Ordering::SeqCst) == CLOSE_GROUP_SIZE {
                    self.events_channel.broadcast(NodeEvent::ConnectedToNetwork);
                }
                Marker::PeerAddedToRoutingTable(peer_id).log();

                if let Err(err) = self.try_trigger_replication(peer_id, false).await {
                    error!("During CloseGroupUpdate, error while triggering replication {err:?}");
                }
            }
            NetworkEvent::PeerRemoved(peer_id) => {
                Marker::PeerRemovedFromRoutingTable(peer_id).log();
                // During a node restart, the new node got added before the old one got removed.
                // If the old one is `pushed out of close_group by the new one`, then the records
                // that being close to the old one won't got replicated during the CloseGroupUpdate
                // of the new one, as the old one still sits in the local kBuckets.
                // Hence, the replication attempts shall also be undertaken when PeerRemoved.
                if let Err(err) = self.try_trigger_replication(peer_id, true).await {
                    error!("During PeerRemoved, error while triggering replication {err:?}");
                }
            }
            NetworkEvent::KeysForReplication(keys) => {
                Marker::fetching_keys_for_replication(&keys).log();

                if let Err(err) = self.fetch_replication_keys_without_wait(keys) {
                    error!("Error while trying to fetch replicated data {err:?}");
                }
            }
            NetworkEvent::NewListenAddr(_) => {
                if !cfg!(feature = "local-discovery") {
                    let network = self.network.clone();
                    let peers = self.initial_peers.clone();
                    let _handle = spawn(async move {
                        for addr in &peers {
                            if let Err(err) = network.dial(addr.clone()).await {
                                tracing::error!("Failed to dial {addr}: {err:?}");
                            };
                        }
                    });
                }
            }
            NetworkEvent::NatStatusChanged(status) => {
                if matches!(status, NatStatus::Private) {
                    tracing::warn!("NAT status is determined to be private!");
                    self.events_channel.broadcast(NodeEvent::BehindNat);
                }
            }
            NetworkEvent::UnverifiedRecord(record) => {
                let key = PrettyPrintRecordKey::from(record.key.clone());
                match self.validate_and_store_record(record).await {
                    Ok(cmdok) => trace!("UnverifiedRecord {key} stored with {cmdok:?}."),
                    Err(err) => {
                        trace!("UnverifiedRecord {key} failed to be stored with error {err:?}.")
                    }
                }
            }
        }
    }

    // Handle the response that was not awaited at the call site
    fn handle_response(&self, response: Response) -> Result<()> {
        match response {
            Response::Cmd(CmdResponse::Replicate(Ok(()))) => {
                // Nothing to do, response was fine
                // This only exists to ensure we dont drop the handle and
                // exit early, potentially logging false connection woes
            }
            other => {
                warn!("handle_response not implemented for {other:?}");
            }
        };

        Ok(())
    }

    async fn handle_request(&self, request: Request, response_channel: MsgResponder) {
        trace!("Handling request: {request:?}");
        let response = match request {
            Request::Cmd(cmd) => self.handle_node_cmd(cmd),
            Request::Query(query) => self.handle_query(query).await,
        };
        self.send_response(response, response_channel);
    }

    async fn handle_query(&self, query: Query) -> Response {
        let resp: QueryResponse = match query {
            Query::GetStoreCost(_address) => {
                trace!("Got GetStoreCost");
                let payment_address = self.reward_address;
                let store_cost = self.current_storecost().await;
                QueryResponse::GetStoreCost {
                    store_cost,
                    payment_address,
                }
            }
        };
        Response::Query(resp)
    }

    fn handle_node_cmd(&self, cmd: Cmd) -> Response {
        Marker::NodeCmdReceived(&cmd).log();
        let resp = match cmd {
            Cmd::Replicate { keys, .. } => {
                debug!("Replicate list received {} keys", keys.len());
                trace!("received replication keys {keys:?}");

                // todo: error is not propagated to the caller here
                let _ = self.add_keys_to_replication_fetcher(keys);
                // if we do not send a response, we can cause connection failures.
                CmdResponse::Replicate(Ok(()))
            }
        };

        Marker::NodeCmdResponded(&resp).log();

        Response::Cmd(resp)
    }

    fn send_response(&self, resp: Response, response_channel: MsgResponder) {
        if let Err(err) = self.network.send_response(resp, response_channel) {
            warn!("Error while sending response: {err:?}");
        }
    }
}
