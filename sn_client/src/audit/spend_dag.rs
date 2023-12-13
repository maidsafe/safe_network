// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;
use crate::Error;

use futures::future::join_all;
use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use sn_transfers::{SignedSpend, SpendAddress, WalletError, WalletResult};
use std::collections::{BTreeMap, BTreeSet};

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
#[derive(Default, Debug, Clone)]
pub struct SpendDag {
    /// A directed graph of spend addresses
    dag: DiGraph<SpendAddress, bool>,
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

    /// Insert a spend into the dag
    /// Creating edges (links) from its ancestors and to its descendants
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
                self.dag.update_edge(ancestor_idx, node_idx, false);
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
                self.dag.update_edge(node_idx, descendant_idx, false);
            }
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
        format!("{:?}", Dot::with_config(&self.dag, &[Config::EdgeNoLabel]))
    }
}

impl Client {
    pub async fn build_spend_dag_from(&self, spend_addr: SpendAddress) -> WalletResult<SpendDag> {
        let mut dag = SpendDag::new();

        // get first spend
        let first_spend = self
            .get_spend_from_network(spend_addr)
            .await
            .map_err(|err| WalletError::CouldNotVerifyTransfer(err.to_string()))?;
        dag.insert(spend_addr, first_spend.clone());

        // use iteration instead of recursion to avoid stack overflow
        let mut txs_to_follow = BTreeSet::from_iter([first_spend.spend.spent_tx]);
        let mut verified_tx = BTreeSet::new();
        let mut gen = 0;
        let start = std::time::Instant::now();

        while !txs_to_follow.is_empty() {
            let mut next_gen_tx = BTreeSet::new();

            for descendant_tx in txs_to_follow.iter() {
                let descendant_tx_hash = descendant_tx.hash();
                let descendant_keys = descendant_tx
                    .outputs
                    .iter()
                    .map(|output| output.unique_pubkey);
                let addrs_to_follow = descendant_keys.map(|k| SpendAddress::from_unique_pubkey(&k));
                debug!("Gen {gen} - Following descendant Tx : {descendant_tx_hash:?}");

                // get all descendant spends in parallel
                let tasks: Vec<_> = addrs_to_follow
                    .clone()
                    .map(|a| self.get_spend_from_network(a))
                    .collect();
                let spends_res = join_all(tasks).await.into_iter().collect::<Vec<_>>();

                // add spends to dag
                for res in spends_res.iter().zip(addrs_to_follow) {
                    match res {
                        (Ok(spend), addr) => {
                            dag.insert(addr, spend.clone());
                            next_gen_tx.insert(spend.spend.spent_tx.clone());
                        }
                        (Err(Error::MissingSpendRecord(_)), addr) => {
                            trace!("Reached UTXO at {addr:?}");
                        }
                        (Err(err), addr) => {
                            error!("Could not verify transfer at {addr:?}: {err:?}");
                        }
                    }
                }
            }

            // only verify tx we haven't already verified
            gen += 1;
            verified_tx.extend(txs_to_follow.iter().map(|tx| tx.hash()));
            txs_to_follow = next_gen_tx
                .into_iter()
                .filter(|tx| !verified_tx.contains(&tx.hash()))
                .collect();
        }

        let elapsed = start.elapsed();
        info!("Finished building SpendDAG in {elapsed:?}");
        Ok(dag)
    }
}
