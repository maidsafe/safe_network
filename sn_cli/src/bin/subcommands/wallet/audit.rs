// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::path::Path;

use color_eyre::Result;
use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
use sn_client::transfers::{CashNoteRedemption, SpendAddress, Transfer, GENESIS_CASHNOTE};
use sn_client::{Client, SpendDag};

const SPEND_DAG_FILENAME: &str = "spend_dag";

async fn gather_spend_dag(client: &Client, root_dir: &Path) -> Result<SpendDag> {
    let dag_path = root_dir.join(SPEND_DAG_FILENAME);
    let dag = match SpendDag::load_from_file(&dag_path) {
        Ok(mut dag) => {
            println!("Starting from the loaded spend dag on disk...");
            client.spend_dag_continue_from_utxos(&mut dag, None).await?;
            dag
        }
        Err(err) => {
            println!("Starting from Genesis as found no local spend dag on disk...");
            info!("Starting from Genesis as failed to load spend dag from disk: {err}");
            let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey());
            client.spend_dag_build_from(genesis_addr, None).await?
        }
    };

    println!("Creating a local backup to disk...");
    dag.dump_to_file(dag_path)?;

    Ok(dag)
}

pub async fn audit(client: &Client, to_dot: bool, royalties: bool, root_dir: &Path) -> Result<()> {
    if to_dot {
        let dag = gather_spend_dag(client, root_dir).await?;
        println!("{}", dag.dump_dot_format());
    } else if royalties {
        let dag = gather_spend_dag(client, root_dir).await?;
        let royalties = dag.all_royalties()?;
        redeem_royalties(royalties, client, root_dir).await?;
    } else {
        //NB TODO use the above DAG to audit too
        println!("Auditing the Currency, note that this might take a very long time...");
        let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey());
        client.follow_spend(genesis_addr).await?;
    }
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
