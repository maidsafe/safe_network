// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::BTreeSet;
use std::path::Path;

use bls::SecretKey;
use color_eyre::Result;
use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
use sn_client::transfers::{CashNoteRedemption, SpendAddress, Transfer, GENESIS_SPEND_UNIQUE_KEY};
use sn_client::{Client, SpendDag};

const SPEND_DAG_FILENAME: &str = "spend_dag";

async fn step_by_step_spend_dag_gathering(client: &Client, mut dag: SpendDag) -> Result<SpendDag> {
    let verify_after = false;
    let start_time = std::time::Instant::now();
    let mut depth_exponential = 1;
    let mut current_utxos = dag.get_utxos();
    let mut last_utxos = BTreeSet::new();

    println!("Gathering the Spend DAG, note that this might take a very long time...");
    while last_utxos != current_utxos {
        let unexplored_utxos = current_utxos.difference(&last_utxos).cloned().collect();
        last_utxos = std::mem::take(&mut current_utxos);

        client
            .spend_dag_continue_from(
                &mut dag,
                unexplored_utxos,
                Some(depth_exponential),
                verify_after,
            )
            .await?;

        depth_exponential += depth_exponential;
        current_utxos = dag.get_utxos();
        let dag_size = dag.all_spends().len();
        println!(
            "Depth {depth_exponential}: the DAG now has {dag_size} spends and {} UTXOs",
            current_utxos.len()
        );
    }
    println!("Done gathering the Spend DAG in {:?}", start_time.elapsed());

    // verify the DAG
    if let Err(e) = dag.record_faults(&dag.source()) {
        println!("DAG verification failed: {e}");
    } else {
        let faults_len = dag.faults().len();
        println!("DAG verification successful, identified {faults_len} faults.",);
        if faults_len > 0 {
            println!("Logging identified faults: {:#?}", dag.faults());
        }
    }
    Ok(dag)
}

async fn gather_spend_dag(client: &Client, root_dir: &Path, fast_mode: bool) -> Result<SpendDag> {
    let dag_path = root_dir.join(SPEND_DAG_FILENAME);
    let mut inital_dag = match SpendDag::load_from_file(&dag_path) {
        Ok(dag) => {
            println!("Found a local spend dag on disk, continuing from it...");
            dag
        }
        Err(err) => {
            println!("Starting from Genesis as found no local spend dag on disk...");
            info!("Starting from Genesis as failed to load spend dag from disk: {err}");
            let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_SPEND_UNIQUE_KEY);
            client
                .spend_dag_build_from(genesis_addr, Some(1), true)
                .await?
        }
    };

    let dag = match fast_mode {
        // fast but silent DAG collection
        true => {
            client
                .spend_dag_continue_from_utxos(&mut inital_dag, None, false)
                .await?;
            inital_dag
        }
        // slow but step by step narrated DAG collection
        false => step_by_step_spend_dag_gathering(client, inital_dag).await?,
    };

    println!("Saving DAG to disk at: {dag_path:?}");
    dag.dump_to_file(dag_path)?;

    Ok(dag)
}

pub async fn audit(
    client: &Client,
    to_dot: bool,
    royalties: bool,
    root_dir: &Path,
    foundation_sk: Option<SecretKey>,
) -> Result<()> {
    let fast_mode = to_dot || royalties;
    let dag = gather_spend_dag(client, root_dir, fast_mode).await?;

    if to_dot {
        println!("==========================   spends DAG digraph   ==========================");
        println!("{}", dag.dump_dot_format());
    }
    if let Some(sk) = foundation_sk {
        println!(
            "==========================   payment forward statistics  =========================="
        );
        println!("{}", dag.dump_payment_forward_statistics(&sk));
    }
    if royalties {
        let royalties = dag.all_royalties()?;
        redeem_royalties(royalties, client, root_dir).await?;
    }

    println!("Audit completed successfully.");
    Ok(())
}

/// Redeem royalties from the Network and deposit them into the wallet
/// Only works if the wallet has the private key for the royalties
async fn redeem_royalties(
    royalties: Vec<CashNoteRedemption>,
    client: &Client,
    root_dir: &Path,
) -> Result<()> {
    if royalties.is_empty() {
        println!("No royalties found to redeem.");
        return Ok(());
    } else {
        println!("Found {} royalties.", royalties.len());
    }

    let mut wallet = load_account_wallet_or_create_with_mnemonic(root_dir, None)?;

    // batch royalties per 100
    let mut batch = Vec::new();
    for (i, royalty) in royalties.iter().enumerate() {
        batch.push(royalty.clone());
        if i % 100 == 0 {
            println!(
                "Attempting to redeem {} royalties from the Network...",
                batch.len()
            );
            let transfer = Transfer::NetworkRoyalties(batch.clone());
            batch.clear();
            println!("Current balance: {}", wallet.balance());
            let cashnotes = client.receive(&transfer, &wallet).await?;
            wallet.deposit_and_store_to_disk(&cashnotes)?;
            println!("Successfully redeemed royalties from the Network.");
            println!("Current balance: {}", wallet.balance());
        }
    }
    Ok(())
}
