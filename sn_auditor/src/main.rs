// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod dag_db;
mod routes;

use bls::SecretKey;
use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use dag_db::SpendDagDb;
use sn_client::Client;
use sn_logging::{Level, LogBuilder, LogFormat, LogOutputDest};
use sn_peers_acquisition::get_peers_from_args;
use sn_peers_acquisition::PeersArgs;
use std::collections::BTreeSet;
use std::path::PathBuf;
use tiny_http::{Response, Server};

/// Backup the beta rewards in a timestamped json file
const BETA_REWARDS_BACKOUP_INTERVAL_SECS: u64 = 20 * 60;

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
    /// Visualize a local DAG file offline, does not connect to the Network
    #[clap(short, long, value_name = "dag_file")]
    offline_viewer: Option<PathBuf>,

    /// Specify the logging output destination.
    ///
    /// Valid values are "stdout", "data-dir", or a custom path.
    ///
    /// `data-dir` is the default value.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/client/logs
    ///  - macOS: $HOME/Library/Application Support/safe/client/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\client\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, value_parser = LogOutputDest::parse_from_str, verbatim_doc_comment, default_value = "data-dir")]
    log_output_dest: LogOutputDest,
    /// Specify the logging format.
    ///
    /// Valid values are "default" or "json".
    ///
    /// If the argument is not used, the default format will be applied.
    #[clap(long, value_parser = LogFormat::parse_from_str, verbatim_doc_comment)]
    log_format: Option<LogFormat>,

    /// Beta rewards program participants to track
    /// Provide a JSON file with a list of Discord usernames as argument
    #[clap(short, long, value_name = "discord_names_file")]
    beta_participants: Option<PathBuf>,

    /// Secret encryption key of the beta rewards to decypher
    /// discord usernames of the beta participants
    #[clap(short = 'k', long, value_name = "hex_secret_key")]
    beta_encryption_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let log_builder = logging_init(opt.log_output_dest, opt.log_format)?;
    let _log_handles = log_builder.initialize()?;

    let beta_participants = load_and_update_beta_participants(opt.beta_participants)?;

    let maybe_sk = if let Some(sk_str) = opt.beta_encryption_key {
        match SecretKey::from_hex(&sk_str) {
            Ok(sk) => Some(sk),
            Err(err) => panic!("Cann't parse Foundation SK from input string: {sk_str}: {err:?}",),
        }
    } else {
        None
    };
    let beta_rewards_on = maybe_sk.is_some();

    if let Some(dag_to_view) = opt.offline_viewer {
        let dag = SpendDagDb::offline(dag_to_view, maybe_sk)?;
        #[cfg(feature = "svg-dag")]
        dag.dump_dag_svg().await?;

        start_server(dag).await?;
        return Ok(());
    }

    let client = connect_to_network(opt.peers).await?;
    let dag = initialize_background_spend_dag_collection(
        client.clone(),
        opt.force_from_genesis,
        opt.clean,
        beta_participants,
        maybe_sk,
    )
    .await?;

    if beta_rewards_on {
        initialize_background_rewards_backup(dag.clone());
    }

    start_server(dag).await
}

fn logging_init(
    log_output_dest: LogOutputDest,
    log_format: Option<LogFormat>,
) -> Result<LogBuilder> {
    color_eyre::install()?;
    let logging_targets = vec![
        ("sn_auditor".to_string(), Level::TRACE),
        ("sn_client".to_string(), Level::DEBUG),
        ("sn_transfers".to_string(), Level::TRACE),
        ("sn_logging".to_string(), Level::INFO),
        ("sn_peers_acquisition".to_string(), Level::INFO),
        ("sn_protocol".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::WARN),
    ];
    let mut log_builder = LogBuilder::new(logging_targets);
    log_builder.output_dest(log_output_dest);
    log_builder.format(log_format.unwrap_or(LogFormat::Default));
    Ok(log_builder)
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
    let client = Client::new(SecretKey::random(), bootstrap_peers, None, None)
        .await
        .map_err(|err| eyre!("Failed to connect to the network: {err}"))?;

    println!("Connected to the network");
    Ok(client)
}

/// Regularly backup the rewards in a timestamped json file
fn initialize_background_rewards_backup(dag: SpendDagDb) {
    tokio::spawn(async move {
        loop {
            trace!(
                "Sleeping for {BETA_REWARDS_BACKOUP_INTERVAL_SECS} seconds before next backup..."
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(
                BETA_REWARDS_BACKOUP_INTERVAL_SECS,
            ))
            .await;
            println!("Backing up beta rewards...");

            if let Err(e) = dag.backup_rewards().await {
                eprintln!("Failed to backup beta rewards: {e}");
            }
        }
    });
}

/// Get DAG from disk or initialize it if it doesn't exist
/// Spawn a background thread to update the DAG in the background
/// Return a handle to the DAG
async fn initialize_background_spend_dag_collection(
    client: Client,
    force_from_genesis: bool,
    clean: bool,
    beta_participants: BTreeSet<String>,
    foundation_sk: Option<SecretKey>,
) -> Result<SpendDagDb> {
    println!("Initialize spend dag...");
    let path = get_auditor_data_dir_path()?;

    // clean the local spend DAG if requested
    if clean {
        println!("Cleaning local spend DAG...");
        let dag_file = path.join(dag_db::SPEND_DAG_FILENAME);
        let _ = std::fs::remove_file(dag_file).map_err(|e| eprintln!("Cleanup interrupted: {e}"));
    }

    // initialize the DAG
    let dag = dag_db::SpendDagDb::new(path.clone(), client.clone(), foundation_sk)
        .await
        .map_err(|e| eyre!("Could not create SpendDag Db: {e}"))?;

    // optional force restart from genesis and merge into our current DAG
    if force_from_genesis {
        println!("Forcing DAG to be updated from genesis...");
        let mut d = dag.clone();
        let mut genesis_dag = client
            .new_dag_with_genesis_only()
            .await
            .map_err(|e| eyre!("Could not create new DAG from genesis: {e}"))?;
        tokio::spawn(async move {
            client
                .spend_dag_continue_from_utxos(&mut genesis_dag, None, true)
                .await;
            let _ = d
                .merge(genesis_dag)
                .await
                .map_err(|e| eprintln!("Failed to merge from genesis DAG into our DAG: {e}"));
        });
    }

    // initialize svg
    #[cfg(feature = "svg-dag")]
    dag.dump_dag_svg().await?;

    // initialize beta rewards program tracking
    if !beta_participants.is_empty() {
        if !dag.has_encryption_sk() {
            panic!("Foundation SK required to initialize beta rewards program");
        };

        println!("Initializing beta rewards program tracking...");
        if let Err(e) = dag.track_new_beta_participants(beta_participants).await {
            eprintln!("Could not initialize beta rewards: {e}");
            return Err(e);
        }
    }

    // background thread to update DAG
    println!("Starting background DAG collection thread...");
    let d = dag.clone();
    tokio::spawn(async move {
        let _ = d
            .continuous_background_update()
            .await
            .map_err(|e| eprintln!("Failed to update DAG in background thread: {e}"));
    });

    Ok(dag)
}

async fn start_server(dag: SpendDagDb) -> Result<()> {
    let server = Server::http("0.0.0.0:4242").expect("Failed to start server");
    println!("Starting dag-query server listening on port 4242...");
    for request in server.incoming_requests() {
        println!(
            "Received request! method: {:?}, url: {:?}",
            request.method(),
            request.url(),
        );

        // Dispatch the request to the appropriate handler
        let response = match request.url() {
            "/" => routes::spend_dag_svg(&dag),
            s if s.starts_with("/spend/") => routes::spend(&dag, &request).await,
            s if s.starts_with("/add-participant/") => {
                routes::add_participant(&dag, &request).await
            }
            "/beta-rewards" => routes::beta_rewards(&dag).await,
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

// get the data dir path for auditor
fn get_auditor_data_dir_path() -> Result<PathBuf> {
    let path = dirs_next::data_dir()
        .ok_or(eyre!("Could not obtain data directory path"))?
        .join("safe")
        .join("auditor");

    Ok(path)
}

fn load_and_update_beta_participants(
    provided_participants_file: Option<PathBuf>,
) -> Result<BTreeSet<String>> {
    let mut beta_participants = if let Some(participants_file) = provided_participants_file {
        let raw_data = std::fs::read_to_string(&participants_file)?;
        // instead of serde_json, just use a line separated file
        let discord_names = raw_data
            .lines()
            .map(|line| line.trim().to_string())
            .collect::<Vec<String>>();
        println!(
            "Tracking beta rewards for the {} discord usernames provided in {:?}",
            discord_names.len(),
            participants_file
        );
        discord_names
    } else {
        vec![]
    };
    // restore beta participants from local cached copy
    let local_participants_file =
        get_auditor_data_dir_path()?.join(dag_db::BETA_PARTICIPANTS_FILENAME);
    if local_participants_file.exists() {
        let raw_data = std::fs::read_to_string(&local_participants_file)?;
        let discord_names = raw_data
            .lines()
            .map(|line| line.trim().to_string())
            .collect::<Vec<String>>();
        println!(
            "Restoring beta rewards for the {} discord usernames from {:?}",
            discord_names.len(),
            local_participants_file
        );
        beta_participants.extend(discord_names);
    }
    // write the beta participants to disk
    let _ = std::fs::write(local_participants_file, beta_participants.join("\n"))
        .map_err(|e| eprintln!("Failed to write beta participants to disk: {e}"));

    Ok(beta_participants.into_iter().collect())
}
