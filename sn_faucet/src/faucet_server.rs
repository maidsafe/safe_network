// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg(feature = "distribution")]
use crate::token_distribution;
use crate::{claim_genesis, send_tokens};
use color_eyre::eyre::Result;
use fs2::FileExt;
use sn_client::{
    acc_packet::load_account_wallet_or_create_with_mnemonic, fund_faucet_from_genesis_wallet,
    Client,
};
use sn_transfers::{
    get_faucet_data_dir, wallet_lockfile_name, NanoTokens, Transfer, WALLET_DIR_NAME,
};
use std::path::Path;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};
use warp::{
    http::{Response, StatusCode},
    Filter, Reply,
};

/// Run the faucet server.
///
/// This will listen on port 8000 and send a transfer of tokens as response to any GET request.
///
/// # Example
///
/// ```bash
/// # run faucet server
/// cargo run  --features="local-discovery" --bin faucet --release -- server
///
/// # query faucet server for money for our address `get local wallet address`
/// curl "localhost:8000/`cargo run  --features="local-discovery"  --bin safe --release  wallet address | tail -n 1`" > transfer_hex
///
/// # receive transfer with our wallet
/// cargo run  --features="local-discovery" --bin safe --release  wallet receive --file transfer_hex
///
/// # balance should be updated
/// ```
pub async fn run_faucet_server(client: &Client) -> Result<()> {
    let root_dir = get_faucet_data_dir();
    let wallet = load_account_wallet_or_create_with_mnemonic(&root_dir, None)?;
    claim_genesis(client, wallet).await.map_err(|err| {
        println!("Faucet Server couldn't start as we failed to claim Genesis");
        eprintln!("Faucet Server couldn't start as we failed to claim Genesis");
        error!("Faucet Server couldn't start as we failed to claim Genesis");
        err
    })?;
    startup_server(client.clone()).await
}

pub async fn restart_faucet_server(client: &Client) -> Result<()> {
    let root_dir = get_faucet_data_dir();
    println!("Loading the previous wallet at {root_dir:?}");
    debug!("Loading the previous wallet at {root_dir:?}");

    deposit(&root_dir)?;

    println!("Previous wallet loaded");
    debug!("Previous wallet loaded");

    startup_server(client.clone()).await
}

#[cfg(feature = "distribution")]
async fn respond_to_distribution_request(
    client: Client,
    query: HashMap<String, String>,
    balances: HashMap<String, NanoTokens>,
    semaphore: Arc<Semaphore>,
) -> std::result::Result<impl Reply, std::convert::Infallible> {
    let permit = semaphore.try_acquire();

    // some rate limiting
    if is_wallet_locked() || permit.is_err() {
        warn!("Rate limited request due to locked wallet");

        let mut response = Response::new("Rate limited".to_string());
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

        // Either opening the file or locking it failed, indicating rate limiting should occur
        return Ok(response);
    }

    let r =
        match token_distribution::handle_distribution_req(&client, query, balances.clone()).await {
            Ok(distribution) => Response::new(distribution.to_string()),
            Err(err) => {
                eprintln!("Failed to get distribution: {err}");
                error!("Failed to get distribution: {err}");
                Response::new(format!("Failed to get distribution: {err}"))
            }
        };

    Ok(r)
}

fn is_wallet_locked() -> bool {
    info!("Checking if wallet is locked");
    let root_dir = get_faucet_data_dir();

    let wallet_dir = root_dir.join(WALLET_DIR_NAME);
    let wallet_lockfile_name = wallet_lockfile_name(&wallet_dir);
    let file_result = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(wallet_lockfile_name)
        .and_then(|file| file.try_lock_exclusive());
    info!("After if wallet is locked");

    if file_result.is_err() {
        // Either opening the file or locking it failed, indicating rate limiting should occur
        return true;
    }

    false
}

async fn respond_to_donate_request(
    client: Client,
    transfer_str: String,
    semaphore: Arc<Semaphore>,
) -> std::result::Result<impl Reply, std::convert::Infallible> {
    let permit = semaphore.try_acquire();
    info!("Got donate request with: {transfer_str}");

    // some rate limiting
    if is_wallet_locked() || permit.is_err() {
        warn!("Rate limited request due");
        let mut response = Response::new("Rate limited".to_string());
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

        // Either opening the file or locking it failed, indicating rate limiting should occur
        return Ok(response);
    }

    let faucet_root = get_faucet_data_dir();
    let mut wallet = match load_account_wallet_or_create_with_mnemonic(&faucet_root, None) {
        Ok(wallet) => wallet,
        Err(_error) => {
            let mut response = Response::new("Could not load wallet".to_string());
            *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;

            // Either opening the file or locking it failed, indicating rate limiting should occur
            return Ok(response);
        }
    };

    if let Err(err) = fund_faucet_from_genesis_wallet(&client, &mut wallet).await {
        eprintln!("Failed to load + fund faucet wallet: {err}");
        error!("Failed to load + fund faucet wallet: {err}");
        let mut response = Response::new(format!("Failed to load faucet wallet: {err}"));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        return Ok(response);
    };

    // return key is Transfer is empty
    if transfer_str.is_empty() {
        let address = wallet.address().to_hex();
        return Ok(Response::new(format!("Faucet wallet address: {address}")));
    }

    // parse transfer
    let transfer = match Transfer::from_hex(&transfer_str) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("Failed to parse transfer: {err}");
            error!("Failed to parse transfer {transfer_str}: {err}");
            let mut response = Response::new(format!("Failed to parse transfer: {err}"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
        }
    };

    // receive transfer
    let res = client.receive(&transfer, &wallet).await;
    match res {
        Ok(cashnotes) => {
            let old_balance = wallet.balance();
            if let Err(e) = wallet.deposit_and_store_to_disk(&cashnotes) {
                eprintln!("Failed to store deposited amount: {e}");
                error!("Failed to store deposited amount: {e}");
                let mut response = Response::new(format!("Failed to store deposited amount: {e}"));
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                return Ok(response);
            }
            let new_balance = wallet.balance();

            info!("Successfully stored cash_note to wallet dir");
            info!("Old balance: {old_balance}, new balance: {new_balance}");

            Ok(Response::new("Thank you!".to_string()))
        }
        Err(err) => {
            eprintln!("Failed to verify and redeem transfer: {err}");
            error!("Failed to verify and redeem transfer: {err}");
            let mut response =
                Response::new(format!("Failed to verify and redeem transfer: {err}"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            Ok(response)
        }
    }
}

async fn respond_to_gift_request(
    client: Client,
    key: String,
    semaphore: Arc<Semaphore>,
) -> std::result::Result<impl Reply, std::convert::Infallible> {
    let faucet_root = get_faucet_data_dir();

    let from = match load_account_wallet_or_create_with_mnemonic(&faucet_root, None) {
        Ok(wallet) => wallet,
        Err(_error) => {
            let mut response = Response::new("Could not load wallet".to_string());
            *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;

            // Either opening the file or locking it failed, indicating rate limiting should occur
            return Ok(response);
        }
    };

    let permit = semaphore.try_acquire();

    // some rate limiting
    if is_wallet_locked() || permit.is_err() {
        warn!("Rate limited request due");
        let mut response = Response::new("Rate limited".to_string());
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

        // Either opening the file or locking it failed, indicating rate limiting should occur
        return Ok(response);
    }

    const GIFT_AMOUNT_SNT: &str = "1";
    match send_tokens(&client, from, GIFT_AMOUNT_SNT, &key).await {
        Ok(transfer) => {
            println!("Sent tokens to {key}");
            debug!("Sent tokens to {key}");
            Ok(Response::new(transfer.to_string()))
        }
        Err(err) => {
            eprintln!("Failed to send tokens to {key}: {err}");
            error!("Failed to send tokens to {key}: {err}");
            Ok(Response::new(format!("Failed to send tokens: {err}")))
        }
    }
}

async fn startup_server(client: Client) -> Result<()> {
    // Create a semaphore with a single permit
    let semaphore = Arc::new(Semaphore::new(1));

    #[allow(unused)]
    let mut balances = HashMap::<String, NanoTokens>::new();
    #[cfg(feature = "distribution")]
    {
        balances = token_distribution::load_maid_snapshot()?;
        let keys = token_distribution::load_maid_claims()?;
        // Each distribution takes about 500ms to create, so for thousands of
        // initial distributions this takes many minutes. This is run in the
        // background instead of blocking the server from starting.
        tokio::spawn(token_distribution::distribute_from_maid_to_tokens(
            client.clone(),
            balances.clone(),
            keys,
        ));
    }

    let gift_client = client.clone();
    let donation_client = client.clone();
    let donation_addr_client = client.clone();
    let donation_semaphore = semaphore.clone();
    let donation_addr_semaphore = semaphore.clone();
    #[cfg(feature = "distribution")]
    let semaphore_dist = semaphore.clone();

    // GET /distribution/address=address&wallet=wallet&signature=signature
    #[cfg(feature = "distribution")]
    let distribution_route = warp::get()
        .and(warp::path("distribution"))
        .and(warp::query::<HashMap<String, String>>())
        .map(|query| {
            debug!("Received distribution request: {query:?}");
            query
        })
        .and_then(move |query| {
            let semaphore = semaphore_dist.clone();
            let client = client.clone();
            respond_to_distribution_request(client, query, balances.clone(), semaphore)
        });

    // GET /key
    let gift_route = warp::get()
        .and(warp::path!(String))
        .map(|query| {
            debug!("Gift distribution request: {query}");
            query
        })
        .and_then(move |key| {
            let client = gift_client.clone();
            let semaphore = semaphore.clone();

            respond_to_gift_request(client, key, semaphore)
        });

    // GET /donate
    let donation_addr = warp::get().and(warp::path("donate")).and_then(move || {
        debug!("Donation address request");
        let client = donation_addr_client.clone();
        let semaphore = donation_addr_semaphore.clone();

        respond_to_donate_request(client, String::new(), semaphore)
    });

    // GET /donate/transfer
    let donation_route = warp::get()
        .and(warp::path!("donate" / String))
        .map(|query| {
            debug!("Donation request: {query}");
            query
        })
        .and_then(move |transfer| {
            let client = donation_client.clone();
            let semaphore = donation_semaphore.clone();

            respond_to_donate_request(client, transfer, semaphore)
        });

    println!("Starting http server listening on port 8000...");
    debug!("Starting http server listening on port 8000...");

    #[cfg(feature = "distribution")]
    warp::serve(
        distribution_route
            .or(donation_route)
            .or(donation_addr)
            .or(gift_route),
    )
    // warp::serve(gift_route)
    .run(([0, 0, 0, 0], 8000))
    .await;

    #[cfg(not(feature = "distribution"))]
    warp::serve(donation_route.or(donation_addr).or(gift_route))
        .run(([0, 0, 0, 0], 8000))
        .await;

    debug!("Server closed");
    Ok(())
}

fn deposit(root_dir: &Path) -> Result<()> {
    let mut wallet = load_account_wallet_or_create_with_mnemonic(root_dir, None)?;

    let previous_balance = wallet.balance();

    wallet.try_load_cash_notes()?;

    let deposited = NanoTokens::from(wallet.balance().as_nano() - previous_balance.as_nano());
    if deposited.is_zero() {
        println!("Nothing deposited.");
    } else if let Err(err) = wallet.deposit_and_store_to_disk(&vec![]) {
        println!("Failed to store deposited ({deposited}) amount: {err:?}");
    } else {
        println!("Deposited {deposited}.");
    }

    Ok(())
}
