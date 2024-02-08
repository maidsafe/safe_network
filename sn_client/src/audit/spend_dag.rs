// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use petgraph::dot::Dot;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use sn_transfers::{NanoTokens, SignedSpend, SpendAddress};
use std::collections::BTreeMap;
use std::path::Path;

use crate::error::{Error, Result};

/// A DAG representing the spends from a specific Spend all the way to the UTXOs.
/// Starting from Genesis, this would encompass all the spends that have happened on the network
/// at a certain point in time.
///
/// ```text
///                                   -> Spend7 ---> UTXO_11
///                                 /
/// Genesis -> Spend1 -----> Spend2 ---> Spend5 ---> UTXO_10
///                   \
///                     ---> Spend3 ---> Spend6 ---> UTXO_9
///                     \
///                       -> Spend4 ---> UTXO_8
///
/// ```
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SpendDag {
    /// A directed graph of spend addresses
    dag: DiGraph<SpendAddress, NanoTokens>,
    /// All the spends refered to in the dag along with their index in the dag, indexed by their SpendAddress
    spends: BTreeMap<SpendAddress, Vec<(Option<SignedSpend>, usize)>>,
}

impl SpendDag {
    pub fn new() -> Self {
        Self {
            dag: DiGraph::new(),
            spends: BTreeMap::new(),
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let dag: SpendDag = rmp_serde::from_slice(&bytes)?;
        Ok(dag)
    }

    pub fn dump_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let bytes = rmp_serde::to_vec(&self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Insert a spend into the dag
    /// Creating edges (links) from its ancestors and to its descendants
    /// If the inserted spend is already known, it will be ignored
    /// If the inserted spend is a double spend, it will be saved along with the previous spend
    pub fn insert(&mut self, spend_addr: SpendAddress, spend: SignedSpend) {
        // get existing entries for this address
        let entries = self.spends.entry(spend_addr).or_default();
        let existing_entry = entries.iter_mut().find(|(s, _idx)| {
            match s {
                // there is an already an entry for the same spend at this address
                Some(existing_spend) => existing_spend == &spend,
                // there is an UTXO entry for this address
                None => true,
            }
        });

        // update existing entry or save our spend as new
        let node_idx = match existing_entry {
            Some(entry) => {
                *entry = (Some(spend.clone()), entry.1);
                NodeIndex::new(entry.1)
            }
            _ => {
                let node_idx = self.dag.add_node(spend_addr);
                entries.push((Some(spend.clone()), node_idx.index()));
                node_idx
            }
        };

        // link to ancestors
        let spend_amount = spend.token();
        for ancestor in spend.spend.parent_tx.inputs.iter() {
            let ancestor_addr = SpendAddress::from_unique_pubkey(&ancestor.unique_pubkey);

            // add ancestor if not already in dag
            let spends_at_addr = self.spends.entry(ancestor_addr).or_insert_with(|| {
                let node_idx = self.dag.add_node(ancestor_addr);
                vec![(None, node_idx.index())]
            });

            // link to ancestor
            for (_, idx) in spends_at_addr {
                let ancestor_idx = NodeIndex::new(*idx);
                self.dag.update_edge(ancestor_idx, node_idx, *spend_amount);
            }
        }

        // link to descendants
        for descendant in spend.spend.spent_tx.outputs.iter() {
            let descendant_addr = SpendAddress::from_unique_pubkey(&descendant.unique_pubkey);

            // add descendant if not already in dag
            let spends_at_addr = self.spends.entry(descendant_addr).or_insert_with(|| {
                let node_idx = self.dag.add_node(descendant_addr);
                vec![(None, node_idx.index())]
            });

            // link to descendant
            for (_, idx) in spends_at_addr {
                let descendant_idx = NodeIndex::new(*idx);
                self.dag
                    .update_edge(node_idx, descendant_idx, descendant.amount);
            }
        }
    }

    /// checks for existing spend at this address and inserts it if it's new
    /// returns true if it did insert and false if it was already in the DAG
    /// errors out but saves the spend in case of a double spend
    pub fn check_and_insert(
        &mut self,
        spend_addr: SpendAddress,
        spend: SignedSpend,
    ) -> Result<bool> {
        if let Some(existing_spends) = self.spends.get(&spend_addr) {
            match existing_spends.as_slice() {
                // there is an already an entry for the same spend at this address
                [(Some(existing_spend), _)] if existing_spend == &spend => Ok(false),
                // there is already an entry for another spend at this address
                [(Some(existing_spend), _)] if existing_spend != &spend => {
                    // save and report double spend
                    self.insert(spend_addr, spend.clone());
                    Err(Error::DoubleSpend(spend_addr))
                }
                // there is an UTXO entry for this address
                [(None, _)] => {
                    // save spend
                    self.insert(spend_addr, spend);
                    Ok(true)
                }
                // there are already multiple spends at this address
                _ => Err(Error::DoubleSpend(spend_addr)),
            }
        } else {
            // there is no entry for this address
            self.insert(spend_addr, spend);
            Ok(true)
        }
    }

    pub fn get_utxos(&self) -> Vec<SpendAddress> {
        let mut leaves = Vec::new();
        for node_index in self.dag.node_indices() {
            if !self
                .dag
                .neighbors_directed(node_index, petgraph::Direction::Outgoing)
                .any(|_| true)
            {
                let utxo_addr = self.dag[node_index];
                leaves.push(utxo_addr);
            }
        }
        leaves
    }

    pub fn dump_dot_format(&self) -> String {
        format!("{:?}", Dot::with_config(&self.dag, &[]))
    }

    /// Merges the given dag into ours
    pub fn merge(&mut self, sub_dag: SpendDag) {
        for (addr, spends) in sub_dag.spends {
            for (spend, _idx) in spends {
                // only add spends to the dag, ignoring utxos
                // utxos will be added automatically as their ancestors are added
                // edges are updated by the insert method
                if let Some(spend) = spend {
                    self.insert(addr, spend);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spend_dag_serialisation() {
        let dag = SpendDag::new();
        let serialized_data = rmp_serde::to_vec(&dag).expect("Serialization failed");
        let deserialized_instance: SpendDag =
            rmp_serde::from_slice(&serialized_data).expect("Deserialization failed");
        let reserialized_data =
            rmp_serde::to_vec(&deserialized_instance).expect("Serialization failed");
        assert_eq!(reserialized_data, serialized_data);
    }
}
