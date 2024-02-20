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
use sn_transfers::{is_genesis_spend, NanoTokens, SignedSpend, SpendAddress};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::error::{Error, Result};

use super::dag_error::DagError;

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

/// The result of a get operation on the DAG
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum SpendDagGet {
    /// Spend does not exist in the DAG
    NotFound,
    /// Spend is an UTXO, meaning it was not spent yet but its ancestors exist
    Utxo,
    /// Spend is a double spend
    DoubleSpend,
    /// Spend is in the DAG
    Spend(Box<SignedSpend>),
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

    /// Get the spend at a given address
    pub fn get_spend(&self, addr: &SpendAddress) -> SpendDagGet {
        match self.spends.get(addr) {
            None => SpendDagGet::NotFound,
            Some(spends) => match spends.as_slice() {
                [(Some(spend), _)] => SpendDagGet::Spend(Box::new(spend.clone())),
                [(None, _)] => SpendDagGet::Utxo,
                _ => SpendDagGet::DoubleSpend,
            },
        }
    }

    /// helper that returns the spend at a given address if it is unique (not double spend) and not an UTXO
    fn get_unique_spend_at(
        &self,
        addr: &SpendAddress,
        recorded_errors: &mut Vec<DagError>,
    ) -> Option<(&SignedSpend, usize)> {
        let spends = self.spends.get(addr)?;
        match spends.as_slice() {
            // spend
            [(Some(s), i)] => Some((s, *i)),
            // utxo
            [(None, _)] => None,
            // double spend
            _ => {
                recorded_errors.push(DagError::DoubleSpend(*addr));
                None
            }
        }
    }

    /// helper that returns the direct ancestors of a given spend
    fn get_ancestor_spends(
        &self,
        spend: &SignedSpend,
    ) -> std::result::Result<BTreeSet<SignedSpend>, DagError> {
        let mut ancestors = BTreeSet::new();
        for input in spend.spend.parent_tx.inputs.iter() {
            let ancestor_addr = SpendAddress::from_unique_pubkey(&input.unique_pubkey);
            match self.get_unique_spend_at(&ancestor_addr, &mut vec![]) {
                Some((ancestor_spend, _)) => {
                    ancestors.insert(ancestor_spend.clone());
                }
                None => {
                    return Err(DagError::MissingAncestry(ancestor_addr));
                }
            }
        }
        Ok(ancestors)
    }

    /// helper that returns all the descendants (recursively all the way to UTXOs) of a given spend
    fn all_descendants(
        &self,
        addr: &SpendAddress,
        recorded_errors: &mut Vec<DagError>,
    ) -> BTreeSet<&SpendAddress> {
        let mut descendants = BTreeSet::new();
        let mut to_traverse = BTreeSet::from_iter(vec![addr]);
        while let Some(current_addr) = to_traverse.pop_first() {
            // get descendants via DAG
            let (spend, idx) = match self.get_unique_spend_at(current_addr, recorded_errors) {
                Some(s) => s,
                None => continue,
            };
            let descendants_via_dag: BTreeSet<&SpendAddress> = self
                .dag
                .neighbors_directed(NodeIndex::new(idx), petgraph::Direction::Outgoing)
                .map(|i| &self.dag[i])
                .collect();

            // get descendants via Tx data
            let descendants_via_tx: BTreeSet<SpendAddress> = self
                .spends
                .get(current_addr)
                .cloned()
                .unwrap_or(vec![])
                .into_iter()
                .filter_map(|(s, _)| s)
                .flat_map(|s| s.spend.spent_tx.outputs.to_vec())
                .map(|o| SpendAddress::from_unique_pubkey(&o.unique_pubkey))
                .collect();

            // report inconsistencies
            if descendants_via_dag != descendants_via_tx.iter().collect() {
                if !is_genesis_spend(spend) {
                    warn!("Incoherent DAG at: {current_addr:?}");
                    recorded_errors.push(DagError::IncoherentDag(
                        *current_addr,
                        format!("descendants via DAG: {descendants_via_dag:?} do not match descendants via TX: {descendants_via_tx:?}")
                    ));
                } else {
                    debug!("Found Genesis at: {current_addr:?}");
                }
            }

            // continue traversal
            let not_transversed = descendants_via_dag.difference(&descendants);
            to_traverse.extend(not_transversed);
            descendants.extend(descendants_via_dag.iter().cloned());
        }
        descendants
    }

    /// find all the orphans in the DAG and record them as OrphanSpend
    fn find_orphans(&self, source: &SpendAddress, recorded_errors: &mut Vec<DagError>) {
        let all_addresses: BTreeSet<&SpendAddress> = self.spends.keys().collect();
        let descendants = self.all_descendants(source, recorded_errors);
        let orphans: BTreeSet<&SpendAddress> =
            all_addresses.difference(&descendants).cloned().collect();
        for orphan in orphans {
            let src = *source;
            let orphan = *orphan;
            recorded_errors.push(DagError::OrphanSpend { orphan, src });
        }
    }

    /// Verify the DAG
    /// Returns a list of errors found in the DAG
    /// Note that the `MissingSource` error makes the entire DAG invalid
    pub fn verify(&self, source: &SpendAddress) -> Vec<DagError> {
        info!("Verifying DAG starting off: {source:?}");
        let mut recorded_errors = Vec::new();

        // verify DAG source is unique (Genesis in case of a complete DAG)
        debug!("Verifying DAG source is unique: {source:?}");
        let (_source_spend, _) = match self.get_unique_spend_at(source, &mut recorded_errors) {
            Some(s) => s,
            None => {
                recorded_errors.push(DagError::MissingSource(*source));
                return recorded_errors;
            }
        };

        // identify orphans
        debug!("Looking for orphans of {source:?}");
        self.find_orphans(source, &mut recorded_errors);

        // check all transactions
        for (addr, _) in self.spends.iter() {
            debug!("Verifying transaction at: {addr:?}");
            // get the spend at this address
            let (spend, _) = match self.get_unique_spend_at(addr, &mut recorded_errors) {
                Some(s) => s,
                None => continue,
            };

            // skip if genesis
            if is_genesis_spend(spend) {
                debug!("Skip transaction verification for Genesis at: {addr:?}");
                continue;
            }

            // get the ancestors of this spend
            let ancestor_spends = match self.get_ancestor_spends(spend) {
                Ok(a) => a,
                Err(e) => {
                    recorded_errors.push(e);
                    continue;
                }
            };

            // verify the tx
            match spend
                .spend
                .parent_tx
                .verify_against_inputs_spent(&ancestor_spends)
            {
                Ok(_) => (),
                Err(e) => {
                    // mark all descendants as poisoned if tx is invalid
                    recorded_errors.push(DagError::InvalidTransaction(*addr, format!("{e}")));
                    for d in self.all_descendants(addr, &mut recorded_errors) {
                        recorded_errors.push(DagError::PoisonedAncestry(
                            *d,
                            format!("ancestor transaction was poisoned at: {addr:?}: {e}"),
                        ))
                    }
                }
            }
        }

        info!(
            "Found {} errors: {recorded_errors:#?}",
            recorded_errors.len()
        );
        recorded_errors
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
