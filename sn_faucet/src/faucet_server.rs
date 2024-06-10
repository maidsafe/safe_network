// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::claim_genesis;
#[cfg(feature = "gifting")]
use crate::send_tokens;
#[cfg(feature = "distribution")]
use crate::token_distribution;
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

#[cfg(feature = "initial-data")]
use crate::gutenberger::{download_book, State};
#[cfg(feature = "initial-data")]
use autonomi::FilesUploader;
#[cfg(feature = "initial-data")]
use reqwest::Client as ReqwestClient;
#[cfg(feature = "initial-data")]
use sn_client::{UploadCfg, BATCH_SIZE};
#[cfg(feature = "initial-data")]
use sn_protocol::storage::{ChunkAddress, RetryStrategy};
#[cfg(feature = "initial-data")]
use std::{fs::File, path::PathBuf};
#[cfg(feature = "initial-data")]
use tokio::{fs, io::AsyncWriteExt};

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

    #[cfg(feature = "initial-data")]
    {
        let _ = upload_initial_data(client, &root_dir).await;
    }

    startup_server(client.clone()).await
}

#[cfg(feature = "initial-data")]
/// Trigger one by one uploading of intitial data packets to the entwork.
async fn upload_initial_data(client: &Client, root_dir: &Path) -> Result<()> {
    let temp_dir = std::env::temp_dir();
    let state_file = temp_dir.join("state.json");
    let uploaded_books_file = temp_dir.join("uploaded_books.json");
    let mut state = State::load_from_file(&state_file)?;

    let reqwest_client = ReqwestClient::new();

    let mut uploaded_books: Vec<(String, String)> = if uploaded_books_file.exists() {
        let file = File::open(&uploaded_books_file)?;
        serde_json::from_reader(file)?
    } else {
        vec![]
    };

    println!("Previous upload state restored");
    info!("Previous upload state restored");

    for book_id in state.max_seen()..u16::MAX as u32 {
        if state.has_seen(book_id) {
            println!("Already seen book ID: {book_id}");
            info!("Already seen book ID: {book_id}");
            continue;
        }

        match download_book(&reqwest_client, book_id).await {
            Ok(data) => {
                println!("Downloaded book ID: {book_id}");
                info!("Downloaded book ID: {book_id}");

                let fname = format!("{book_id}.book");
                let fpath = temp_dir.join(fname.clone());

                match mark_download_progress(book_id, &fpath, data, &mut state, &state_file).await {
                    Ok(_) => {
                        println!("Marked download progress book ID: {book_id} completed");
                        info!("Marked download progress book ID: {book_id} completed");
                    }
                    Err(err) => {
                        println!("When marking download progress book ID: {book_id}, encountered error {err:?}");
                        error!("When marking download progress book ID: {book_id}, encountered error {err:?}");
                        continue;
                    }
                }

                match upload_downloaded_book(client, root_dir, fpath).await {
                    Ok(head_addresses) => {
                        println!("Uploaded book ID: {book_id}");
                        info!("Uploaded book ID: {book_id}");

                        // There shall be just one
                        for head_address in head_addresses {
                            uploaded_books.push((fname.clone(), head_address.to_hex()));

                            match mark_upload_progress(&uploaded_books_file, &uploaded_books) {
                                Ok(_) => {
                                    println!("Marked upload progress book ID: {book_id} completed");
                                    info!("Marked upload progress book ID: {book_id} completed");
                                }
                                Err(err) => {
                                    println!("When marking upload progress book ID: {book_id}, encountered error {err:?}");
                                    error!("When marking upload progress book ID: {book_id}, encountered error {err:?}");
                                    continue;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        println!("Failed to upload book ID: {book_id} with error {err:?}");
                        info!("Failed to upload book ID: {book_id} with error {err:?}");
                    }
                }

                println!("Sleeping for 1 minutes...");
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
            Err(e) => {
                eprintln!("Failed to download book ID {book_id}: {e:?}");
            }
        }
    }

    Ok(())
}

#[cfg(feature = "initial-data")]
async fn mark_download_progress(
    book_id: u32,
    fpath: &Path,
    data: Vec<u8>,
    state: &mut State,
    state_file: &Path,
) -> Result<()> {
    let mut dest = fs::File::create(fpath).await?;
    dest.write_all(&data).await?;

    state.mark_seen(book_id);
    state.save_to_file(state_file)?;
    Ok(())
}

#[cfg(feature = "initial-data")]
fn mark_upload_progress(fpath: &Path, uploaded_books: &Vec<(String, String)>) -> Result<()> {
    let file = File::create(fpath)?;
    serde_json::to_writer(file, &uploaded_books)?;
    Ok(())
}

#[cfg(feature = "initial-data")]
async fn upload_downloaded_book(
    client: &Client,
    root_dir: &Path,
    file_path: PathBuf,
) -> Result<Vec<ChunkAddress>> {
    let upload_cfg = UploadCfg {
        batch_size: BATCH_SIZE,
        verify_store: true,
        retry_strategy: RetryStrategy::Quick,
        ..Default::default()
    };

    let files_uploader = FilesUploader::new(client.clone(), root_dir.to_path_buf())
        .set_make_data_public(true)
        .set_upload_cfg(upload_cfg)
        .insert_path(&file_path);

    let summary = match files_uploader.start_upload().await {
        Ok(summary) => summary,
        Err(err) => {
            println!("Failed to upload {file_path:?} with error {err:?}");
            return Ok(vec![]);
        }
    };

    info!(
        "File {file_path:?} uploaded completed with summary {:?}",
        summary.upload_summary
    );
    println!(
        "File {file_path:?} uploaded completed with summary {:?}",
        summary.upload_summary
    );

    let mut head_addresses = vec![];
    for (_, file_name, head_address) in summary.completed_files.iter() {
        info!(
            "Head address of {file_name:?} is {:?}",
            head_address.to_hex()
        );
        println!(
            "Head address of {file_name:?} is {:?}",
            head_address.to_hex()
        );
        head_addresses.push(*head_address);
    }

    Ok(head_addresses)
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

#[cfg(not(feature = "gifting"))]
#[allow(clippy::unused_async)]
async fn respond_to_gift_request(
    _client: Client,
    _key: String,
    _semaphore: Arc<Semaphore>,
) -> std::result::Result<impl Reply, std::convert::Infallible> {
    let mut response = Response::new("Gifting not enabled".to_string());
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;

    Ok(response)
}

#[cfg(feature = "gifting")]
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
