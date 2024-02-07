// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::path::Path;

use color_eyre::Result;
use sn_client::{Client, SpendDag};
use sn_transfers::{SpendAddress, GENESIS_CASHNOTE};

const SPEND_DAG_FILENAME: &str = "spend_dag";

async fn gather_spend_dag(client: &Client, root_dir: &Path) -> Result<SpendDag> {
    let dag_path = root_dir.join(SPEND_DAG_FILENAME);
    let dag = match SpendDag::load_from_file(&dag_path) {
        Ok(mut dag) => {
            info!("Starting from the loaded spend dag on disk");
            client.spend_dag_continue_from_utxos(&mut dag).await?;
            dag
        }
        Err(err) => {
            info!("Starting from Genesis as failed to load spend dag from disk: {err}");
            let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey());
            client.spend_dag_build_from(genesis_addr).await?
        }
    };

    dag.dump_to_file(dag_path)?;

    Ok(dag)
}

pub async fn audit(client: &Client, to_dot: bool, root_dir: &Path) -> Result<()> {
    if to_dot {
        let dag = gather_spend_dag(client, root_dir).await?;
        println!("{}", dag.dump_dot_format());
    } else {
        //NB TODO use the above DAG to audit too
        println!("Auditing the Currency, note that this might take a very long time...");
        let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey());
        client.follow_spend(genesis_addr).await?;
    }
    Ok(())
}
