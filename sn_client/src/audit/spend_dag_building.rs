// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Client, Error, SpendDag};

use futures::{future::join_all, StreamExt};
use rand::prelude::SliceRandom;
use sn_networking::{GetRecordError, NetworkError};
use sn_transfers::{SignedSpend, SpendAddress, WalletError, WalletResult};
use std::collections::HashSet;
impl Client {
    /// Builds a SpendDag from a given SpendAddress recursively following descendants all the way to UTxOs
    /// Started from Genesis this gives the entire SpendDag of the Network at a certain point in time
    /// Once the DAG collected, optionally verifies and records errors in the DAG
    pub async fn spend_dag_build_from(
        &self,
        spend_addr: SpendAddress,
        max_updates: Option<usize>,
        verify: bool,
    ) -> WalletResult<SpendDag> {
        info!("Building spend DAG from {spend_addr:?}");
        let mut dag = SpendDag::new(spend_addr);

        let mut dag_updates_applied = 0;

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
        let mut txs_to_follow = HashSet::new();
        txs_to_follow.insert(first_spend.spend.spent_tx);

        let mut known_tx = HashSet::new();
        let mut gen: u32 = 0;
        let start = std::time::Instant::now();

        while !txs_to_follow.is_empty() {
            let mut next_gen_tx = HashSet::new();

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
                // For crawling, a special fetch policy is deployed to improve the performance:
                //   1, Expect `majority` copies as it is a `Spend`;
                //   2, But don't retry as most will be `UTXO` which won't be found.
                .map(|a| async move { (self.crawl_spend_from_network(a).await, a) })
                .buffer_unordered(crate::MAX_CONCURRENT_TASKS * 2);
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
                        info!("Fetched spend {addr:?} from network.");
                        next_gen_tx.insert(spend.spend.spent_tx.clone());
                        dag.insert(addr, spend);
                        dag_updates_applied += 1;

                        if let Some(max_update_count) = max_updates {
                            if dag_updates_applied >= max_update_count {
                                info!(
                                "Reached max updates count of {max_update_count}, stopping DAG collection from {spend_addr:?}"
                            );
                                break;
                            }
                        }
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
            txs_to_follow = HashSet::new();

            for tx in next_gen_tx {
                if !known_tx.contains(&tx.hash()) {
                    txs_to_follow.insert(tx);
                }
            }

            // go on to next gen
            gen += 1;

            if let Some(max_update_count) = max_updates {
                if dag_updates_applied >= max_update_count {
                    info!(
                    "Reached max updates count of {max_update_count}, stopping DAG collection from {spend_addr:?}"
                );
                    break;
                }
            }
        }

        let elapsed = start.elapsed();
        info!("Finished building SpendDAG from {spend_addr:?} in {elapsed:?}");

        // verify the DAG
        if verify {
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
        }
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
        let mut txs_to_verify = HashSet::new();

        txs_to_verify.insert(new_spend.spend.parent_tx);

        let mut depth = 0;
        let mut known_txs = HashSet::new();
        let start = std::time::Instant::now();

        let mut still_txs_to_verify = true;
        while still_txs_to_verify {
            let mut next_gen_tx = HashSet::new();

            for parent_tx in txs_to_verify {
                let parent_tx_hash = parent_tx.hash();
                let parent_keys = parent_tx.inputs.iter().map(|input| input.unique_pubkey);
                let addrs_to_verify = parent_keys.map(|k| SpendAddress::from_unique_pubkey(&k));
                debug!("Depth {depth} - checking parent Tx : {parent_tx_hash:?} with inputs: {addrs_to_verify:?}");

                // get all parent spends in parallel
                let tasks: Vec<_> = addrs_to_verify
                    .clone()
                    .map(|a| self.crawl_spend_from_network(a))
                    .collect();
                let spends = join_all(tasks).await
                    .into_iter()
                    .zip(addrs_to_verify.clone())
                    .map(|(maybe_spend, a)|
                        maybe_spend.map_err(|err| WalletError::CouldNotVerifyTransfer(format!("at depth {depth} - Failed to get spend {a:?} from network for parent Tx {parent_tx_hash:?}: {err}"))))
                    .collect::<WalletResult<HashSet<_>>>()?;
                debug!(
                    "Depth {depth} - Got {:?} spends for parent Tx: {parent_tx_hash:?}",
                    spends.len()
                );
                trace!("Spends for {parent_tx_hash:?} - {spends:?}");

                // check if we reached the genesis Tx
                if parent_tx == *sn_transfers::GENESIS_CASHNOTE_PARENT_TX
                    && spends
                        .iter()
                        .all(|s| s.spend.unique_pubkey == *sn_transfers::GENESIS_SPEND_UNIQUE_KEY)
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

            txs_to_verify = HashSet::new();
            // only verify parents we haven't already verified
            for tx in next_gen_tx {
                if !known_txs.contains(&tx.hash()) {
                    txs_to_verify.insert(tx);
                }
            }

            if txs_to_verify.is_empty() {
                still_txs_to_verify = false;
            }

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

    /// Extends an existing SpendDag starting from the given utxos
    /// If verify is true, records faults in the DAG
    /// Stops gathering after max_depth generations
    ///
    /// Returns the number of updates made to the DAG
    pub async fn spend_dag_continue_from(
        &self,
        dag: &mut SpendDag,
        utxos: HashSet<SpendAddress>,
        max_updates: Option<usize>,
        verify: bool,
    ) -> WalletResult<usize> {
        let main_dag_src = dag.source();
        let starting_dag_size = dag.spends_count();

        debug!("Starting DAG size: {starting_dag_size}");
        let mut total_dag_size = starting_dag_size;

        let all_utxos_len = utxos.len();
        info!(
            "Expanding spend DAG with source: {main_dag_src:?} from {} utxos",
            all_utxos_len
        );
        // Select a subset of utxos up to max_updates, shuffling to prevent bias
        let mut utxos_vec: Vec<_> = utxos.into_iter().collect();
        let mut rng = rand::rngs::OsRng;
        utxos_vec.shuffle(&mut rng);

        let subset_utxos = utxos_vec
            .into_iter()
            .take(max_updates.unwrap_or(all_utxos_len))
            .collect::<HashSet<_>>();
        let mut stream = futures::stream::iter(subset_utxos.into_iter())
            .map(|utxo| async move {
                debug!("Queuing task to gather DAG from utxo: {:?}", utxo);
                (
                    self.spend_dag_build_from(utxo, max_updates, false).await,
                    utxo,
                )
            })
            .buffer_unordered(crate::MAX_CONCURRENT_TASKS * 2);

        while let Some((res, addr)) = stream.next().await {
            match res {
                Ok(sub_dag) => {
                    let new_entries = sub_dag
                        .spends_count()
                        .checked_sub(starting_dag_size)
                        .unwrap_or(0);
                    debug!("subdag spends {:}", sub_dag.spends_count());
                    debug!("Gathered sub DAG from: {addr:?}, with {new_entries} new entries");
                    if let Err(e) = dag.merge(sub_dag, verify) {
                        warn!("Failed to merge sub dag from {addr:?} into dag: {e}");
                    }
                    total_dag_size += new_entries;
                }
                Err(e) => warn!("Failed to gather sub dag from {addr:?}: {e}"),
            };
        }

        info!("Done gathering spend DAG from utxos");

        let total_new_entries = total_dag_size - starting_dag_size;
        Ok(total_new_entries)
    }

    /// Extends an existing SpendDag starting from the utxos in this DAG
    /// Covers the entirety of currently existing Spends if the DAG was built from Genesis
    ///
    /// // if max_updates is Some(n), stops after n updates
    /// If verify is true, records faults in the DAG
    /// Stops gathering after max_depth generations
    pub async fn spend_dag_continue_from_utxos(
        &self,
        dag: &mut SpendDag,
        max_updates: Option<usize>,
        verify: bool,
    ) -> WalletResult<usize> {
        let utxos = dag.get_utxos();

        self.spend_dag_continue_from(dag, utxos, max_updates, verify)
            .await
    }
}
