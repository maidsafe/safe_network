// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::path::Path;
use std::str::FromStr;

use bls::SecretKey;
use color_eyre::eyre::bail;
use color_eyre::Result;
use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
use sn_client::transfers::{CashNoteRedemption, SpendAddress, Transfer, GENESIS_SPEND_UNIQUE_KEY};
use sn_client::{Client, SpendDag};

const SPEND_DAG_FILENAME: &str = "spend_dag";
const SPENDS_PROCESSING_BUFFER_SIZE: usize = 4096;

async fn step_by_step_spend_dag_gathering(client: &Client, mut dag: SpendDag) -> Result<SpendDag> {
    let start_time = std::time::Instant::now();
    println!("Gathering the Spend DAG, note that this might take a very long time...");
    let (tx, mut rx) = tokio::sync::mpsc::channel(SPENDS_PROCESSING_BUFFER_SIZE);
    tokio::spawn(async move {
        let mut spend_count = 0;
        let mut exponential = 64;
        while let Some(_spend) = rx.recv().await {
            spend_count += 1;
            if spend_count % exponential == 0 {
                println!("Collected {spend_count} spends...");
                exponential *= 2;
            }
        }
    });

    client
        .spend_dag_continue_from_utxos(&mut dag, Some(tx), false)
        .await;
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

/// Gather the Spend DAG from the Network and store it on disk
/// If a DAG is found on disk, it will continue from it
/// If fast_mode is true, gathers in a silent and fast way
/// else enjoy a step by step slow narrated gathering
async fn gather_spend_dag(client: &Client, root_dir: &Path, fast_mode: bool) -> Result<SpendDag> {
    let dag_path = root_dir.join(SPEND_DAG_FILENAME);
    let inital_dag = match SpendDag::load_from_file(&dag_path) {
        Ok(mut dag) => {
            println!("Found a local spend dag on disk, continuing from it...");
            if fast_mode {
                client
                    .spend_dag_continue_from_utxos(&mut dag, Default::default(), false)
                    .await;
            }
            dag
        }
        Err(err) => {
            println!("Starting from Genesis as found no local spend dag on disk...");
            info!("Starting from Genesis as failed to load spend dag from disk: {err}");
            let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_SPEND_UNIQUE_KEY);
            if fast_mode {
                client
                    .spend_dag_build_from(genesis_addr, Default::default(), true)
                    .await?
            } else {
                client.new_dag_with_genesis_only().await?
            }
        }
    };

    let dag = match fast_mode {
        true => inital_dag,
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
    let fast_mode = to_dot || royalties || foundation_sk.is_some();
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

/// Verify a spend's existance on the Network.
/// If genesis is true, verify all the way to Genesis, note that this might take A VERY LONG TIME
pub async fn verify_spend_at(
    spend_address: String,
    genesis: bool,
    client: &Client,
    root_dir: &Path,
) -> Result<()> {
    // get spend
    println!("Verifying spend's existance at: {spend_address}");
    let addr = SpendAddress::from_str(&spend_address)?;
    let spend = match client.get_spend_from_network(addr).await {
        Ok(s) => {
            println!("Confirmed spend's existance on the Network at {addr:?}");
            s
        }
        Err(err) => {
            bail!("Could not confirm spend's validity, be careful: {err}")
        }
    };

    // stop here if we don't go all the way to Genesis
    if !genesis {
        return Ok(());
    }
    println!("Verifying spend all the way to Genesis, note that this might take a while...");

    // extend DAG until spend
    let dag_path = root_dir.join(SPEND_DAG_FILENAME);
    let mut dag = match SpendDag::load_from_file(&dag_path) {
        Ok(d) => {
            println!("Found a local spend dag on disk, continuing from it, this might make things faster...");
            d
        }
        Err(err) => {
            info!("Starting verification from an empty DAG as failed to load spend dag from disk: {err}");
            let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_SPEND_UNIQUE_KEY);
            SpendDag::new(genesis_addr)
        }
    };
    info!("Extending DAG with {spend_address} {addr:?}");
    client.spend_dag_extend_until(&mut dag, addr, spend).await?;
    info!("Saving DAG locally at: {dag_path:?}");
    dag.dump_to_file(dag_path)?;

    // verify spend is not faulty
    let faults = dag.get_spend_faults(&addr);
    if faults.is_empty() {
        println!(
            "Successfully confirmed spend at {spend_address} is valid, and comes from Genesis!"
        );
    } else {
        println!("Spend at {spend_address} has {} faults", faults.len());
        println!("{faults:#?}");
    }

    Ok(())
}
