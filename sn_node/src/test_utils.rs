// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use eyre::{eyre, Result};
use libp2p::{
    identity::Keypair,
    kad::{Record, RecordKey},
    PeerId,
};
use sn_dbc::SignedSpend;
use sn_networking::{Error as NetworkError, SwarmCmd, CLOSE_GROUP_SIZE};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{Cmd, Query, QueryResponse, Request, Response},
    storage::try_deserialize_record,
};
use std::collections::{hash_map::HashMap, HashSet};
use tokio::sync::mpsc::Receiver;

#[derive(Clone)]
pub(crate) struct TestSwarm {
    pub(crate) records: HashMap<RecordKey, Record>,
    pub(crate) record_holders: HashMap<PeerId, HashSet<RecordKey>>,
    pub(crate) our_node: PeerId,
    pub(crate) nodes: HashSet<PeerId>,
}

impl TestSwarm {
    pub(crate) fn new(our_node: PeerId) -> Self {
        Self {
            records: HashMap::default(),
            record_holders: HashMap::default(),
            nodes: HashSet::from([our_node]),
            our_node,
        }
    }

    pub(crate) fn add_nodes(&mut self, num: usize) {
        for _ in 0..num {
            let keypair = Keypair::generate_ed25519();
            let peer_id = PeerId::from(keypair.public());
            let _ = self.nodes.insert(peer_id);
        }
    }

    pub(crate) fn spawn_handler(mut self, mut swarm_cmd_rx: Receiver<SwarmCmd>) {
        let _handle = tokio::spawn(async move {
            loop {
                let cmd = swarm_cmd_rx
                    .recv()
                    .await
                    .expect("Swarm cmd channel has been closed");

                if let Err(err) = self.handle_swarm_cmd(cmd) {
                    panic!("MockNetwork failed with {err:?}");
                };
            }
        });
    }

    pub(crate) fn handle_swarm_cmd(&mut self, cmd: SwarmCmd) -> Result<()> {
        match cmd {
            SwarmCmd::AddToRoutingTable {
                peer_id, sender, ..
            } => {
                let _ = self.nodes.insert(peer_id);
                sender
                    .send(Ok(()))
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::PutLocalRecord { record } => {
                let key = record.key.clone();
                let _ = self
                    .record_holders
                    .entry(self.our_node)
                    .and_modify(|keys| {
                        let _ = keys.insert(key.clone());
                    })
                    .or_insert(HashSet::from([key.clone()]));
                let _ = self.records.insert(key, record);
            }
            SwarmCmd::GetNetworkRecord { key, sender } => {
                let record = match self.records.get(&key).cloned() {
                    Some(record) => Ok(record),
                    None => Err(NetworkError::RecordNotFound),
                };
                sender
                    .send(record)
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::GetLocalRecord { key, sender } => {
                let record = if self.record_holders.contains_key(&self.our_node) {
                    self.records.get(&key).cloned()
                } else {
                    None
                };
                sender
                    .send(record)
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::RecordStoreHasKey { key, sender } => {
                let has_key = if let Some(our_keys) = self.record_holders.get(&self.our_node) {
                    our_keys.contains(&key)
                } else {
                    false
                };

                sender
                    .send(has_key)
                    .expect("Failed to send through channel");
            }
            SwarmCmd::GetClosestPeers { key, sender } => {
                let closest = self.nodes.iter().take(CLOSE_GROUP_SIZE).cloned().collect();
                sender
                    .send(closest)
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::SendRequest { req, sender, .. } => match req {
                Request::Cmd(cmd) => match cmd {
                    Cmd::SpendDbc(_) => todo!(),
                    Cmd::Replicate { holder, keys } => todo!(),
                    Cmd::RequestReplication(_) => todo!(),
                },
                Request::Query(query) => {
                    let resp = match query {
                        Query::GetChunk(_) => todo!(),
                        Query::GetSpend(addr) => {
                            let key = RecordKey::new(addr.name());
                            match self.records.get(&key) {
                                Some(record) => {
                                    let mut spends =
                                        try_deserialize_record::<Vec<SignedSpend>>(record)?;
                                    QueryResponse::GetDbcSpend(Ok(spends.remove(0)))
                                }
                                None => QueryResponse::GetDbcSpend(Err(
                                    ProtocolError::SpendNotFound(addr),
                                )),
                            }
                        }
                        Query::GetReplicatedData { requester, address } => {
                            todo!()
                        }
                    };
                    if let Some(sender) = sender {
                        sender.send(Ok(Response::Query(resp)));
                    }
                }
            },
            _ => return Err(eyre!("No impl for {cmd:?} inside MockNetwork")),
        }

        Ok(())
    }
}
