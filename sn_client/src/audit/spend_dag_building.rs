// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{Client, SpendDag};
use crate::{Error, Result};

use futures::future::join_all;
use sn_transfers::{SignedSpend, SpendAddress, WalletError, WalletResult};
use std::collections::BTreeSet;

impl Client {
    /// Builds a SpendDag from a given SpendAddress recursively following descendants all the way to UTxOs
    /// Started from Genesis this gives the entire SpendDag of the Network at a certain point in time
    /// Does not verify the validity of the transactions
    pub async fn spend_dag_build_from(&self, spend_addr: SpendAddress) -> WalletResult<SpendDag> {
        let mut dag = SpendDag::new();

        // get first spend
        let first_spend = match self.get_spend_from_network(spend_addr).await {
            Ok(s) => s,
            Err(Error::MissingSpendRecord(_)) => {
                trace!("UTXO at {spend_addr:?}");
                return Ok(dag);
            }
            Err(e) => return Err(WalletError::FailedToGetSpend(e.to_string())),
        };
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

    /// Extends an existing SpendDag with a new SignedSpend,
    /// tracing back the ancestors of that Spend all the way to a known Spend in the DAG or else back to Genesis
    /// Verifies all transactions on the way, making sure only valid data is inserted in the DAG
    /// This is useful to keep a partial SpendDag to be able to verify that new spends come from Genesis
    ///
    /// ```text
    ///              ... --
    ///                     \
    ///              ... ----                  ... --    
    ///                       \                       \
    /// Spend0 -> Spend1 -----> Spend2 ---> Spend5 ---> Spend2 ---> Genesis
    ///                   \                           /
    ///                    ---> Spend3 ---> Spend6 ->
    ///                     \            /
    ///                      -> Spend4 ->
    ///                                /
    ///                            ...
    ///
    /// ```
    pub async fn spend_dag_extend(
        &self,
        dag: &mut SpendDag,
        spend_addr: SpendAddress,
        new_spend: SignedSpend,
    ) -> WalletResult<()> {
        // check existence of spend in dag
        let is_new_spend = dag
            .check_and_insert(spend_addr, new_spend.clone())
            .map_err(|err| {
                WalletError::CouldNotVerifyTransfer(format!("Failed to insert spend in DAG: {err}"))
            })?;
        if !is_new_spend {
            return Ok(());
        }

        // use iteration instead of recursion to avoid stack overflow
        let mut txs_to_verify = BTreeSet::from_iter([new_spend.spend.parent_tx]);
        let mut depth = 0;
        let mut verified_tx = BTreeSet::new();
        let start = std::time::Instant::now();

        while !txs_to_verify.is_empty() {
            let mut next_gen_tx = BTreeSet::new();

            for parent_tx in txs_to_verify {
                let parent_tx_hash = parent_tx.hash();
                let parent_keys = parent_tx.inputs.iter().map(|input| input.unique_pubkey);
                let addrs_to_verify = parent_keys.map(|k| SpendAddress::from_unique_pubkey(&k));
                debug!("Depth {depth} - Verifying parent Tx : {parent_tx_hash:?}");

                // get all parent spends in parallel
                let tasks: Vec<_> = addrs_to_verify
                    .clone()
                    .map(|a| self.get_spend_from_network(a))
                    .collect();
                let spends = join_all(tasks).await
                    .into_iter()
                    .collect::<Result<BTreeSet<_>>>()
                    .map_err(|err| WalletError::CouldNotVerifyTransfer(format!("at depth {depth} - Failed to get spends from network for parent Tx {parent_tx_hash:?}: {err}")))?;
                debug!(
                    "Depth {depth} - Got {:?} spends for parent Tx: {parent_tx_hash:?}",
                    spends.len()
                );
                trace!("Spends for {parent_tx_hash:?} - {spends:?}");

                // check if we reached the genesis Tx
                if parent_tx == sn_transfers::GENESIS_CASHNOTE.src_tx
                    && spends
                        .iter()
                        .all(|s| s.spend.unique_pubkey == sn_transfers::GENESIS_CASHNOTE.id)
                    && spends.len() == 1
                {
                    debug!("Depth {depth} - Reached genesis Tx on one branch: {parent_tx_hash:?}");
                    verified_tx.insert(parent_tx_hash);
                    continue;
                }

                // verify tx with those spends
                parent_tx
                    .verify_against_inputs_spent(&spends)
                    .map_err(|err| WalletError::CouldNotVerifyTransfer(format!("at depth {depth} - Failed to verify parent Tx {parent_tx_hash:?}: {err}")))?;
                verified_tx.insert(parent_tx_hash);
                debug!("Depth {depth} - Verified parent Tx: {parent_tx_hash:?}");

                // add spends to the dag
                for (spend, addr) in spends.clone().into_iter().zip(addrs_to_verify) {
                    let spend_parent_tx = spend.spend.parent_tx.clone();
                    let is_new_spend = dag.check_and_insert(addr, spend).map_err(|err| {
                        WalletError::CouldNotVerifyTransfer(format!(
                            "Failed to insert spend in DAG: {err}"
                        ))
                    })?;

                    // no need to check this spend's parents if it was already in the DAG
                    if is_new_spend {
                        next_gen_tx.insert(spend_parent_tx);
                    }
                }
            }

            // only verify parents we haven't already verified
            txs_to_verify = next_gen_tx
                .into_iter()
                .filter(|tx| !verified_tx.contains(&tx.hash()))
                .collect();

            depth += 1;
            let elapsed = start.elapsed();
            let n = verified_tx.len();
            info!("Now at depth {depth} - Verified {n} transactions in {elapsed:?}");
        }

        let elapsed = start.elapsed();
        let n = verified_tx.len();
        info!("Verified all the way to known spends or genesis! Through {depth} generations, verifying {n} transactions in {elapsed:?}");
        Ok(())
    }

    /// Extends an existing SpendDag starting from the utxos in this DAG
    /// Covers the entirety of currently existing Spends if the DAG was built from Genesis
    pub async fn spend_dag_continue_from_utxos(&self, dag: &mut SpendDag) -> WalletResult<()> {
        let utxos = dag.get_utxos();
        for utxo in utxos {
            let sub_dag = self.spend_dag_build_from(utxo).await?;
            dag.merge(sub_dag);
        }
        Ok(())
    }
}
