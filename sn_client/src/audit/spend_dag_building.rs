// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Client, Error, SpendDag};

use futures::{future::join_all, StreamExt};
use sn_networking::{GetRecordError, NetworkError};
use sn_transfers::{SignedSpend, SpendAddress, WalletError, WalletResult};
use std::collections::BTreeSet;

impl Client {
    /// Builds a SpendDag from a given SpendAddress recursively following descendants all the way to UTxOs
    /// Started from Genesis this gives the entire SpendDag of the Network at a certain point in time
    /// Once the DAG collected, verifies and records errors in the DAG
    pub async fn spend_dag_build_from(
        &self,
        spend_addr: SpendAddress,
        max_depth: Option<u32>,
    ) -> WalletResult<SpendDag> {
        info!("Building spend DAG from {spend_addr:?}");
        let mut dag = SpendDag::new(spend_addr);

        // get first spend
        let first_spend = match self.get_spend_from_network(spend_addr).await {
            Ok(s) => s,
            Err(Error::Network(NetworkError::GetRecordError(GetRecordError::RecordNotFound))) => {
                // the cashnote was not spent yet, so it's an UTXO
                info!("UTXO at {spend_addr:?}");
                return Ok(dag);
            }
            Err(Error::Network(NetworkError::DoubleSpendAttempt(s1, s2))) => {
                // the cashnote was spent twice, take it into account and continue
                info!("Double spend at {spend_addr:?}");
                dag.insert(spend_addr, *s2);
                *s1
            }
            Err(e) => {
                warn!("Failed to get spend at {spend_addr:?}: {e}");
                return Err(WalletError::FailedToGetSpend(e.to_string()));
            }
        };
        dag.insert(spend_addr, first_spend.clone());

        // use iteration instead of recursion to avoid stack overflow
        let mut txs_to_follow = BTreeSet::from_iter([first_spend.spend.spent_tx]);
        let mut known_tx = BTreeSet::new();
        let mut gen: u32 = 0;
        let start = std::time::Instant::now();

        while !txs_to_follow.is_empty() {
            let mut next_gen_tx = BTreeSet::new();

            // list up all descendants
            let mut addrs = vec![];
            for descendant_tx in txs_to_follow.iter() {
                let descendant_tx_hash = descendant_tx.hash();
                let descendant_keys = descendant_tx
                    .outputs
                    .iter()
                    .map(|output| output.unique_pubkey);
                let addrs_to_follow = descendant_keys.map(|k| SpendAddress::from_unique_pubkey(&k));
                info!("Gen {gen} - Following descendant Tx : {descendant_tx_hash:?}");
                addrs.extend(addrs_to_follow);
            }

            // get all spends in parallel
            let mut stream = futures::stream::iter(addrs.clone())
                .map(|a| async move { (self.get_spend_from_network(a).await, a) })
                .buffer_unordered(crate::MAX_CONCURRENT_TASKS);
            info!(
                "Gen {gen} - Getting {} spends from {} txs in batches of: {}",
                addrs.len(),
                txs_to_follow.len(),
                crate::MAX_CONCURRENT_TASKS,
            );

            // insert spends in the dag as they are collected
            while let Some((res, addr)) = stream.next().await {
                match res {
                    Ok(spend) => {
                        dag.insert(addr, spend.clone());
                        next_gen_tx.insert(spend.spend.spent_tx.clone());
                    }
                    Err(Error::Network(NetworkError::GetRecordError(
                        GetRecordError::RecordNotFound,
                    ))) => {
                        info!("Reached UTXO at {addr:?}");
                    }
                    Err(err) => {
                        error!("Failed to get spend at {addr:?} during DAG collection: {err:?}");
                    }
                }
            }

            // only follow tx we haven't already gathered
            known_tx.extend(txs_to_follow.iter().map(|tx| tx.hash()));
            txs_to_follow = next_gen_tx
                .into_iter()
                .filter(|tx| !known_tx.contains(&tx.hash()))
                .collect();

            // go on to next gen
            gen += 1;
            if gen >= max_depth.unwrap_or(u32::MAX) {
                info!("Reached generation {gen}, stopping DAG collection from {spend_addr:?}");
                break;
            }
        }

        let elapsed = start.elapsed();
        info!("Finished building SpendDAG from {spend_addr:?} in {elapsed:?}");

        // verify the DAG
        info!("Now verifying SpendDAG from {spend_addr:?} and recording errors...");
        let start = std::time::Instant::now();
        if let Err(e) = dag.record_faults(&dag.source()) {
            let s = format!(
                "Collected DAG starting at {spend_addr:?} is invalid, this is probably a bug: {e}"
            );
            error!("{s}");
            return Err(WalletError::Dag(s));
        }
        let elapsed = start.elapsed();
        info!("Finished verifying SpendDAG from {spend_addr:?} in {elapsed:?}");
        Ok(dag)
    }

    /// Extends an existing SpendDag with a new SignedSpend,
    /// tracing back the ancestors of that Spend all the way to a known Spend in the DAG or else back to Genesis
    /// Verifies the DAG and records faults if any
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
    pub async fn spend_dag_extend_until(
        &self,
        dag: &mut SpendDag,
        spend_addr: SpendAddress,
        new_spend: SignedSpend,
    ) -> WalletResult<()> {
        // check existence of spend in dag
        let is_new_spend = dag.insert(spend_addr, new_spend.clone());
        if !is_new_spend {
            return Ok(());
        }

        // use iteration instead of recursion to avoid stack overflow
        let mut txs_to_verify = BTreeSet::from_iter([new_spend.spend.parent_tx]);
        let mut depth = 0;
        let mut known_txs = BTreeSet::new();
        let start = std::time::Instant::now();

        while !txs_to_verify.is_empty() {
            let mut next_gen_tx = BTreeSet::new();

            for parent_tx in txs_to_verify {
                let parent_tx_hash = parent_tx.hash();
                let parent_keys = parent_tx.inputs.iter().map(|input| input.unique_pubkey);
                let addrs_to_verify = parent_keys.map(|k| SpendAddress::from_unique_pubkey(&k));
                debug!("Depth {depth} - checking parent Tx : {parent_tx_hash:?} with inputs: {addrs_to_verify:?}");

                // get all parent spends in parallel
                let tasks: Vec<_> = addrs_to_verify
                    .clone()
                    .map(|a| self.get_spend_from_network(a))
                    .collect();
                let spends = join_all(tasks).await
                    .into_iter()
                    .zip(addrs_to_verify.clone())
                    .map(|(maybe_spend, a)|
                        maybe_spend.map_err(|err| WalletError::CouldNotVerifyTransfer(format!("at depth {depth} - Failed to get spend {a:?} from network for parent Tx {parent_tx_hash:?}: {err}"))))
                    .collect::<WalletResult<BTreeSet<_>>>()?;
                debug!(
                    "Depth {depth} - Got {:?} spends for parent Tx: {parent_tx_hash:?}",
                    spends.len()
                );
                trace!("Spends for {parent_tx_hash:?} - {spends:?}");

                // check if we reached the genesis Tx
                if parent_tx == sn_transfers::GENESIS_CASHNOTE.parent_tx
                    && spends.iter().all(|s| {
                        s.spend.unique_pubkey == sn_transfers::GENESIS_CASHNOTE.unique_pubkey
                    })
                    && spends.len() == 1
                {
                    debug!("Depth {depth} - reached genesis Tx on one branch: {parent_tx_hash:?}");
                    known_txs.insert(parent_tx_hash);
                    continue;
                }

                known_txs.insert(parent_tx_hash);
                debug!("Depth {depth} - Verified parent Tx: {parent_tx_hash:?}");

                // add spends to the dag
                for (spend, addr) in spends.clone().into_iter().zip(addrs_to_verify) {
                    let spend_parent_tx = spend.spend.parent_tx.clone();
                    let is_new_spend = dag.insert(addr, spend);

                    // no need to check this spend's parents if it was already in the DAG
                    if is_new_spend {
                        next_gen_tx.insert(spend_parent_tx);
                    }
                }
            }

            // only verify parents we haven't already verified
            txs_to_verify = next_gen_tx
                .into_iter()
                .filter(|tx| !known_txs.contains(&tx.hash()))
                .collect();

            depth += 1;
            let elapsed = start.elapsed();
            let n = known_txs.len();
            info!("Now at depth {depth} - Collected spends from {n} transactions in {elapsed:?}");
        }

        let elapsed = start.elapsed();
        let n = known_txs.len();
        info!("Collected the DAG branch all the way to known spends or genesis! Through {depth} generations, collecting spends from {n} transactions in {elapsed:?}");

        // verify the DAG
        info!("Now verifying SpendDAG extended at {spend_addr:?} and recording errors...");
        let start = std::time::Instant::now();
        if let Err(e) = dag.record_faults(&dag.source()) {
            let s = format!(
                "Collected DAG starting at {spend_addr:?} is invalid, this is probably a bug: {e}"
            );
            error!("{s}");
            return Err(WalletError::Dag(s));
        }
        let elapsed = start.elapsed();
        info!("Finished verifying SpendDAG extended from {spend_addr:?} in {elapsed:?}");
        Ok(())
    }

    /// Extends an existing SpendDag starting from the utxos in this DAG
    /// Covers the entirety of currently existing Spends if the DAG was built from Genesis
    /// Records errors in the new DAG branches if any
    /// Stops gathering after max_depth generations
    pub async fn spend_dag_continue_from_utxos(
        &self,
        dag: &mut SpendDag,
        max_depth: Option<u32>,
    ) -> WalletResult<()> {
        let main_dag_src = dag.source();
        info!("Expanding spend DAG with source: {main_dag_src:?} from utxos...");
        let utxos = dag.get_utxos();

        let mut stream = futures::stream::iter(utxos.into_iter())
            .map(|utxo| async move {
                debug!("Queuing task to gather DAG from utxo: {:?}", utxo);
                (self.spend_dag_build_from(utxo, max_depth).await, utxo)
            })
            .buffer_unordered(crate::MAX_CONCURRENT_TASKS);

        while let Some((res, addr)) = stream.next().await {
            match res {
                Ok(sub_dag) => {
                    debug!("Gathered sub DAG from: {addr:?}");
                    if let Err(e) = dag.merge(sub_dag) {
                        warn!("Failed to merge sub dag from {addr:?} into dag: {e}");
                    }
                }
                Err(e) => warn!("Failed to gather sub dag from {addr:?}: {e}"),
            };
        }

        dag.record_faults(&dag.source())
            .map_err(|e| WalletError::Dag(e.to_string()))?;

        info!("Done gathering spend DAG from utxos");
        Ok(())
    }
}
