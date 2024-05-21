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
use serde::{Deserialize, Serialize};
use sn_client::networking::NetworkError;
use sn_client::transfers::{Hash, NanoTokens, SignedSpend, SpendAddress, GENESIS_SPEND_UNIQUE_KEY};
use sn_client::Error as ClientError;
use sn_client::{Client, SpendDag, SpendDagGet};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

pub const SPEND_DAG_FILENAME: &str = "spend_dag";
#[cfg(feature = "svg-dag")]
pub const SPEND_DAG_SVG_FILENAME: &str = "spend_dag.svg";

/// Abstraction for the Spend DAG database
/// Currently in memory, with disk backup, but should probably be a real DB at scale
#[derive(Clone)]
pub struct SpendDagDb {
    client: Option<Client>,
    path: PathBuf,
    dag: Arc<RwLock<SpendDag>>,
    forwarded_payments: Arc<RwLock<ForwardedPayments>>,
    pub(crate) beta_participants: Arc<RwLock<BTreeMap<Hash, String>>>,
    encryption_sk: Option<SecretKey>,
}

/// Map of Discord usernames to their tracked forwarded payments
type ForwardedPayments = BTreeMap<String, BTreeSet<(SpendAddress, NanoTokens)>>;

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
        let dag_path = path.join(SPEND_DAG_FILENAME);
        info!("Loading DAG from {dag_path:?}...");
        let dag = match SpendDag::load_from_file(&dag_path) {
            Ok(d) => {
                println!("Found a local spend DAG file");
                d
            }
            Err(_) => {
                println!("Found no local spend DAG file, starting from Genesis");
                new_dag_with_genesis_only(&client).await?
            }
        };

        Ok(Self {
            client: Some(client),
            path,
            dag: Arc::new(RwLock::new(dag)),
            forwarded_payments: Arc::new(RwLock::new(BTreeMap::new())),
            beta_participants: Arc::new(RwLock::new(BTreeMap::new())),
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
            forwarded_payments: Arc::new(RwLock::new(BTreeMap::new())),
            beta_participants: Arc::new(RwLock::new(BTreeMap::new())),
            encryption_sk,
        })
    }

    /// Get info about a single spend in JSON format
    pub fn spend_json(&self, address: SpendAddress) -> Result<String> {
        let dag_ref = self.dag.clone();
        let r_handle = dag_ref
            .read()
            .map_err(|e| eyre!("Failed to get read lock: {e}"))?;
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
    pub fn dump(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)?;
        let dag_path = self.path.join(SPEND_DAG_FILENAME);
        let dag_ref = self.dag.clone();
        let r_handle = dag_ref
            .read()
            .map_err(|e| eyre!("Failed to get read lock: {e}"))?;
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
    pub fn dump_dag_svg(&self) -> Result<()> {
        info!("Dumping DAG to svg...");
        std::fs::create_dir_all(&self.path)?;
        let svg_path = self.path.join(SPEND_DAG_SVG_FILENAME);
        let dag_ref = self.dag.clone();
        let r_handle = dag_ref
            .read()
            .map_err(|e| eyre!("Failed to get read lock: {e}"))?;
        let svg = dag_to_svg(&r_handle)?;
        std::fs::write(svg_path.clone(), svg)?;
        info!("Successfully dumped DAG to {svg_path:?}...");
        Ok(())
    }

    /// Update DAG from Network
    pub async fn update(&mut self) -> Result<()> {
        // read current DAG
        let mut dag = {
            self.dag
                .clone()
                .read()
                .map_err(|e| eyre!("Failed to get read lock: {e}"))?
                .clone()
        };

        // update that copy 10 generations further
        const NEXT_10_GEN: u32 = 10;
        self.client
            .clone()
            .ok_or(eyre!("Cannot update in offline mode"))?
            .spend_dag_continue_from_utxos(&mut dag, Some(NEXT_10_GEN), true)
            .await?;

        // write update to DAG
        let mut dag_w_handle = self
            .dag
            .write()
            .map_err(|e| eyre!("Failed to get write lock: {e}"))?;
        *dag_w_handle = dag;
        std::mem::drop(dag_w_handle);

        #[cfg(feature = "svg-dag")]
        {
            // update and save svg to file in a background thread so we don't block
            //
            let self_clone = self.clone();
            tokio::spawn(async move {
                if let Err(e) = self_clone.dump_dag_svg() {
                    error!("Failed to dump DAG svg: {e}");
                }
            });
        }

        // gather forwarded payments in a background thread so we don't block
        let self_clone = self.clone();
        tokio::spawn(async move {
            if let Err(e) = self_clone.gather_forwarded_payments().await {
                error!("Failed to gather forwarded payments: {e}");
            }
        });

        Ok(())
    }

    /// Merge a SpendDag into the current DAG
    /// This can be used to enrich our DAG with a DAG from another node to avoid costly computations
    /// Make sure to verify the other DAG is trustworthy before calling this function to merge it in
    pub fn merge(&mut self, other: SpendDag) -> Result<()> {
        let mut w_handle = self
            .dag
            .write()
            .map_err(|e| eyre!("Failed to get write lock: {e}"))?;
        w_handle.merge(other, true)?;
        Ok(())
    }

    /// Returns the current state of the beta program in JSON format, including total rewards for each participant
    pub(crate) fn beta_program_json(&self) -> Result<String> {
        let r_handle = self.forwarded_payments.clone();
        let beta_rewards = r_handle.read();

        let participants =
            beta_rewards.map_err(|e| eyre!("Failed to get beta rewards read lock: {e}"))?;
        let mut rewards_output = vec![];
        for (participant, rewards) in participants.iter() {
            let total_rewards = rewards
                .iter()
                .map(|(_, amount)| amount.as_nano())
                .sum::<u64>();

            rewards_output.push((participant.clone(), total_rewards));
        }
        let json = serde_json::to_string_pretty(&rewards_output)?;
        Ok(json)
    }

    /// Track new beta participants. This just add the participants to the list of tracked participants.
    pub(crate) fn track_new_beta_participants(&self, participants: Vec<String>) -> Result<()> {
        // track new participants
        {
            let mut beta_participants = self
                .beta_participants
                .write()
                .map_err(|e| eyre!("Failed to get beta participants write lock: {e}"))?;
            for p in participants.iter() {
                beta_participants.insert(Hash::hash(p.as_bytes()), p.clone());
            }
        }
        // initialize forwarded payments
        {
            let mut fwd_payments = self
                .forwarded_payments
                .write()
                .map_err(|e| eyre!("Failed to get forwarded payments write lock: {e}"))?;
            *fwd_payments = participants
                .into_iter()
                .map(|n| (n, BTreeSet::new()))
                .collect();
        }
        Ok(())
    }

    /// Initialize reward forward tracking, gathers current rewards from the DAG
    pub(crate) async fn init_reward_forward_tracking(
        &self,
        participants: Vec<String>,
    ) -> Result<()> {
        self.track_new_beta_participants(participants)?;
        self.gather_forwarded_payments().await?;
        Ok(())
    }

    // Gather forwarded payments from the DAG
    pub(crate) async fn gather_forwarded_payments(&self) -> Result<()> {
        info!("Gathering forwarded payments...");

        // make sure we have the foundation secret key
        let sk = self
            .encryption_sk
            .clone()
            .ok_or_else(|| eyre!("Foundation secret key not set"))?;

        // get spends from current DAG
        let dag = self.dag.read().map_err(|e| {
            eyre!("Failed to get dag read lock for gathering forwarded payments: {e}")
        })?;
        let all_spends = dag.all_spends();

        // find spends with payments
        let mut payments: ForwardedPayments = BTreeMap::new();
        for spend in all_spends {
            let user_name_hash = match spend.reason().get_sender_hash(&sk) {
                Some(n) => n,
                None => continue,
            };
            let addr = spend.address();
            let amount = spend.spend.amount;
            let beta_participants_read = self
                .beta_participants
                .read()
                .map_err(|e| eyre!("Failed to get payments write lock: {e}"))?;
            if let Some(user_name) = beta_participants_read.get(&user_name_hash) {
                debug!("Got forwarded reward from {user_name} of {amount} at {addr:?}");
                payments
                    .entry(user_name.to_owned())
                    .or_default()
                    .insert((addr, amount));
            } else {
                info!(
                    "Found a forwarded reward for an unknown participant at {:?}: {user_name_hash:?}",
                    spend.address()
                );
                payments
                    .entry(format!("unknown participant: {user_name_hash:?}"))
                    .or_default()
                    .insert((addr, amount));
            }
        }

        // save new payments
        let mut self_payments = self
            .forwarded_payments
            .write()
            .map_err(|e| eyre!("Failed to get payments write lock: {e}"))?;
        self_payments.extend(payments);
        info!("Done gathering forwarded payments");
        Ok(())
    }

    /// Backup beta rewards to a timestamped json file
    pub(crate) fn backup_rewards(&self) -> Result<()> {
        info!("Beta rewards backup requested");
        let json = match self.beta_program_json() {
            Ok(j) => j,
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

        Ok(())
    }
}

pub async fn new_dag_with_genesis_only(client: &Client) -> Result<SpendDag> {
    let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_SPEND_UNIQUE_KEY);
    let mut dag = SpendDag::new(genesis_addr);
    let genesis_spend = match client.get_spend_from_network(genesis_addr).await {
        Ok(s) => s,
        Err(ClientError::Network(NetworkError::DoubleSpendAttempt(spend1, spend2)))
        | Err(ClientError::DoubleSpend(_, spend1, spend2)) => {
            let addr = spend1.address();
            println!("Double spend detected at Genesis: {addr:?}");
            dag.insert(genesis_addr, *spend2);
            dag.record_faults(&dag.source())?;
            *spend1
        }
        Err(e) => return Err(eyre!("Failed to get genesis spend: {e}")),
    };
    dag.insert(genesis_addr, genesis_spend);

    Ok(dag)
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
