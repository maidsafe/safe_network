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
use sn_transfers::{
    SignedSpend, SpendAddress, SpendReason, WalletError, WalletResult,
    DEFAULT_NETWORK_ROYALTIES_PK, GENESIS_SPEND_UNIQUE_KEY, NETWORK_ROYALTIES_PK,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};
use tokio::{sync::mpsc::Sender, task::JoinSet};

const SPENDS_PROCESSING_BUFFER_SIZE: usize = 4096;

enum InternalGetNetworkSpend {
    Spend(Box<SignedSpend>),
    DoubleSpend(Vec<SignedSpend>),
    NotFound,
    Error(Error),
}

impl Client {
    pub async fn new_dag_with_genesis_only(&self) -> WalletResult<SpendDag> {
        let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_SPEND_UNIQUE_KEY);
        let mut dag = SpendDag::new(genesis_addr);
        match self.get_spend_from_network(genesis_addr).await {
            Ok(spend) => {
                dag.insert(genesis_addr, spend);
            }
            Err(Error::Network(NetworkError::DoubleSpendAttempt(spends))) => {
                println!("Double spend detected at Genesis: {genesis_addr:?}");
                for spend in spends.into_iter() {
                    dag.insert(genesis_addr, spend);
                }
            }
            Err(e) => return Err(WalletError::FailedToGetSpend(e.to_string())),
        };

        Ok(dag)
    }

    /// Builds a SpendDag from a given SpendAddress recursively following descendants all the way to UTxOs
    /// Started from Genesis this gives the entire SpendDag of the Network at a certain point in time
    /// Once the DAG collected, optionally verifies and records errors in the DAG
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
    pub async fn spend_dag_build_from(
        &self,
        spend_addr: SpendAddress,
        spend_processing: Option<Sender<(SignedSpend, u64)>>,
        verify: bool,
    ) -> WalletResult<SpendDag> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(SPENDS_PROCESSING_BUFFER_SIZE);

        // start crawling from the given spend address
        let self_clone = self.clone();
        let crawl_handle =
            tokio::spawn(async move { self_clone.spend_dag_crawl_from(spend_addr, tx).await });

        // start DAG building from the spends gathered while crawling
        // forward spends to processing if provided
        let build_handle: tokio::task::JoinHandle<Result<SpendDag, WalletError>> =
            tokio::spawn(async move {
                debug!("Starting building DAG from {spend_addr:?}...");
                let now = std::time::Instant::now();
                let mut dag = SpendDag::new(spend_addr);
                while let Some(spend) = rx.recv().await {
                    let addr = spend.address();
                    debug!(
                        "Inserting spend at {addr:?} size: {}",
                        dag.all_spends().len()
                    );
                    dag.insert(addr, spend.clone());
                    if let Some(sender) = &spend_processing {
                        let outputs = spend.spend.spent_tx.outputs.len() as u64;
                        sender
                            .send((spend, outputs))
                            .await
                            .map_err(|e| WalletError::SpendProcessing(e.to_string()))?;
                    }
                }
                info!(
                    "Done gathering DAG of size: {} in {:?}",
                    dag.all_spends().len(),
                    now.elapsed()
                );
                Ok(dag)
            });

        // wait for both to finish
        let (crawl_res, build_res) = tokio::join!(crawl_handle, build_handle);
        crawl_res.map_err(|e| {
            WalletError::SpendProcessing(format!("Failed to Join crawling results {e}"))
        })??;
        let mut dag = build_res.map_err(|e| {
            WalletError::SpendProcessing(format!("Failed to Join DAG building results {e}"))
        })??;

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

    /// Get spends from a set of given SpendAddresses
    /// Recursivly fetching till reached frontline of the DAG tree.
    /// Return with UTXOs for re-attempt (with insertion time stamp)
    pub async fn crawl_to_next_utxos(
        &self,
        addrs_to_get: &mut BTreeSet<SpendAddress>,
        sender: Sender<(SignedSpend, u64)>,
        reattempt_interval: Duration,
    ) -> WalletResult<BTreeMap<SpendAddress, Instant>> {
        let mut failed_utxos = BTreeMap::new();
        let mut tasks = JoinSet::new();

        while !addrs_to_get.is_empty() || !tasks.is_empty() {
            while tasks.len() < 32 && !addrs_to_get.is_empty() {
                if let Some(addr) = addrs_to_get.pop_first() {
                    let client_clone = self.clone();
                    let _ =
                        tasks.spawn(async move { (client_clone.crawl_spend(addr).await, addr) });
                }
            }

            if let Some(Ok((result, address))) = tasks.join_next().await {
                match result {
                    InternalGetNetworkSpend::Spend(spend) => {
                        let for_further_track = beta_track_analyze_spend(&spend);
                        let _ = sender
                            .send((*spend, for_further_track.len() as u64))
                            .await
                            .map_err(|e| WalletError::SpendProcessing(e.to_string()));
                        addrs_to_get.extend(for_further_track);
                    }
                    InternalGetNetworkSpend::DoubleSpend(_spends) => {
                        warn!("Detected double spend regarding {address:?}");
                    }
                    InternalGetNetworkSpend::NotFound => {
                        let _ = failed_utxos.insert(address, Instant::now() + reattempt_interval);
                    }
                    InternalGetNetworkSpend::Error(e) => {
                        warn!("Fetching spend {address:?} result in error {e:?}");
                        // Error of `NotEnoughCopies` could be re-attempted and succeed eventually.
                        let _ = failed_utxos.insert(address, Instant::now() + reattempt_interval);
                    }
                }
            }
        }

        Ok(failed_utxos)
    }

    /// Crawls the Spend Dag from a given SpendAddress recursively
    /// following descendants all the way to UTXOs
    /// Returns the UTXOs reached
    pub async fn spend_dag_crawl_from(
        &self,
        spend_addr: SpendAddress,
        spend_processing: Sender<SignedSpend>,
    ) -> WalletResult<BTreeSet<SpendAddress>> {
        info!("Crawling spend DAG from {spend_addr:?}");
        let mut utxos = BTreeSet::new();

        // get first spend
        let mut txs_to_follow = match self.crawl_spend(spend_addr).await {
            InternalGetNetworkSpend::Spend(spend) => {
                let spend = *spend;
                let txs = BTreeSet::from_iter([spend.spend.spent_tx.clone()]);

                spend_processing
                    .send(spend)
                    .await
                    .map_err(|e| WalletError::SpendProcessing(e.to_string()))?;
                txs
            }
            InternalGetNetworkSpend::DoubleSpend(spends) => {
                let mut txs = BTreeSet::new();
                for spend in spends.into_iter() {
                    txs.insert(spend.spend.spent_tx.clone());
                    spend_processing
                        .send(spend)
                        .await
                        .map_err(|e| WalletError::SpendProcessing(e.to_string()))?;
                }
                txs
            }
            InternalGetNetworkSpend::NotFound => {
                // the cashnote was not spent yet, so it's an UTXO
                info!("UTXO at {spend_addr:?}");
                utxos.insert(spend_addr);
                return Ok(utxos);
            }
            InternalGetNetworkSpend::Error(e) => {
                return Err(WalletError::FailedToGetSpend(e.to_string()));
            }
        };

        // use iteration instead of recursion to avoid stack overflow
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
                .map(|a| async move { (self.crawl_spend(a).await, a) })
                .buffer_unordered(crate::MAX_CONCURRENT_TASKS);
            info!(
                "Gen {gen} - Getting {} spends from {} txs in batches of: {}",
                addrs.len(),
                txs_to_follow.len(),
                crate::MAX_CONCURRENT_TASKS,
            );

            // insert spends in the dag as they are collected
            while let Some((get_spend, addr)) = stream.next().await {
                match get_spend {
                    InternalGetNetworkSpend::Spend(spend) => {
                        next_gen_tx.insert(spend.spend.spent_tx.clone());
                        spend_processing
                            .send(*spend.clone())
                            .await
                            .map_err(|e| WalletError::SpendProcessing(e.to_string()))?;
                    }
                    InternalGetNetworkSpend::DoubleSpend(spends) => {
                        info!("Fetched double spend(s) of len {} at {addr:?} from network, following all of them.", spends.len());
                        for s in spends.into_iter() {
                            next_gen_tx.insert(s.spend.spent_tx.clone());
                            spend_processing
                                .send(s.clone())
                                .await
                                .map_err(|e| WalletError::SpendProcessing(e.to_string()))?;
                        }
                    }
                    InternalGetNetworkSpend::NotFound => {
                        info!("Reached UTXO at {addr:?}");
                        utxos.insert(addr);
                    }
                    InternalGetNetworkSpend::Error(err) => {
                        error!("Failed to get spend at {addr:?} during DAG collection: {err:?}")
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
        }

        let elapsed = start.elapsed();
        info!("Finished crawling SpendDAG from {spend_addr:?} in {elapsed:?}");
        Ok(utxos)
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
                    .map(|a| self.crawl_spend(a))
                    .collect();
                let mut spends = BTreeSet::new();
                for (spend_get, a) in join_all(tasks)
                    .await
                    .into_iter()
                    .zip(addrs_to_verify.clone())
                {
                    match spend_get {
                        InternalGetNetworkSpend::Spend(s) => {
                            spends.insert(*s);
                        }
                        InternalGetNetworkSpend::DoubleSpend(s) => {
                            spends.extend(s.into_iter());
                        }
                        InternalGetNetworkSpend::NotFound => {
                            return Err(WalletError::FailedToGetSpend(format!(
                                "Missing ancestor spend at {a:?}"
                            )))
                        }
                        InternalGetNetworkSpend::Error(e) => {
                            return Err(WalletError::FailedToGetSpend(format!(
                                "Failed to get ancestor spend at {a:?}: {e}"
                            )))
                        }
                    }
                }
                let spends_len = spends.len();
                debug!("Depth {depth} - Got {spends_len} spends for parent Tx: {parent_tx_hash:?}");
                trace!("Spends for {parent_tx_hash:?} - {spends:?}");

                // check if we reached the genesis Tx
                known_txs.insert(parent_tx_hash);
                if parent_tx == *sn_transfers::GENESIS_CASHNOTE_PARENT_TX
                    && spends
                        .iter()
                        .all(|s| s.spend.unique_pubkey == *sn_transfers::GENESIS_SPEND_UNIQUE_KEY)
                    && spends.len() == 1
                {
                    debug!("Depth {depth} - reached genesis Tx on one branch: {parent_tx_hash:?}");
                    continue;
                }

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

    /// Extends an existing SpendDag starting from the given utxos
    /// If verify is true, records faults in the DAG
    pub async fn spend_dag_continue_from(
        &self,
        dag: &mut SpendDag,
        utxos: BTreeSet<SpendAddress>,
        spend_processing: Option<Sender<(SignedSpend, u64)>>,
        verify: bool,
    ) {
        let main_dag_src = dag.source();
        info!(
            "Expanding spend DAG with source: {main_dag_src:?} from {} utxos",
            utxos.len()
        );

        let sender = spend_processing.clone();
        let tasks = utxos
            .iter()
            .map(|utxo| self.spend_dag_build_from(*utxo, sender.clone(), false));
        let sub_dags = join_all(tasks).await;
        for (res, addr) in sub_dags.into_iter().zip(utxos.into_iter()) {
            match res {
                Ok(sub_dag) => {
                    debug!("Gathered sub DAG from: {addr:?}");
                    if let Err(e) = dag.merge(sub_dag, verify) {
                        warn!("Failed to merge sub dag from {addr:?} into dag: {e}");
                    }
                }
                Err(e) => warn!("Failed to gather sub dag from {addr:?}: {e}"),
            };
        }

        info!("Done gathering spend DAG from utxos");
    }

    /// Extends an existing SpendDag starting from the utxos in this DAG
    /// Covers the entirety of currently existing Spends if the DAG was built from Genesis
    /// If verify is true, records faults in the DAG
    /// Stops gathering after max_depth generations
    pub async fn spend_dag_continue_from_utxos(
        &self,
        dag: &mut SpendDag,
        spend_processing: Option<Sender<(SignedSpend, u64)>>,
        verify: bool,
    ) {
        let utxos = dag.get_utxos();
        self.spend_dag_continue_from(dag, utxos, spend_processing, verify)
            .await
    }

    /// Internal get spend helper for DAG purposes
    /// For crawling, a special fetch policy is deployed to improve the performance:
    ///   1. Expect `majority` copies as it is a `Spend`;
    ///   2. But don't retry as most will be `UTXO` which won't be found.
    async fn crawl_spend(&self, spend_addr: SpendAddress) -> InternalGetNetworkSpend {
        match self.crawl_spend_from_network(spend_addr).await {
            Ok(s) => {
                debug!("DAG crawling: fetched spend {spend_addr:?} from network");
                InternalGetNetworkSpend::Spend(Box::new(s))
            }
            Err(Error::Network(NetworkError::GetRecordError(GetRecordError::RecordNotFound))) => {
                debug!("DAG crawling: spend at {spend_addr:?} not found on the network");
                InternalGetNetworkSpend::NotFound
            }
            Err(Error::Network(NetworkError::DoubleSpendAttempt(spends))) => {
                debug!("DAG crawling: got a double spend(s) of len {} at {spend_addr:?} on the network", spends.len());
                InternalGetNetworkSpend::DoubleSpend(spends)
            }
            Err(e) => {
                debug!(
                    "DAG crawling: got an error for spend at {spend_addr:?} on the network: {e}"
                );
                InternalGetNetworkSpend::Error(e)
            }
        }
    }
}

/// Helper function to analyze spend for beta_tracking optimization.
/// returns the new_utxos that needs to be further tracked.
fn beta_track_analyze_spend(spend: &SignedSpend) -> BTreeSet<SpendAddress> {
    // Filter out royalty outputs
    let royalty_pubkeys: BTreeSet<_> = spend
        .spend
        .network_royalties
        .iter()
        .map(|derivation_idx| NETWORK_ROYALTIES_PK.new_unique_pubkey(derivation_idx))
        .collect();
    let default_royalty_pubkeys: BTreeSet<_> = spend
        .spend
        .network_royalties
        .iter()
        .map(|derivation_idx| DEFAULT_NETWORK_ROYALTIES_PK.new_unique_pubkey(derivation_idx))
        .collect();

    let new_utxos: BTreeSet<_> = spend
        .spend
        .spent_tx
        .outputs
        .iter()
        .filter_map(|output| {
            if default_royalty_pubkeys.contains(&output.unique_pubkey) {
                return None;
            }
            if !royalty_pubkeys.contains(&output.unique_pubkey) {
                Some(SpendAddress::from_unique_pubkey(&output.unique_pubkey))
            } else {
                None
            }
        })
        .collect();

    if let SpendReason::BetaRewardTracking(_) = spend.reason() {
        // Do not track down forwarded payment further
        Default::default()
    } else {
        trace!(
            "Spend original has {} outputs, tracking {} of them.",
            spend.spend.spent_tx.outputs.len(),
            new_utxos.len()
        );
        new_utxos
    }
}
