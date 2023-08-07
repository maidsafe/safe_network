// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use common::{get_client_and_wallet, get_funded_wallet, get_wallet};

use safenode_proto::{safe_node_client::SafeNodeClient, NodeInfoRequest, RestartRequest};

use assert_fs::TempDir;
use bytes::Bytes;
use eyre::{bail, eyre, Result};
use rand::{rngs::OsRng, Rng};
use sn_client::{Client, Error, Files, WalletClient};
use sn_dbc::{Dbc, MainKey, Token};
use sn_logging::{init_logging, LogFormat, LogOutputDest};
use sn_protocol::{
    storage::{ChunkAddress, DbcAddress, RegisterAddress},
    NetworkAddress,
};
use sn_transfers::wallet::LocalWallet;

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{fs::remove_dir_all, sync::RwLock, time::sleep};
use tonic::Request;
use tracing::trace;
use tracing_core::Level;
use xor_name::XorName;

// this includes code generated from .proto files
#[allow(unused_qualifications, unreachable_pub, clippy::unwrap_used)]
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

const NODE_COUNT: u32 = 25;

const EXTRA_CHURN_COUNT: u32 = 5;
const CHURN_CYCLES: u32 = 1;
const CHUNK_CREATION_RATIO_TO_CHURN: u32 = 2;
const REGISTER_CREATION_RATIO_TO_CHURN: u32 = 5;
const DBC_CREATION_RATIO_TO_CHURN: u32 = 2;

const CHUNKS_SIZE: usize = 1024 * 1024;

const CONTENT_QUERY_RATIO_TO_CHURN: u32 = 12;
const MAX_NUM_OF_QUERY_ATTEMPTS: u8 = 5;

// Default total amount of time we run the checks for before reporting the outcome.
// It can be overriden by setting the 'TEST_DURATION_MINS' env var.
const TEST_DURATION: Duration = Duration::from_secs(60 * 60); // 1hr

const PAYING_WALLET_INITIAL_BALANCE: u64 = 100_000_000_000_000;
const TRANSFERS_WALLET_INITIAL_BALANCE: u64 = 200_000_000_000;

type ContentList = Arc<RwLock<VecDeque<NetworkAddress>>>;
type DbcMap = Arc<RwLock<BTreeMap<DbcAddress, Dbc>>>;

struct ContentError {
    net_addr: NetworkAddress,
    attempts: u8,
    last_err: Error,
}

impl fmt::Debug for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?}, attempts: {}, last error: {:?}",
            self.net_addr, self.attempts, self.last_err
        )
    }
}

type ContentErredList = Arc<RwLock<BTreeMap<NetworkAddress, ContentError>>>;

#[tokio::test(flavor = "multi_thread")]
async fn data_availability_during_churn() -> Result<()> {
    let test_duration = if let Ok(str) = std::env::var("TEST_DURATION_MINS") {
        Duration::from_secs(60 * str.parse::<u64>()?)
    } else {
        TEST_DURATION
    };

    let churn_period = if let Ok(str) = std::env::var("TEST_TOTAL_CHURN_CYCLES") {
        println!("Using value set in 'TEST_TOTAL_CHURN_CYCLES' env var: {str}");
        let cycles = str.parse::<u32>()?;
        test_duration / cycles
    } else {
        // Ensure at least some nodes got churned twice.
        test_duration / std::cmp::max(CHURN_CYCLES * NODE_COUNT, NODE_COUNT + EXTRA_CHURN_COUNT)
    };
    println!("Nodes will churn every {churn_period:?}");

    // Create a cross thread usize for tracking churned nodes
    let churn_count = Arc::new(RwLock::new(0_usize));

    // Allow to disable Registers data creation/checks, storing and querying only Chunks during churn.
    // Default to be not carry out chunks only during churn.
    let chunks_only = std::env::var("CHUNKS_ONLY").is_ok();

    println!(
        "Running this test for {test_duration:?}{}...",
        if chunks_only { " (Chunks only)" } else { "" }
    );

    let tmp_dir = std::env::temp_dir();
    let logging_targets = vec![
        ("safenode".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::TRACE),
        ("sn_networking".to_string(), Level::TRACE),
        ("sn_node".to_string(), Level::TRACE),
    ];
    let log_appender_guard = init_logging(
        logging_targets,
        LogOutputDest::Path(tmp_dir.join("safe-client")),
        LogFormat::Default,
    )?;

    // The testnet will create a `faucet` at last. To avoid mess up with that,
    // wait for a while to ensure the spends of that got settled.
    sleep(std::time::Duration::from_secs(10)).await;

    println!("Creating a client and paying wallet...");
    let paying_wallet_dir = TempDir::new()?;
    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), PAYING_WALLET_INITIAL_BALANCE).await?;

    println!(
        "Client and paying_wallet created with signing key: {:?}",
        client.signer_pk()
    );

    // Shared bucket where we keep track of content created/stored on the network
    let content = ContentList::default();

    // Shared bucket where we keep track of DBCs created/stored on the network
    let dbcs = DbcMap::default();

    // Spawn a task to create Registers and DBCs at random locations,
    // at a higher frequency than the churning events
    if !chunks_only {
        println!("Creating transfer wallet taking balance from the payment wallet");
        let transfers_wallet_dir = TempDir::new()?;
        let transfers_wallet = get_funded_wallet(
            &client,
            paying_wallet,
            transfers_wallet_dir.path(),
            TRANSFERS_WALLET_INITIAL_BALANCE,
        )
        .await?;
        println!("Transfer wallet created");

        create_registers_task(client.clone(), content.clone(), churn_period);

        create_dbc_task(
            client.clone(),
            transfers_wallet,
            content.clone(),
            dbcs.clone(),
            churn_period,
        );
    }

    // The paying wallet could be updated to pay for the transfer_wallet.
    // Hence reload it here to avoid double spend, AND the clippy complains.
    let paying_wallet = get_wallet(paying_wallet_dir.path()).await;

    println!("Uploading some chunks before carry out node churning");

    // Spawn a task to store Chunks at random locations, at a higher frequency than the churning events
    store_chunks_task(client.clone(), paying_wallet, content.clone(), churn_period);

    // Spawn a task to churn nodes
    churn_nodes_task(churn_count.clone(), test_duration, churn_period);

    // Shared bucket where we keep track of the content which erred when creating/storing/fetching.
    // We remove them from this bucket if we are then able to query/fetch them successfully.
    // We only try to query them 'MAX_NUM_OF_QUERY_ATTEMPTS' times, then report them effectivelly as failures.
    let content_erred = ContentErredList::default();

    // Shared bucket where we keep track of the content we failed to fetch for 'MAX_NUM_OF_QUERY_ATTEMPTS' times.
    let failures = ContentErredList::default();

    // Spawn a task to randomly query/fetch the content we create/store
    query_content_task(
        client.clone(),
        content.clone(),
        content_erred.clone(),
        dbcs.clone(),
        churn_period,
    );

    // Spawn a task to retry querying the content that failed, up to 'MAX_NUM_OF_QUERY_ATTEMPTS' times,
    // and mark them as failures if they effectivelly cannot be retrieved.
    retry_query_content_task(
        client.clone(),
        content_erred.clone(),
        failures.clone(),
        dbcs.clone(),
        churn_period,
    );

    let start_time = Instant::now();
    while start_time.elapsed() < test_duration {
        let failed = failures.read().await;
        println!(
            "Current failures after {:?} ({}): {:?}",
            start_time.elapsed(),
            failed.len(),
            failed.values()
        );
        sleep(churn_period).await;
    }

    println!();
    println!(
        ">>>>>> Test stopping after running for {:?}. <<<<<<",
        start_time.elapsed()
    );
    println!("{:?} churn events happened.", *churn_count.read().await);
    println!();

    let failed = failures.read().await;
    if failed.len() > 0 {
        bail!("{} failure/s in test: {:?}", failed.len(), failed.values());
    }

    // The churning of storing_chunk/querying_chunk are all random,
    // which will have a high chance that newly stored chunk got queried BEFORE
    // the original holders churned out.
    // i.e. the test may pass even without any replication
    // Hence, we carry out a final round of query all data to confirm storage.
    println!("Final querying confirmation of content");
    let mut content_queried_count = 0;

    // take one read lock to avoid holding the lock for the whole loop
    // prevent any late content uploads being added to the list
    let content = content.read().await;
    let uploaded_content_count = content.len();
    for net_addr in content.iter() {
        let result = final_retry_query_content(&client, net_addr, dbcs.clone(), churn_period).await;
        assert!(
            result.is_ok(),
            "Failed to query content at {net_addr:?} with error {result:?}"
        );

        content_queried_count += 1;
    }

    println!("{:?} pieces of content queried", content_queried_count);

    assert_eq!(
        content_queried_count, uploaded_content_count,
        "Not all content was queried"
    );

    drop(log_appender_guard);

    println!("Test passed after running for {:?}.", start_time.elapsed());
    Ok(())
}

// Spawns a task which periodically creates DBCs at random locations.
fn create_dbc_task(
    client: Client,
    transfers_wallet: LocalWallet,
    content: ContentList,
    dbcs: DbcMap,
    churn_period: Duration,
) {
    let _handle = tokio::spawn(async move {
        // Create Dbc at a higher frequency than the churning events
        let delay = churn_period / DBC_CREATION_RATIO_TO_CHURN;

        let mut wallet_client = WalletClient::new(client.clone(), transfers_wallet);

        loop {
            sleep(delay).await;

            let dest_pk = MainKey::random().public_address();
            let dbc = wallet_client
                .send(Token::from_nano(10), dest_pk, false)
                .await
                .expect("Failed to send DBC to {dest_pk}");

            let dbc_addr = DbcAddress::from_dbc_id(&dbc.id());
            let net_addr = NetworkAddress::DbcAddress(dbc_addr);
            println!("Created DBC at {dbc_addr:?} after {delay:?}");
            content.write().await.push_back(net_addr);
            let _ = dbcs.write().await.insert(dbc_addr, dbc);
        }
    });
}

// Spawns a task which periodically creates Registers at random locations.
fn create_registers_task(client: Client, content: ContentList, churn_period: Duration) {
    let _handle = tokio::spawn(async move {
        // Create Registers at a higher frequency than the churning events
        let delay = churn_period / REGISTER_CREATION_RATIO_TO_CHURN;

        loop {
            let meta = XorName(rand::random());
            let owner = client.signer_pk();

            let addr = RegisterAddress::new(meta, owner);
            println!("Creating Register at {addr:?} in {delay:?}");
            sleep(delay).await;

            match client.create_register(meta, false).await {
                Ok(_) => content
                    .write()
                    .await
                    .push_back(NetworkAddress::RegisterAddress(addr)),
                Err(err) => println!("Discarding new Register ({addr:?}) due to error: {err:?}"),
            }
        }
    });
}

// Spawns a task which periodically stores Chunks at random locations.
fn store_chunks_task(
    client: Client,
    paying_wallet: LocalWallet,
    content: ContentList,
    churn_period: Duration,
) {
    let _handle = tokio::spawn(async move {
        // Store Chunks at a higher frequency than the churning events
        let delay = churn_period / CHUNK_CREATION_RATIO_TO_CHURN;
        let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

        let file_api = Files::new(client);
        let mut rng = OsRng;
        loop {
            let random_bytes: Vec<u8> = ::std::iter::repeat(())
                .map(|()| rng.gen::<u8>())
                .take(CHUNKS_SIZE)
                .collect();
            let bytes = Bytes::copy_from_slice(&random_bytes);

            let (addr, chunks) = file_api
                .chunk_bytes(bytes.clone())
                .expect("Failed to chunk bytes");

            println!(
                "Paying storage for ({}) new Chunk/s of file ({} bytes) at {addr:?} in {delay:?}",
                chunks.len(),
                bytes.len()
            );
            sleep(delay).await;
            let proofs = wallet_client
                .pay_for_storage(chunks.iter().map(|c| c.name()), true)
                .await
                .expect("Failed to pay for storage for new file at {addr:?}");

            println!(
                "Storing ({}) Chunk/s of file ({} bytes) at {addr:?} in {delay:?}",
                chunks.len(),
                bytes.len()
            );
            sleep(delay).await;

            match file_api
                .upload_chunks_in_batches(chunks.into_iter(), &proofs, false)
                .await
            {
                Ok(()) => content
                    .write()
                    .await
                    .push_back(NetworkAddress::ChunkAddress(ChunkAddress::new(addr))),
                Err(err) => println!("Discarding new Chunk ({addr:?}) due to error: {err:?}"),
            }
        }
    });
}

// Spawns a task which periodically queries a content by randomly choosing it from the list
// of content created by another task.
fn query_content_task(
    client: Client,
    content: ContentList,
    content_erred: ContentErredList,
    dbcs: DbcMap,
    churn_period: Duration,
) {
    let _handle = tokio::spawn(async move {
        let delay = churn_period / CONTENT_QUERY_RATIO_TO_CHURN;
        loop {
            let len = content.read().await.len();
            if len == 0 {
                println!("No content created/stored just yet, let's try in {delay:?} ...");
                sleep(delay).await;
                continue;
            }

            // let's choose a random content to query, picking it from the list of created
            let index = rand::thread_rng().gen_range(0..len);
            let net_addr = content.read().await[index].clone();
            trace!("Querying content (bucket index: {index}) at {net_addr:?} in {delay:?}");
            sleep(delay).await;

            match query_content(&client, &net_addr, dbcs.clone()).await {
                Ok(_) => {
                    let _ = content_erred.write().await.remove(&net_addr);
                }
                Err(last_err) => {
                    println!(
                        "Failed to query content (index: {index}) at {net_addr}: {last_err:?}"
                    );
                    // mark it to try 'MAX_NUM_OF_QUERY_ATTEMPTS' times.
                    let _ = content_erred
                        .write()
                        .await
                        .entry(net_addr.clone())
                        .and_modify(|curr| curr.attempts += 1)
                        .or_insert(ContentError {
                            net_addr,
                            attempts: 1,
                            last_err,
                        });
                }
            }
        }
    });
}

// Spawns a task which periodically picks up a node, and restarts it to cause churn in the network.
fn churn_nodes_task(
    churn_count: Arc<RwLock<usize>>,
    test_duration: Duration,
    churn_period: Duration,
) {
    let start = Instant::now();
    let _handle = tokio::spawn(async move {
        let mut node_index = 1;
        let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        loop {
            sleep(churn_period).await;

            // break out if we've run the duration of churn
            if start.elapsed() > test_duration {
                println!("Test duration reached, stopping churn nodes task");
                break;
            }

            addr.set_port(12000 + node_index);

            println!("Restarting node through its RPC service at {addr}");

            if let Err(err) = node_restart(addr).await {
                println!("Failed to restart node with RPC endpoint {addr}: {err}");
                continue;
            }

            *churn_count.write().await += 1;

            node_index += 1;
            if node_index > NODE_COUNT as u16 {
                node_index = 1;
            }
        }
    });
}

// Checks (periodically) for any content that an error was reported either at the moment of its creation or
// in a later query attempt.
fn retry_query_content_task(
    client: Client,
    content_erred: ContentErredList,
    failures: ContentErredList,
    dbcs: DbcMap,
    churn_period: Duration,
) {
    let _handle = tokio::spawn(async move {
        let delay = 2 * churn_period;
        loop {
            sleep(delay).await;

            // let's try to query from the bucket of those that erred upon creation/query
            let erred = content_erred.write().await.pop_first();

            if let Some((net_addr, mut content_error)) = erred {
                let attempts = content_error.attempts + 1;

                println!("Querying erred content at {net_addr}, attempt: #{attempts} ...");
                if let Err(last_err) = query_content(&client, &net_addr, dbcs.clone()).await {
                    println!("Erred content is still not retrievable at {net_addr} after {attempts} attempts: {last_err:?}");
                    // We only keep it to retry 'MAX_NUM_OF_QUERY_ATTEMPTS' times,
                    // otherwise report it effectivelly as failure.
                    content_error.attempts = attempts;
                    content_error.last_err = last_err;

                    if attempts == MAX_NUM_OF_QUERY_ATTEMPTS {
                        let _ = failures.write().await.insert(net_addr, content_error);
                    } else {
                        let _ = content_erred.write().await.insert(net_addr, content_error);
                    }
                }
            }
        }
    });
}

async fn final_retry_query_content(
    client: &Client,
    net_addr: &NetworkAddress,
    dbcs: DbcMap,
    churn_period: Duration,
) -> Result<()> {
    let mut attempts = 1;
    loop {
        println!("Querying content at {net_addr}, attempt: #{attempts} ...");
        if let Err(last_err) = query_content(client, net_addr, dbcs.clone()).await {
            if attempts == MAX_NUM_OF_QUERY_ATTEMPTS {
                bail!("Final check: Content is still not retrievable at {net_addr} after {attempts} attempts: {last_err:?}");
            } else {
                attempts += 1;
                let delay = 2 * churn_period;
                sleep(delay).await;
                continue;
            }
        } else {
            // content retrieved fine
            return Ok(());
        }
    }
}

async fn node_restart(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;

    let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
    let log_dir = Path::new(&response.get_ref().log_dir);
    let root_dir = log_dir
        .parent()
        .ok_or_else(|| eyre!("could not obtain parent from logging directory"))?;

    // remove Chunks records
    let chunks_records = root_dir.join("record_store");
    if let Ok(true) = chunks_records.try_exists() {
        println!("Removing Chunks records from {}", chunks_records.display());
        remove_dir_all(chunks_records).await?;
    }

    // remove Registers records
    let registers_records = root_dir.join("registers");
    if let Ok(true) = registers_records.try_exists() {
        println!(
            "Removing Registers records from {}",
            registers_records.display()
        );
        remove_dir_all(registers_records).await?;
    }

    let _response = client
        .restart(Request::new(RestartRequest { delay_millis: 0 }))
        .await?;

    println!(
        "Node restart requested to RPC service at {addr}, and removed all its chunks and registers records at {}",
        log_dir.display()
    );

    Ok(())
}

async fn query_content(
    client: &Client,
    net_addr: &NetworkAddress,
    dbcs: DbcMap,
) -> Result<(), Error> {
    match net_addr {
        NetworkAddress::DbcAddress(addr) => {
            if let Some(dbc) = dbcs.read().await.get(addr) {
                match client.verify(dbc).await {
                    Ok(_) => Ok(()),
                    Err(err) => Err(Error::CouldNotVerifyTransfer(format!(
                        "Verification of dbc {addr:?} failed with error: {err:?}"
                    ))),
                }
            } else {
                Err(Error::CouldNotVerifyTransfer(format!(
                    "Do not have the DBC: {addr:?}"
                )))
            }
        }
        NetworkAddress::RegisterAddress(addr) => {
            let _ = client.get_register(*addr).await?;
            Ok(())
        }
        NetworkAddress::ChunkAddress(addr) => {
            let file_api = Files::new(client.clone());
            let _ = file_api.read_bytes(*addr).await?;
            Ok(())
        }
        _other => Ok(()), // we don't create/store any other type of content in this test yet
    }
}
