// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::get_client;

use crate::{
    client::{Client, Error},
    network::Error as NetworkError,
    protocol::storage::RegisterAddress,
};

use safenode_proto::{safe_node_client::SafeNodeClient, RestartRequest};

use eyre::{bail, Result};
use rand::Rng;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::RwLock, time::sleep};
use tonic::Request;
use xor_name::XorName;

// this includes code generated from .proto files
#[allow(unused_qualifications, unreachable_pub, clippy::unwrap_used)]
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

const NODE_COUNT: u16 = 25;
const CHURN_PERIOD_MILLIS: u64 = 30_000;
const REGISTER_CREATION_RATIO_TO_CHURN: u64 = 10;
const REGISTER_QUERY_RATIO_TO_CHURN: u64 = 12;
const MAX_NUM_OF_QUERY_ATTEMPTS: u8 = 10;

// Total amount of time we run the checks for before reporting the outcome
const TOTAL_TIME_OF_TEST: Duration = Duration::from_secs(60 * 60); // 1hr

type RegistersList = Arc<RwLock<VecDeque<RegisterAddress>>>;

struct RegError {
    addr: RegisterAddress,
    attempts: u8,
    last_err: Error,
}

impl fmt::Debug for RegError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?}, attempts: {}, last error: {:?}",
            self.addr, self.attempts, self.last_err
        )
    }
}

type RegistersErredList = Arc<RwLock<BTreeMap<RegisterAddress, RegError>>>;

#[tokio::test(flavor = "multi_thread")]
async fn register_data_during_churn() -> Result<()> {
    let mut rng = rand::thread_rng();
    println!("Creating a client...");
    let client = get_client().await;
    println!("Client created with signing key: {:?}", client.signer_pk());

    // Spawn a task to churn nodes
    churn_nodes_task();

    // Shared bucket where we keep track of Registers created
    let registers = RegistersList::default();

    // Shared bucket where we keep track of those Registers which erred when creating/fetching them.
    // We remove them from this bucket if we are then able to query/fetch them successfully.
    // We only try to query them 'MAX_NUM_OF_QUERY_ATTEMPTS' times, then report them effectivelly as failures.
    let regs_erred = RegistersErredList::default();

    // Shared bucket where we keep track of those Registers we failed to fetch for 'MAX_NUM_OF_QUERY_ATTEMPTS' times.
    let failures = RegistersErredList::default();

    // Spawn a task to randomly query/fetch the Registers we create
    query_registers_task(client.clone(), registers.clone(), regs_erred.clone());

    // Spawn a task to retry querying the Registers that failed, up to 'MAX_NUM_OF_QUERY_ATTEMPTS' times,
    // and mark them as failures if they effectivelly cannot be retrieved.
    retry_query_registers_task(client.clone(), regs_erred.clone(), failures.clone());

    let start_time = Instant::now();

    // Create Registers at random locations, at a higher frequency than the churning events
    let delay = Duration::from_millis(CHURN_PERIOD_MILLIS / REGISTER_CREATION_RATIO_TO_CHURN);
    while start_time.elapsed() < TOTAL_TIME_OF_TEST {
        let xorname = XorName::random(&mut rng);
        let tag = rng.gen();

        let addr = RegisterAddress { name: xorname, tag };
        println!("Creating Register at {addr:?} in {delay:?}");
        sleep(delay).await;

        match client.create_register(xorname, tag).await {
            Err(err @ Error::Network(NetworkError::OutboundError(_))) => {
                println!("Error (recoverable) when creating Register at {addr:?}: {err:?}");
                let _ = regs_erred.write().await.insert(
                    addr,
                    RegError {
                        addr,
                        attempts: 0,
                        last_err: err,
                    },
                );
                registers.write().await.push_back(addr);
            }
            Err(err) => {
                println!("Discarding Register due to error when creating it {addr:?}: {err:?}")
            }
            Ok(_) => registers.write().await.push_back(addr),
        }

        let failed = failures.read().await;
        println!(
            "Current failures after {:?} ({}): {:?}",
            start_time.elapsed(),
            failed.len(),
            failed.values()
        );
    }

    println!();
    println!("Test stopped after running for {:?}.", start_time.elapsed());
    println!();

    let failed = failures.read().await;
    if failed.len() > 0 {
        bail!("{} failure/s in test: {:?}", failed.len(), failed.values());
    }

    println!("Test passed after running for {:?}.", start_time.elapsed());
    Ok(())
}

// Spawns a task which periodically queries a Register by randomly choosing it from the list
// of registers created by another task.
fn query_registers_task(client: Client, registers: RegistersList, regs_erred: RegistersErredList) {
    let _handle = tokio::spawn(async move {
        let delay = Duration::from_millis(CHURN_PERIOD_MILLIS / REGISTER_QUERY_RATIO_TO_CHURN);
        loop {
            let len = registers.read().await.len();
            if len == 0 {
                println!("No Registers created just yet, let's try in {delay:?} ...");
                sleep(delay).await;
                continue;
            }

            // let's choose a random Register to query, picking it from the list of created
            let index = rand::thread_rng().gen_range(0..len);
            let addr = registers.read().await[index];
            println!("Querying Register (bucket index: {index}) at {addr:?} in {delay:?}");
            sleep(delay).await;

            match client.get_register(*addr.name(), addr.tag()).await {
                Ok(_) => {
                    let _ = regs_erred.write().await.remove(&addr);
                }
                Err(last_err) => {
                    println!("Failed to query Register (index: {index}) at {addr:?}: {last_err:?}");
                    // mark it to try 'MAX_NUM_OF_QUERY_ATTEMPTS' times.
                    let _ = regs_erred
                        .write()
                        .await
                        .entry(addr)
                        .and_modify(|curr| curr.attempts += 1)
                        .or_insert(RegError {
                            addr,
                            attempts: 1,
                            last_err,
                        });
                }
            }
        }
    });
}

// Spawns a task which periodically picks up a random node, and restarts it to cause churn in the network.
fn churn_nodes_task() {
    let _handle = tokio::spawn(async {
        let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        let delay = Duration::from_millis(CHURN_PERIOD_MILLIS);
        loop {
            // let's choose a random node to restart
            let node_index = rand::thread_rng().gen_range(1..=NODE_COUNT);
            addr.set_port(12000 + node_index);

            println!("Restarting node through its RPC service at {addr} in {delay:?}");
            sleep(delay).await;

            node_restart(addr, 1000)
                .await
                .expect("Failed to restart node with RPC endpoint {addr}");
        }
    });
}

// Checks (periodically) for any registers that an error was reported either at the moment of its creation or
// in a later query attempt.
fn retry_query_registers_task(
    client: Client,
    regs_erred: RegistersErredList,
    failures: RegistersErredList,
) {
    let _handle = tokio::spawn(async move {
        let delay = Duration::from_millis(2 * CHURN_PERIOD_MILLIS);
        loop {
            sleep(delay).await;

            // let's try to query from the bucket of those that erred upon creation/query
            let erred = regs_erred.write().await.pop_first();

            if let Some((addr, mut reg_error)) = erred {
                let attempts = reg_error.attempts + 1;
                println!("Querying erred Register at {addr:?}, attempt: #{attempts} ...");
                if let Err(last_err) = client.get_register(*addr.name(), addr.tag()).await {
                    println!("Erred Register is still not retrievable at {addr:?} after {attempts} attempts: {last_err:?}");
                    // We only keep it to retry 'MAX_NUM_OF_QUERY_ATTEMPTS' times,
                    // otherwise report it effectivelly as failure.
                    reg_error.attempts = attempts;
                    reg_error.last_err = last_err;

                    if attempts == MAX_NUM_OF_QUERY_ATTEMPTS {
                        let _ = failures.write().await.insert(addr, reg_error);
                    } else {
                        let _ = regs_erred.write().await.insert(addr, reg_error);
                    }
                }
            }
        }
    });
}

async fn node_restart(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .restart(Request::new(RestartRequest { delay_millis }))
        .await?;
    Ok(())
}
