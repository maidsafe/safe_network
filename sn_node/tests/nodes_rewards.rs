// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use crate::common::{get_client_and_wallet, random_content};
use sn_client::{Client, ClientEvent, WalletClient};
use sn_logging::LogBuilder;
use sn_node::{NodeEvent, TRANSFER_NOTIF_TOPIC};
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, NodeEventsRequest, TransferNotifsFilterRequest,
};
use sn_transfers::{
    CashNoteRedemption, LocalWallet, NanoTokens, NETWORK_ROYALTIES_AMOUNT_PER_ADDR,
    NETWORK_ROYALTIES_PK,
};
use xor_name::XorName;

use assert_fs::TempDir;
use bls::{PublicKey, SecretKey, PK_SIZE};
use eyre::{eyre, Result};
use tokio::{
    task::JoinHandle,
    time::{sleep, timeout, Duration},
};
use tokio_stream::StreamExt;
use tonic::Request;

#[tokio::test]
async fn nodes_rewards_for_storing_chunks() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_000_333;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, _paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let (files_api, _content_bytes, content_addr, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    let prev_rewards_balance = current_rewards_balance()?;
    println!("With {prev_rewards_balance:?} current balance, paying for {} random addresses... {chunks:?}", chunks.len());

    let (_file_addr, rewards_paid, _royalties_fees) = files_api
        .pay_and_upload_bytes_test(*content_addr.xorname(), chunks, true)
        .await?;

    println!("Paid {rewards_paid:?} total rewards for the chunks");

    let expected_rewards_balance = prev_rewards_balance
        .checked_add(rewards_paid)
        .ok_or_else(|| eyre!("Failed to sum up rewards balance"))?;

    verify_rewards(expected_rewards_balance).await?;

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_for_storing_registers() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_000_444;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let rb = current_rewards_balance()?;

    let mut rng = rand::thread_rng();
    let register_addr = XorName::random(&mut rng);
    let expected_royalties_fees = NETWORK_ROYALTIES_AMOUNT_PER_ADDR; // fee for a single address

    println!("Paying for random Register address {register_addr:?} with current balance {rb:?}");

    let prev_rewards_balance = current_rewards_balance()?;

    let (_register, cost) = client
        .create_and_pay_for_register(register_addr, &mut wallet_client, false)
        .await?;
    println!("Cost is {cost:?}: {prev_rewards_balance:?}");
    let rewards_paid = cost
        .checked_sub(expected_royalties_fees)
        .ok_or_else(|| eyre!("Failed to substract rewards balance"))?;

    let expected_rewards_balance = prev_rewards_balance
        .checked_add(rewards_paid)
        .ok_or_else(|| eyre!("Failed to sum up rewards balance"))?;

    verify_rewards(expected_rewards_balance).await?;

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_for_chunks_notifs_over_gossipsub() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_111_000;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, _paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let (files_api, _content_bytes, content_addr, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    let handle = spawn_royalties_payment_client_listener(&client)?;

    let num_of_chunks = chunks.len();
    println!("Paying for {num_of_chunks} random addresses...");
    let (_, storage_cost, royalties_cost) = files_api
        .pay_and_upload_bytes_test(*content_addr.xorname(), chunks, false)
        .await?;

    println!("Random chunks stored, paid {storage_cost}/{royalties_cost}");

    let count = handle.await??;
    let expected = royalties_cost.as_nano() as usize;

    println!("Number of notifications received by node: {count}");
    assert_eq!(
        count, expected,
        "Unexpected number of notifications received"
    );

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_for_register_notifs_over_gossipsub() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_222_000;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let mut rng = rand::thread_rng();
    let register_addr = XorName::random(&mut rng);

    let handle = spawn_royalties_payment_client_listener(&client)?;

    println!("Paying for random Register address {register_addr:?} ...");
    let (_, cost) = client
        .create_and_pay_for_register(register_addr, &mut wallet_client, false)
        .await?;
    println!("Random Register created, paid {cost}");

    let count = handle.await??;
    println!("Number of notifications received by node: {count}");
    assert_eq!(count, 1, "Unexpected number of notifications received");

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_transfer_notifs_filter() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_111_111_000;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, _paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let (files_api, _content_bytes, content_addr, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    // this node shall receive the notifications since we set the correct royalties pk as filter
    let royalties_pk = NETWORK_ROYALTIES_PK.public_key();
    let handle_1 =
        spawn_royalties_payment_listener("https://127.0.0.1:12001".to_string(), royalties_pk, true);
    // this other node shall *not* receive any notification since we set the wrong pk as filter
    let random_pk = SecretKey::random().public_key();
    let handle_2 =
        spawn_royalties_payment_listener("https://127.0.0.1:12002".to_string(), random_pk, true);
    // this other node shall *not* receive any notification either since we don't set any pk as filter
    let handle_3 = spawn_royalties_payment_listener(
        "https://127.0.0.1:12003".to_string(),
        royalties_pk,
        false,
    );

    let num_of_chunks = chunks.len();
    println!("Paying for {num_of_chunks} random addresses...");
    let (_, storage_cost, royalties_cost) = files_api
        .pay_and_upload_bytes_test(*content_addr.xorname(), chunks, false)
        .await?;
    println!("Random chunks stored, paid {storage_cost}/{royalties_cost}");

    let count_1 = handle_1.await??;
    let expected = royalties_cost.as_nano() as usize;
    println!("Number of notifications received by node #1: {count_1}");
    let count_2 = handle_2.await??;
    println!("Number of notifications received by node #2: {count_2}");
    let count_3 = handle_3.await??;
    println!("Number of notifications received by node #3: {count_3}");

    assert_eq!(count_1, 1, "Not enough notifications received");
    assert_eq!(count_2, 0, "Notifications were not expected");
    assert_eq!(count_3, 0, "Notifications were not expected");

    Ok(())
}

async fn verify_rewards(expected_rewards_balance: NanoTokens) -> Result<()> {
    let mut iteration = 0;
    let mut cur_rewards_history = Vec::new();

    // An initial sleep to avoid access to the wallet file synced with the node operations.
    // Ideally, there shall be wallet file locker to prevent handle multiple processes access.
    sleep(Duration::from_secs(5)).await;

    while iteration < 15 {
        iteration += 1;
        println!("Current iteration {iteration}");
        let new_rewards_balance = current_rewards_balance()?;
        if expected_rewards_balance == new_rewards_balance {
            return Ok(());
        }
        cur_rewards_history.push(new_rewards_balance);
        sleep(Duration::from_secs(2)).await;
    }

    Err(eyre!("Network doesn't get expected reward {expected_rewards_balance:?} after {iteration} iterations, history is {cur_rewards_history:?}"))
}

// Helper which reads all nodes local wallets returning the total balance
fn current_rewards_balance() -> Result<NanoTokens> {
    let mut total_rewards = NanoTokens::zero();
    let node_dir_path = dirs_next::data_dir()
        .ok_or_else(|| eyre!("Failed to obtain data directory path"))?
        .join("safe")
        .join("node");

    for entry in std::fs::read_dir(node_dir_path)? {
        let path = entry?.path();
        let wallet = LocalWallet::try_load_from(&path)?;
        let balance = wallet.balance();
        println!("Node's wallet {path:?} currently have balance of {balance:?}");
        total_rewards = total_rewards
            .checked_add(balance)
            .ok_or_else(|| eyre!("Faied to sum up rewards balance"))?;
    }

    println!("Current total balance is {total_rewards:?}");

    Ok(total_rewards)
}

fn spawn_royalties_payment_listener(
    endpoint: String,
    royalties_pk: PublicKey,
    set_fiter: bool,
) -> JoinHandle<Result<usize, eyre::Report>> {
    tokio::spawn(async move {
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
        if set_fiter {
            let _ = rpc_client
                .transfer_notifs_filter(Request::new(TransferNotifsFilterRequest {
                    pk: royalties_pk.to_bytes().to_vec(),
                }))
                .await?;
        }
        let response = rpc_client
            .node_events(Request::new(NodeEventsRequest {}))
            .await?;

        let mut count = 0;
        let mut stream = response.into_inner();

        let duration = Duration::from_secs(10);
        println!("Awaiting transfers notifs for {duration:?}...");
        if timeout(duration, async {
            while let Some(Ok(e)) = stream.next().await {
                match NodeEvent::from_bytes(&e.event) {
                    Ok(NodeEvent::TransferNotif { key, .. }) => {
                        println!("Transfer notif received for key {key:?}");
                        if key == royalties_pk {
                            count += 1;
                            // if count == 1 {
                            //     break;
                            // }
                        }
                    }
                    Ok(_) => { /* ignored */ }
                    Err(_) => {
                        println!("Error while parsing received NodeEvent");
                    }
                }
            }
        })
        .await
        .is_err()
        {
            println!("Timeout after {duration:?}.");
        }

        Ok(count)
    })
}

fn spawn_royalties_payment_client_listener(
    client: &Client,
) -> Result<JoinHandle<Result<usize, eyre::Report>>> {
    let royalties_pk = NETWORK_ROYALTIES_PK.public_key();
    client.subscribe_to_topic(TRANSFER_NOTIF_TOPIC.to_string())?;
    let mut events_receiver = client.events_channel();

    let handle = tokio::spawn(async move {
        let mut count = 0;

        let duration = Duration::from_secs(10);
        println!("Awaiting transfers notifs for {duration:?}...");
        if timeout(duration, async {
            while let Ok(event) = events_receiver.recv().await {
                match event {
                    ClientEvent::GossipsubMsg { topic, msg } => {
                        // we assume it's a notification of a transfer as that's the only topic we've subscribed to
                        match try_decode_transfer_notif(&msg) {
                            Ok((key, _)) => {
                                println!("Transfer notif received for key {key:?}");
                                if key == royalties_pk {
                                    count += 1;
                                }
                            }
                            Err(err) => println!("GossipsubMsg received on topic '{topic}' couldn't be decoded as transfer notif: {err:?}"),
                        }
                    },
                    _ => { /* ignored */ }
                }
            }
        })
        .await
        .is_err()
        {
            println!("Timeout after {duration:?}.");
        }

        Ok(count)
    });

    Ok(handle)
}

fn try_decode_transfer_notif(msg: &[u8]) -> eyre::Result<(PublicKey, Vec<CashNoteRedemption>)> {
    let mut key_bytes = [0u8; PK_SIZE];
    key_bytes.copy_from_slice(
        msg.get(0..PK_SIZE)
            .ok_or_else(|| eyre::eyre!("msg doesn't have enough bytes"))?,
    );
    let key = PublicKey::from_bytes(key_bytes)?;
    let cashnote_redemptions: Vec<CashNoteRedemption> = bincode::deserialize(&msg[PK_SIZE..])?;
    Ok((key, cashnote_redemptions))
}
