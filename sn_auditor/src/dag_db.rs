// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::SecretKey;
#[cfg(feature = "svg-dag")]
use color_eyre::eyre::Context;
use color_eyre::eyre::{bail, eyre, Result};
#[cfg(feature = "svg-dag")]
use graphviz_rust::{cmd::Format, exec, parse, printer::PrinterContext};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sn_client::transfers::{
    Hash, NanoTokens, SignedSpend, SpendAddress, DEFAULT_PAYMENT_FORWARD_SK,
};
use sn_client::transfers::{DEFAULT_NETWORK_ROYALTIES_PK, NETWORK_ROYALTIES_PK};
use sn_client::{Client, SpendDag, SpendDagGet};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

pub const SPEND_DAG_FILENAME: &str = "spend_dag";
#[cfg(feature = "svg-dag")]
pub const SPEND_DAG_SVG_FILENAME: &str = "spend_dag.svg";
/// Store a locally copy to restore on restart
pub const BETA_PARTICIPANTS_FILENAME: &str = "beta_participants.txt";

lazy_static! {
    /// time in seconds UTXOs are refetched in DAG crawl
    static ref UTXO_REATTEMPT_INTERVAL: Duration = Duration::from_secs(
        std::env::var("UTXO_REATTEMPT_INTERVAL")
            .unwrap_or("7200".to_string())
            .parse::<u64>()
            .unwrap_or(7200)
    );

    /// time in seconds to rest between DAG crawls
    static ref DAG_CRAWL_REST_INTERVAL: Duration = Duration::from_secs(
        std::env::var("DAG_CRAWL_REST_INTERVAL")
            .unwrap_or("60".to_string())
            .parse::<u64>()
            .unwrap_or(60)
    );
}

const SPENDS_PROCESSING_BUFFER_SIZE: usize = 4096;

/// Abstraction for the Spend DAG database
/// Currently in memory, with disk backup, but should probably be a real DB at scale
#[derive(Clone)]
pub struct SpendDagDb {
    client: Option<Client>,
    pub(crate) path: PathBuf,
    dag: Arc<RwLock<SpendDag>>,
    beta_tracking: Arc<RwLock<BetaTracking>>,
    beta_participants: Arc<RwLock<BTreeMap<Hash, String>>>,
    utxo_addresses: Arc<RwLock<BTreeMap<SpendAddress, (Instant, NanoTokens)>>>,
    encryption_sk: Option<SecretKey>,
}

#[derive(Clone, Default)]
struct BetaTracking {
    forwarded_payments: ForwardedPayments,
    processed_spends: u64,
    total_accumulated_utxo: u64,
    total_on_track_utxo: u64,
    total_royalties: BTreeMap<SpendAddress, u64>,
}

/// Map of Discord usernames to their tracked forwarded payments
type ForwardedPayments = BTreeMap<String, BTreeSet<(SpendAddress, NanoTokens)>>;

type UtxoStatus = (
    BTreeMap<SpendAddress, (Instant, NanoTokens)>,
    BTreeMap<SpendAddress, (Instant, NanoTokens)>,
);

#[derive(Clone, Serialize, Deserialize)]
struct SpendJsonResponse {
    address: String,
    fault: String,
    spend_type: String,
    spends: Vec<SignedSpend>,
}

impl SpendDagDb {
    /// Create a new SpendDagDb
    /// If a local spend DAG file is found, it will be loaded
    /// Else a new DAG will be created containing only Genesis
    pub async fn new(
        path: PathBuf,
        client: Client,
        encryption_sk: Option<SecretKey>,
    ) -> Result<Self> {
        if !path.exists() {
            debug!("Creating directory {path:?}...");
            std::fs::create_dir_all(&path)?;
        }
        let dag_path = path.join(SPEND_DAG_FILENAME);
        info!("Loading DAG from {dag_path:?}...");
        let dag = match SpendDag::load_from_file(&dag_path) {
            Ok(d) => {
                info!("Found a local spend DAG file");
                d
            }
            Err(_) => {
                info!("Found no local spend DAG file, starting from Genesis");
                client.new_dag_with_genesis_only().await?
            }
        };

        Ok(Self {
            client: Some(client),
            path,
            dag: Arc::new(RwLock::new(dag)),
            beta_tracking: Arc::new(RwLock::new(Default::default())),
            beta_participants: Arc::new(RwLock::new(BTreeMap::new())),
            utxo_addresses: Arc::new(RwLock::new(BTreeMap::new())),
            encryption_sk,
        })
    }

    // Check if the DAG has an encryption secret key set
    pub fn has_encryption_sk(&self) -> bool {
        self.encryption_sk.is_some()
    }

    /// Create a new SpendDagDb from a local file and no network connection
    pub fn offline(dag_path: PathBuf, encryption_sk: Option<SecretKey>) -> Result<Self> {
        let path = dag_path
            .parent()
            .ok_or_else(|| eyre!("Failed to get parent path"))?
            .to_path_buf();
        let dag = SpendDag::load_from_file(&dag_path)?;
        Ok(Self {
            client: None,
            path,
            dag: Arc::new(RwLock::new(dag)),
            beta_tracking: Arc::new(RwLock::new(Default::default())),
            beta_participants: Arc::new(RwLock::new(BTreeMap::new())),
            utxo_addresses: Arc::new(RwLock::new(BTreeMap::new())),
            encryption_sk,
        })
    }

    /// Get info about a single spend in JSON format
    pub async fn spend_json(&self, address: SpendAddress) -> Result<String> {
        let dag_ref = Arc::clone(&self.dag);
        let r_handle = dag_ref.read().await;
        let spend = r_handle.get_spend(&address);
        let faults = r_handle.get_spend_faults(&address);
        let fault = if faults.is_empty() {
            "none".to_string()
        } else {
            faults.iter().fold(String::new(), |mut output, b| {
                let _ = write!(output, "{b:?}; ");
                output
            })
        };

        let (spend_type, spends) = match spend {
            SpendDagGet::SpendNotFound => ("SpendNotFound", vec![]),
            SpendDagGet::Utxo => ("Utxo", vec![]),
            SpendDagGet::DoubleSpend(vs) => ("DoubleSpend", vs),
            SpendDagGet::Spend(s) => ("Spend", vec![*s]),
        };

        let spend_json = SpendJsonResponse {
            address: address.to_hex(),
            fault,
            spend_type: spend_type.to_string(),
            spends,
        };

        let json = serde_json::to_string_pretty(&spend_json)?;
        Ok(json)
    }

    /// Dump DAG to disk
    pub async fn dump(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)?;
        let dag_path = self.path.join(SPEND_DAG_FILENAME);
        let dag_ref = Arc::clone(&self.dag);
        let r_handle = dag_ref.read().await;
        r_handle.dump_to_file(dag_path)?;
        Ok(())
    }

    /// Load current DAG svg from disk
    #[cfg(feature = "svg-dag")]
    pub fn load_svg(&self) -> Result<Vec<u8>> {
        let svg_path = self.path.join(SPEND_DAG_SVG_FILENAME);
        let svg = std::fs::read(&svg_path)
            .context(format!("Could not load svg from path: {svg_path:?}"))?;
        Ok(svg)
    }

    /// Dump current DAG as svg to disk
    #[cfg(feature = "svg-dag")]
    pub async fn dump_dag_svg(&self) -> Result<()> {
        info!("Dumping DAG to svg...");
        std::fs::create_dir_all(&self.path)?;
        let svg_path = self.path.join(SPEND_DAG_SVG_FILENAME);
        let dag_ref = Arc::clone(&self.dag);
        let r_handle = dag_ref.read().await;
        let svg = dag_to_svg(&r_handle)?;
        std::fs::write(svg_path.clone(), svg)?;
        info!("Successfully dumped DAG to {svg_path:?}...");
        Ok(())
    }

    /// Update DAG from Network continuously
    pub async fn continuous_background_update(self, storage_dir: PathBuf) -> Result<()> {
        let client = if let Some(client) = &self.client {
            client.clone()
        } else {
            bail!("Cannot update DAG in offline mode")
        };

        // init utxos to fetch
        let start_dag = { Arc::clone(&self.dag).read().await.clone() };
        {
            let mut utxo_addresses = self.utxo_addresses.write().await;
            for addr in start_dag.get_utxos().iter() {
                let _ = utxo_addresses.insert(*addr, (Instant::now(), NanoTokens::zero()));
            }
        }

        // beta rewards processing
        let self_clone = self.clone();
        let spend_processing = if let Some(sk) = self.encryption_sk.clone() {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<(SignedSpend, u64, bool)>(
                SPENDS_PROCESSING_BUFFER_SIZE,
            );
            tokio::spawn(async move {
                let mut double_spends = BTreeSet::new();
                let mut detected_spends = BTreeSet::new();

                while let Some((spend, utxos_for_further_track, is_double_spend)) = rx.recv().await
                {
                    let content_hash = spend.spend.hash();

                    if detected_spends.insert(content_hash) {
                        let hex_content_hash = content_hash.to_hex();
                        let addr_hex = spend.address().to_hex();
                        let file_name = format!("{addr_hex}_{hex_content_hash}");
                        let spend_copy = spend.clone();
                        let file_path = storage_dir.join(&file_name);

                        tokio::spawn(async move {
                            let bytes = spend_copy.to_bytes();
                            match std::fs::write(&file_path, bytes) {
                                Ok(_) => {
                                    info!("Wrote spend {file_name} to disk!");
                                }
                                Err(err) => {
                                    error!("Error writing spend {file_name}, error: {err:?}");
                                }
                            }
                        });
                    }

                    if is_double_spend {
                        self_clone
                            .beta_background_process_double_spend(
                                spend.clone(),
                                &sk,
                                utxos_for_further_track,
                            )
                            .await;

                        // For double_spend, only credit the owner first time
                        // The performance track only count the received spend & utxos once.
                        if double_spends.insert(spend.address()) {
                            self_clone
                                .beta_background_process_spend(spend, &sk, utxos_for_further_track)
                                .await;
                        }
                    } else {
                        self_clone
                            .beta_background_process_spend(spend, &sk, utxos_for_further_track)
                            .await;
                    }
                }
            });
            Some(tx)
        } else {
            warn!("Foundation secret key not set! Beta rewards will not be processed.");
            None
        };

        let mut addrs_to_get = BTreeSet::new();

        loop {
            // get expired utxos for re-attempt fetch
            {
                let now = Instant::now();
                let mut utxo_addresses = self.utxo_addresses.write().await;
                let mut utxos_to_fetch = BTreeSet::new();
                utxo_addresses.retain(|address, (time_stamp, amount)| {
                    let not_expired = *time_stamp > now;
                    if !not_expired {
                        if amount.as_nano() > 100000 {
                            info!("re-attempt fetching big-UTXO {address:?} with {amount}");
                        }
                        let _ = utxos_to_fetch.insert((*address, *amount));
                    }
                    not_expired
                });
                addrs_to_get.extend(utxos_to_fetch);
            }

            if addrs_to_get.is_empty() {
                debug!(
                    "Sleeping for {:?} until next re-attempt...",
                    *DAG_CRAWL_REST_INTERVAL
                );
                tokio::time::sleep(*DAG_CRAWL_REST_INTERVAL).await;
                continue;
            }

            if cfg!(feature = "dag-collection") {
                let new_utxos = self
                    .crawl_and_generate_local_dag(
                        addrs_to_get.iter().map(|(addr, _amount)| *addr).collect(),
                        spend_processing.clone(),
                        client.clone(),
                    )
                    .await;
                addrs_to_get.clear();

                let mut utxo_addresses = self.utxo_addresses.write().await;
                utxo_addresses.extend(new_utxos.into_iter().map(|a| {
                    (
                        a,
                        (
                            Instant::now() + *UTXO_REATTEMPT_INTERVAL,
                            NanoTokens::zero(),
                        ),
                    )
                }));
            } else if let Some(sender) = spend_processing.clone() {
                if let Ok(reattempt_addrs) = client
                    .crawl_to_next_utxos(
                        &mut addrs_to_get,
                        sender.clone(),
                        *UTXO_REATTEMPT_INTERVAL,
                    )
                    .await
                {
                    let mut utxo_addresses = self.utxo_addresses.write().await;
                    utxo_addresses.extend(reattempt_addrs);
                }
            } else {
                panic!("There is no point in running the auditor if we are not collecting the DAG or collecting data through crawling. Please enable the `dag-collection` feature or provide beta program related arguments.");
            };
        }
    }

    async fn crawl_and_generate_local_dag(
        &self,
        from: BTreeSet<SpendAddress>,
        spend_processing: Option<Sender<(SignedSpend, u64, bool)>>,
        client: Client,
    ) -> BTreeSet<SpendAddress> {
        // get a copy of the current DAG
        let mut dag = { Arc::clone(&self.dag).read().await.clone() };

        // update it
        client
            .spend_dag_continue_from(&mut dag, from, spend_processing.clone(), true)
            .await;
        let new_utxos = dag.get_utxos();

        // write updates to local DAG and save to disk
        let mut dag_w_handle = self.dag.write().await;
        *dag_w_handle = dag;
        std::mem::drop(dag_w_handle);
        if let Err(e) = self.dump().await {
            error!("Failed to dump DAG: {e}");
        }

        // update and save svg to file in a background thread so we don't block
        #[cfg(feature = "svg-dag")]
        {
            let self_clone = self.clone();
            tokio::spawn(async move {
                if let Err(e) = self_clone.dump_dag_svg().await {
                    error!("Failed to dump DAG svg: {e}");
                }
            });
        }

        new_utxos
    }

    /// Process each spend and update beta rewards data
    pub async fn beta_background_process_spend(
        &self,
        spend: SignedSpend,
        sk: &SecretKey,
        utxos_for_further_track: u64,
    ) {
        let mut beta_tracking = self.beta_tracking.write().await;
        beta_tracking.processed_spends += 1;
        beta_tracking.total_accumulated_utxo += spend.spend.spent_tx.outputs.len() as u64;
        beta_tracking.total_on_track_utxo += utxos_for_further_track;

        // Collect royalties
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
        let mut royalties = BTreeMap::new();
        for output in spend.spend.spent_tx.outputs.iter() {
            if default_royalty_pubkeys.contains(&output.unique_pubkey)
                || royalty_pubkeys.contains(&output.unique_pubkey)
            {
                let _ = royalties.insert(
                    SpendAddress::from_unique_pubkey(&output.unique_pubkey),
                    output.amount.as_nano(),
                );
            }
        }

        if royalties.len() > (spend.spend.spent_tx.outputs.len() - 1) / 2 {
            eprintln!(
                "Spend: {:?} has incorrect royalty of {}, with amount {} with reason {:?}",
                spend.spend.unique_pubkey,
                royalties.len(),
                spend.spend.amount.as_nano(),
                spend.spend.reason
            );
            eprintln!(
                "Incorrect royalty spend has {} royalties, {:?} - {:?}",
                spend.spend.network_royalties.len(),
                spend.spend.spent_tx.inputs,
                spend.spend.spent_tx.outputs
            );
            warn!(
                "Spend: {:?} has incorrect royalty of {}, with amount {} with reason {:?}",
                spend.spend.unique_pubkey,
                royalties.len(),
                spend.spend.amount.as_nano(),
                spend.spend.reason
            );
            warn!(
                "Incorrect royalty spend has {} royalties, {:?} - {:?}",
                spend.spend.network_royalties.len(),
                spend.spend.spent_tx.inputs,
                spend.spend.spent_tx.outputs
            );
        }
        beta_tracking.total_royalties.extend(royalties);

        let addr = spend.address();
        let amount = spend.spend.amount;

        // check for beta rewards reason
        let user_name_hash = match spend.reason().get_sender_hash(sk) {
            Some(n) => n,
            None => {
                if let Some(default_user_name_hash) =
                    spend.reason().get_sender_hash(&DEFAULT_PAYMENT_FORWARD_SK)
                {
                    warn!("With default key, got forwarded reward of {amount} at {addr:?}");
                    println!("With default key, got forwarded reward of {amount} at {addr:?}");
                    default_user_name_hash
                } else {
                    warn!(
                        "Can't descrypt discord_id from {addr:?} with compile key nor default key"
                    );
                    println!(
                        "Can't descrypt discord_id from {addr:?} with compile key nor default key"
                    );
                    return;
                }
            }
        };

        // add to local rewards
        let beta_participants_read = self.beta_participants.read().await;

        if let Some(user_name) = beta_participants_read.get(&user_name_hash) {
            trace!("Got forwarded reward {amount} from {user_name} of {amount} at {addr:?}");
            beta_tracking
                .forwarded_payments
                .entry(user_name.to_owned())
                .or_default()
                .insert((addr, amount));
        } else {
            // check with default key
            if let Some(default_user_name_hash) =
                spend.reason().get_sender_hash(&DEFAULT_PAYMENT_FORWARD_SK)
            {
                if let Some(user_name) = beta_participants_read.get(&default_user_name_hash) {
                    warn!("With default key, got forwarded reward from {user_name} of {amount} at {addr:?}");
                    println!("With default key, got forwarded reward from {user_name} of {amount} at {addr:?}");
                    beta_tracking
                        .forwarded_payments
                        .entry(user_name.to_owned())
                        .or_default()
                        .insert((addr, amount));
                    return;
                }
            }

            warn!("Found a forwarded reward {amount} for an unknown participant at {addr:?}: {user_name_hash:?}");
            beta_tracking
                .forwarded_payments
                .entry(format!("unknown participant: {user_name_hash:?}"))
                .or_default()
                .insert((addr, amount));
        }
    }

    async fn beta_background_process_double_spend(
        &self,
        spend: SignedSpend,
        sk: &SecretKey,
        _utxos_for_further_track: u64,
    ) {
        let user_name_hash = match spend.reason().get_sender_hash(sk) {
            Some(n) => n,
            None => {
                return;
            }
        };

        let addr = spend.address();

        let beta_participants_read = self.beta_participants.read().await;

        if let Some(user_name) = beta_participants_read.get(&user_name_hash) {
            println!("Found double spend from {user_name} at {addr:?}");
        } else {
            if let Some(default_user_name_hash) =
                spend.reason().get_sender_hash(&DEFAULT_PAYMENT_FORWARD_SK)
            {
                if let Some(user_name) = beta_participants_read.get(&default_user_name_hash) {
                    println!("Found double spend from {user_name} at {addr:?} using default key");
                    return;
                }
            }

            println!(
                "Found double spend from an unknown participant {user_name_hash:?} at {addr:?}"
            );
        }
    }

    /// Merge a SpendDag into the current DAG
    /// This can be used to enrich our DAG with a DAG from another node to avoid costly computations
    /// Make sure to verify the other DAG is trustworthy before calling this function to merge it in
    pub async fn merge(&mut self, other: SpendDag) -> Result<()> {
        let mut w_handle = self.dag.write().await;
        w_handle.merge(other, true)?;
        Ok(())
    }

    /// Returns the current state of the beta program in JSON format,
    /// including total rewards for each participant.
    /// Also returns the current tracking performance in readable format.
    pub(crate) async fn beta_program_json(&self) -> Result<(String, String)> {
        let r_handle = Arc::clone(&self.beta_tracking);
        let beta_tracking = r_handle.read().await;
        let r_utxo_handler = Arc::clone(&self.utxo_addresses);
        let utxo_addresses = r_utxo_handler.read().await;
        let mut rewards_output = vec![];
        let mut total_hits = 0_u64;
        let mut total_amount = 0_u64;
        for (participant, rewards) in beta_tracking.forwarded_payments.iter() {
            total_hits += rewards.len() as u64;
            let total_rewards = rewards
                .iter()
                .map(|(_, amount)| amount.as_nano())
                .sum::<u64>();
            total_amount += total_rewards;

            rewards_output.push((participant.clone(), total_rewards));
        }
        let json = serde_json::to_string_pretty(&rewards_output)?;

        let mut tracking_performance = format!("processed_spends: {}\ntotal_accumulated_utxo:{}\ntotal_on_track_utxo:{}\nskipped_utxo:{}\nrepeated_utxo:{}\ntotal_hits:{}\ntotal_amount:{}",
            beta_tracking.processed_spends, beta_tracking.total_accumulated_utxo, beta_tracking.total_on_track_utxo, beta_tracking.total_accumulated_utxo - beta_tracking.total_on_track_utxo,
            utxo_addresses.len(), total_hits, total_amount
            );

        tracking_performance = format!(
            "{tracking_performance}\ntotal_royalties hits: {}",
            beta_tracking.total_royalties.len()
        );
        let total_royalties = beta_tracking.total_royalties.values().sum::<u64>();
        tracking_performance =
            format!("{tracking_performance}\ntotal_royalties amount: {total_royalties}");

        // UTXO amount that greater than 100000 nanos shall be considered as `change`
        // which indicates the `wallet balance`
        let (big_utxos, small_utxos): UtxoStatus = utxo_addresses
            .iter()
            .partition(|(_address, (_time_stamp, amount))| amount.as_nano() > 100000);

        let total_big_utxo_amount = big_utxos
            .iter()
            .map(|(_addr, (_time, amount))| amount.as_nano())
            .sum::<u64>();
        tracking_performance =
            format!("{tracking_performance}\ntotal_big_utxo_amount: {total_big_utxo_amount}");

        let total_small_utxo_amount = small_utxos
            .iter()
            .map(|(_addr, (_time, amount))| amount.as_nano())
            .sum::<u64>();
        tracking_performance =
            format!("{tracking_performance}\ntotal_small_utxo_amount: {total_small_utxo_amount}");

        for (addr, (_time, amount)) in big_utxos.iter() {
            tracking_performance =
                format!("{tracking_performance}\n{addr:?}, {}", amount.as_nano());
        }
        for (addr, (_time, amount)) in small_utxos.iter() {
            tracking_performance =
                format!("{tracking_performance}\n{addr:?}, {}", amount.as_nano());
        }

        Ok((json, tracking_performance))
    }

    /// Track new beta participants. This just add the participants to the list of tracked participants.
    pub(crate) async fn track_new_beta_participants(
        &self,
        participants: BTreeSet<String>,
    ) -> Result<()> {
        let mut new_participants = vec![];
        // track new participants
        {
            let mut beta_participants = self.beta_participants.write().await;
            beta_participants.extend(participants.iter().map(|p| {
                let hash: Hash = Hash::hash(p.as_bytes());
                new_participants.push((hash, p.clone()));
                (hash, p.clone())
            }));
        }
        // initialize forwarded payments
        {
            let mut beta_tracking = self.beta_tracking.write().await;
            for (hash, p) in new_participants {
                let unkown_str = format!("unknown participant: {hash:?}");
                let mut payments = beta_tracking
                    .forwarded_payments
                    .remove(&unkown_str)
                    .unwrap_or_default();

                if let Some(existing) = beta_tracking
                    .forwarded_payments
                    .insert(p.clone(), payments.clone())
                {
                    warn!("Overwriting existing participant {p} with new participant {hash:?}");
                    payments.extend(existing);
                    let _ = beta_tracking.forwarded_payments.insert(p.clone(), payments);
                }
            }
        }
        Ok(())
    }

    /// Check if a participant is being tracked
    pub(crate) async fn is_participant_tracked(&self, discord_id: &str) -> Result<bool> {
        let beta_participants = self.beta_participants.read().await;
        debug!("Existing beta participants: {beta_participants:?}");

        debug!(
            "Adding new beta participants: {discord_id}, {:?}",
            Hash::hash(discord_id.as_bytes())
        );
        Ok(beta_participants.contains_key(&Hash::hash(discord_id.as_bytes())))
    }

    /// Backup beta rewards to a timestamped json file
    pub(crate) async fn backup_rewards(&self) -> Result<()> {
        info!("Beta rewards backup requested");
        let (json, tracking_performance) = match self.beta_program_json().await {
            Ok(r) => r,
            Err(e) => bail!("Failed to get beta rewards json: {e}"),
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| format!("{t:?}"))
            .unwrap_or_default();
        let backup_file = self.path.join(format!("beta_rewards_{timestamp}.json"));
        info!("Writing rewards backup to {backup_file:?}");
        std::fs::write(backup_file, json)
            .map_err(|e| eyre!("Could not write rewards backup to disk: {e}"))?;

        let backup_file = self
            .path
            .join(format!("tracking_performance_{timestamp}.log"));
        info!("Writing tracking performance to {backup_file:?}");
        std::fs::write(backup_file, tracking_performance)
            .map_err(|e| eyre!("Could not write tracking performance to disk: {e}"))?;

        Ok(())
    }
}

#[cfg(feature = "svg-dag")]
fn dag_to_svg(dag: &SpendDag) -> Result<Vec<u8>> {
    let dot = dag.dump_dot_format();
    let graph = parse(&dot).map_err(|err| eyre!("Failed to parse dag from dot: {err}"))?;
    let graph_svg = exec(
        graph,
        &mut PrinterContext::default(),
        vec![Format::Svg.into()],
    )
    .map_err(|e| eyre!("Failed to generate svg, is graphviz installed? dot: {e}"))?;
    let svg = quick_edit_svg(graph_svg, dag)?;
    Ok(svg)
}

// quick n dirty svg editing
// - makes spends clickable
// - spend address reveals on hover
// - marks poisoned spends as red
// - marks UTXOs and unknown ancestors as yellow
// - just pray it works on windows
#[cfg(feature = "svg-dag")]
fn quick_edit_svg(svg: Vec<u8>, dag: &SpendDag) -> Result<Vec<u8>> {
    let mut str = String::from_utf8(svg).map_err(|err| eyre!("Failed svg conversion: {err}"))?;

    let spend_addrs: Vec<_> = dag.all_spends().iter().map(|s| s.address()).collect();
    let pending_addrs = dag.get_pending_spends();
    let all_addrs = spend_addrs.iter().chain(pending_addrs.iter());

    for addr in all_addrs {
        let addr_hex = addr.to_hex().to_string();
        let is_fault = !dag.get_spend_faults(addr).is_empty();
        let is_known_but_not_gathered = matches!(dag.get_spend(addr), SpendDagGet::Utxo);
        let colour = if is_fault {
            "red"
        } else if is_known_but_not_gathered {
            "yellow"
        } else {
            "none"
        };

        let link = format!("<a xlink:href=\"/spend/{addr_hex}\">");
        let idxs = dag.get_spend_indexes(addr);
        for i in idxs {
            let title = format!("<title>{i}</title>\n<ellipse fill=\"none");
            let new_title = format!("<title>{addr_hex}</title>\n{link}\n<ellipse fill=\"{colour}");
            str = str.replace(&title, &new_title);
        }

        // close the link tag
        let end = format!("{addr:?}</text>\n</g>");
        let new_end = format!("{addr:?}</text>\n</a>\n</g>");
        str = str.replace(&end, &new_end);
    }

    Ok(str.into_bytes())
}
