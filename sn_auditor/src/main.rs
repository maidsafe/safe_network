// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod dag_db;
mod routes;

use dag_db::SpendDagDb;

use bls::SecretKey;
use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use sn_client::Client;
use sn_peers_acquisition::get_peers_from_args;
use sn_peers_acquisition::PeersArgs;
use tiny_http::{Response, Server};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
    #[command(flatten)]
    peers: PeersArgs,
    /// Force the spend DAG to be updated from genesis
    #[clap(short, long)]
    force_from_genesis: bool,
    /// Clear the local spend DAG and start from scratch
    #[clap(short, long)]
    clean: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let opt = Opt::parse();
    let client = connect_to_network(opt.peers).await?;
    let dag = initialize_background_spend_dag_collection(
        client.clone(),
        opt.force_from_genesis,
        opt.clean,
    )
    .await?;
    start_server(dag).await
}

async fn connect_to_network(peers: PeersArgs) -> Result<Client> {
    let bootstrap_peers = get_peers_from_args(peers).await?;
    println!(
        "Connecting to the network with {} bootstrap peers",
        bootstrap_peers.len(),
    );
    let bootstrap_peers = if bootstrap_peers.is_empty() {
        // empty vec is returned if `local-discovery` flag is provided
        None
    } else {
        Some(bootstrap_peers)
    };
    let client = Client::new(SecretKey::random(), bootstrap_peers, false, None, None)
        .await
        .map_err(|err| eyre!("Failed to connect to the network: {err}"))?;

    Ok(client)
}

/// Get DAG from disk or initialize it if it doesn't exist
/// Spawn a background thread to update the DAG in the background
/// Return a handle to the DAG
async fn initialize_background_spend_dag_collection(
    client: Client,
    force_from_genesis: bool,
    clean: bool,
) -> Result<SpendDagDb> {
    println!("Gather Spend DAG...");
    let path = dirs_next::data_dir()
        .ok_or(eyre!("Could not obtain data directory path"))?
        .join("safe")
        .join("auditor");

    // clean the local spend DAG if requested
    if clean {
        println!("Cleaning local spend DAG...");
        let dag_file = path.join(dag_db::SPEND_DAG_FILENAME);
        let _ = std::fs::remove_file(dag_file).map_err(|e| eprintln!("Cleanup interrupted: {e}"));
    }

    // initialize the DAG
    let dag = dag_db::SpendDagDb::new(path.clone(), client.clone())
        .await
        .map_err(|e| eyre!("Could not create SpendDag Db: {e}"))?;

    // optional force restart from genesis and merge into our current DAG
    if force_from_genesis {
        println!("Forcing DAG to be updated from genesis...");
        let mut d = dag.clone();
        let mut genesis_dag = dag_db::new_dag_with_genesis_only(&client)
            .await
            .map_err(|e| eyre!("Could not create new DAG from genesis: {e}"))?;
        tokio::spawn(async move {
            let _ = client
                .spend_dag_continue_from_utxos(&mut genesis_dag)
                .await
                .map_err(|e| eprintln!("Could not update DAG from genesis: {e}"));
            let _ = d
                .merge(genesis_dag)
                .map_err(|e| eprintln!("Failed to merge from genesis DAG into our DAG: {e}"));
        });
    }

    // background thread to update DAG
    println!("Starting background DAG collection thread...");
    let mut d = dag.clone();
    tokio::spawn(async move {
        loop {
            println!("Updating DAG...");
            let _ = d
                .update()
                .await
                .map_err(|e| eprintln!("Could not update DAG: {e}"));
            let _ = d
                .dump()
                .map_err(|e| eprintln!("Could not dump DAG to disk: {e}"));
            println!("Sleeping for 60 seconds...");
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    Ok(dag)
}

async fn start_server(dag: SpendDagDb) -> Result<()> {
    let server = Server::http("0.0.0.0:4242").expect("Failed to start server");
    println!("Starting http server listening on port 4242...");
    for request in server.incoming_requests() {
        println!(
            "Received request! method: {:?}, url: {:?}",
            request.method(),
            request.url(),
        );

        // Dispatch the request to the appropriate handler
        let response = match request.url() {
            "/" => routes::spend_dag_svg(&dag),
            s if s.starts_with("/spend/") => routes::spend(&dag, &request),
            _ => routes::not_found(),
        };

        // Send a response to the client
        match response {
            Ok(res) => {
                let _ = request
                    .respond(res)
                    .map_err(|err| eprintln!("Failed to send response: {err}"));
            }
            Err(e) => {
                eprint!("Sending error to client: {e}");
                let res = Response::from_string(format!("Error: {e}")).with_status_code(500);
                let _ = request
                    .respond(res)
                    .map_err(|err| eprintln!("Failed to send error response: {err}"));
            }
        }
    }
    Ok(())
}
