// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Error, SwarmCmd};
use eyre::{eyre, Result};
use libp2p::{
    kad::{Record, RecordKey},
    PeerId,
};
use sn_protocol::NetworkAddress;
use std::collections::{hash_map::HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct TestSwarm {
    pub records: HashMap<RecordKey, Record>,
    pub record_holders: HashMap<PeerId, HashSet<RecordKey>>,
    pub our_node_peer_id: PeerId,
    pub nodes: HashSet<PeerId>,
}

impl TestSwarm {
    pub fn new(our_node_peer_id: PeerId) -> Self {
        Self {
            records: HashMap::default(),
            record_holders: HashMap::default(),
            our_node_peer_id,
            nodes: HashSet::from([our_node_peer_id]),
        }
    }

    pub async fn handle_swarm_cmd(&mut self, cmd: SwarmCmd) -> Result<()> {
        match cmd {
            SwarmCmd::AddToRoutingTable {
                peer_id, sender, ..
            } => {
                self.nodes.insert(peer_id);
                sender
                    .send(Ok(()))
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::PutLocalRecord { record } => {
                let key = record.key.clone();
                self.record_holders
                    .entry(self.our_node_peer_id)
                    .and_modify(|keys| {
                        keys.insert(key.clone());
                    })
                    .or_insert(HashSet::from([key.clone()]));
                self.records.insert(key, record);
            }
            SwarmCmd::GetNetworkRecord { key, sender } => {
                let record = match self.records.get(&key).cloned() {
                    Some(record) => Ok(record),
                    None => Err(Error::RecordNotFound),
                };
                sender
                    .send(record)
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::GetLocalRecord { key, sender } => {
                let record = if self.record_holders.contains_key(&self.our_node_peer_id) {
                    self.records.get(&key).cloned()
                } else {
                    None
                };
                sender
                    .send(record)
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            SwarmCmd::RecordStoreHasKey { key, sender } => {
                let has_key =
                    if let Some(our_keys) = self.record_holders.get(&self.our_node_peer_id) {
                        our_keys.contains(&key)
                    } else {
                        false
                    };

                sender
                    .send(has_key)
                    .expect("Failed to send through channel");
            }
            SwarmCmd::GetAllRecordAddress { sender } => {
                let addresses = match self.record_holders.get(&self.our_node_peer_id) {
                    Some(keys) => keys
                        .iter()
                        .map(|k| NetworkAddress::from_record_key(k.clone()))
                        .collect(),
                    None => HashSet::new(),
                };
                sender
                    .send(addresses)
                    .map_err(|_| eyre!("Could not send through sender"))?;
            }
            _ => return Err(eyre!("No impl for {cmd:?} inside MockNetwork")),
        }

        Ok(())
    }
}
