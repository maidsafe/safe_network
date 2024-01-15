// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use crate::common::{
    client::{get_all_rpc_addresses, get_gossip_client_and_wallet},
    get_safenode_rpc_client, random_content,
};
use assert_fs::TempDir;
use bls::{PublicKey, SecretKey, PK_SIZE};
use eyre::{eyre, Result};
use sn_client::{Client, ClientEvent, FilesUpload, WalletClient};
use sn_logging::LogBuilder;
use sn_node::{NodeEvent, ROYALTY_TRANSFER_NOTIF_TOPIC};
use sn_protocol::safenode_proto::{
    GossipsubSubscribeRequest, NodeEventsRequest, TransferNotifsFilterRequest,
};
use sn_registers::Permissions;
use sn_transfers::{
    CashNoteRedemption, LocalWallet, MainSecretKey, NanoTokens, NETWORK_ROYALTIES_PK,
};
use std::net::SocketAddr;
use tokio::{
    task::JoinHandle,
    time::{sleep, timeout, Duration},
};
use tokio_stream::StreamExt;
use tonic::Request;
use xor_name::XorName;

#[tokio::test]
async fn nodes_rewards_for_storing_chunks() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_000_333;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, _paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let (files_api, _content_bytes, _content_addr, chunks) =
        random_content(&client, paying_wallet_dir.to_path_buf(), chunks_dir.path())?;

    let chunks_len = chunks.len();
    let prev_rewards_balance = current_rewards_balance()?;
    println!("With {prev_rewards_balance:?} current balance, paying for {} random addresses... {chunks:?}", chunks.len());

    let mut files_upload = FilesUpload::new(files_api.clone()).set_show_holders(true);
    files_upload.upload_chunks(chunks).await?;
    let storage_cost = files_upload.get_upload_storage_cost();

    println!("Paid {storage_cost:?} total rewards for the chunks");

    verify_rewards(prev_rewards_balance, storage_cost, chunks_len).await?;

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_for_storing_registers() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_000_444;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let rb = current_rewards_balance()?;

    let mut rng = rand::thread_rng();
    let register_addr = XorName::random(&mut rng);

    println!("Paying for random Register address {register_addr:?} with current balance {rb:?}");

    let prev_rewards_balance = current_rewards_balance()?;

    let (_register, storage_cost, _royalties_fees) = client
        .create_and_pay_for_register(
            register_addr,
            &mut wallet_client,
            false,
            Permissions::new_owner_only(),
        )
        .await?;
    println!("Cost is {storage_cost:?}: {prev_rewards_balance:?}");

    verify_rewards(prev_rewards_balance, storage_cost, 1).await?;

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_for_chunks_notifs_over_gossipsub() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_111_000;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, _paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let (files_api, _content_bytes, _content_addr, chunks) =
        random_content(&client, paying_wallet_dir.to_path_buf(), chunks_dir.path())?;

    let num_of_chunks = chunks.len();
    let handle = spawn_royalties_payment_client_listener(client.clone(), num_of_chunks).await?;

    let num_of_chunks = chunks.len();

    tracing::info!("Paying for {num_of_chunks} random addresses...");
    println!("Paying for {num_of_chunks} random addresses...");
    let mut files_upload = FilesUpload::new(files_api.clone()).set_show_holders(true);
    files_upload.upload_chunks(chunks).await?;
    let storage_cost = files_upload.get_upload_storage_cost();
    let royalties_fees = files_upload.get_upload_royalty_fees();

    println!("Random chunks stored, paid {storage_cost}/{royalties_fees}");

    let (count, amount) = handle.await??;

    println!("Number of notifications received: {count}");
    println!("Amount notified for royalties fees: {amount}");
    assert_eq!(
        amount, royalties_fees,
        "Unexpected amount of royalties fees received"
    );
    assert!(
        count >= num_of_chunks,
        "Unexpected number of royalties fees notifications received"
    );

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_for_register_notifs_over_gossipsub() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_000_222_000;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let mut rng = rand::thread_rng();
    let register_addr = XorName::random(&mut rng);

    let handle = spawn_royalties_payment_client_listener(client.clone(), 1).await?;

    println!("Paying for random Register address {register_addr:?} ...");
    let (_, storage_cost, royalties_fees) = client
        .create_and_pay_for_register(
            register_addr,
            &mut wallet_client,
            false,
            Permissions::new_owner_only(),
        )
        .await?;
    println!("Random Register created, paid {storage_cost}/{royalties_fees}");

    let (count, amount) = handle.await??;
    println!("Number of notifications received: {count}");
    println!("Amount notified for royalties fees: {amount}");
    assert_eq!(
        amount, royalties_fees,
        "Unexpected amount of royalties fees received"
    );
    assert_eq!(
        count, 1,
        "Unexpected number of royalties fees notifications received"
    );

    Ok(())
}

#[tokio::test]
async fn nodes_rewards_transfer_notifs_filter() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("nodes_rewards");

    let paying_wallet_balance = 10_000_111_111_000;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, _paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let (files_api, _content_bytes, _content_addr, chunks) =
        random_content(&client, paying_wallet_dir.to_path_buf(), chunks_dir.path())?;
    let node_rpc_addresses = get_all_rpc_addresses(false)?;

    // this node shall receive the notifications since we set the correct royalties pk as filter
    let royalties_pk = NETWORK_ROYALTIES_PK.public_key();
    let handle_1 = spawn_royalties_payment_listener(
        node_rpc_addresses[0],
        royalties_pk,
        true,
        chunks.len(),
        false,
    )
    .await;
    // this other node shall *not* receive any notification since we set the wrong pk as filter
    let random_pk = SecretKey::random().public_key();
    let handle_2 =
        spawn_royalties_payment_listener(node_rpc_addresses[1], random_pk, true, 0, false).await;
    // this other node shall *not* receive any notification either since we don't set any pk as filter
    let handle_3 =
        spawn_royalties_payment_listener(node_rpc_addresses[2], royalties_pk, false, 0, true).await;

    let num_of_chunks = chunks.len();
    println!("Paying for {num_of_chunks} chunks");
    let mut files_upload = FilesUpload::new(files_api.clone()).set_show_holders(true);
    files_upload.upload_chunks(chunks).await?;
    let storage_cost = files_upload.get_upload_storage_cost();
    let royalties_fees = files_upload.get_upload_royalty_fees();

    println!("Random chunks stored, paid {storage_cost}/{royalties_fees}");

    let count_1 = handle_1.await??;
    println!("Number of notifications received by node #1: {count_1}");
    let count_2 = handle_2.await??;
    println!("Number of notifications received by node #2: {count_2}");
    let count_3 = handle_3.await??;
    println!("Number of notifications received by node #3: {count_3}");

    assert!(
        count_1 >= num_of_chunks,
        "expected: {num_of_chunks:?}, received {count_1:?}... Not enough notifications received"
    );
    assert_eq!(count_2, 0, "Notifications were not expected");
    assert_eq!(count_3, 0, "Notifications were not expected");

    Ok(())
}

async fn verify_rewards(
    prev_rewards_balance: NanoTokens,
    rewards_paid: NanoTokens,
    put_record_count: usize,
) -> Result<()> {
    let expected_rewards_balance = prev_rewards_balance
        .checked_add(rewards_paid)
        .ok_or_else(|| eyre!("Failed to sum up rewards balance"))?;

    let mut iteration = 0;
    let mut cur_rewards_history = Vec::new();

    // An initial sleep to avoid access to allow for reward receipts to be processed
    sleep(Duration::from_secs(20)).await;

    while iteration < put_record_count {
        iteration += 1;
        println!("Current iteration {iteration}");
        let new_rewards_balance = current_rewards_balance()?;
        if expected_rewards_balance == new_rewards_balance {
            return Ok(());
        }
        cur_rewards_history.push(new_rewards_balance);
        sleep(Duration::from_secs(10)).await;
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

        total_rewards = total_rewards
            .checked_add(balance)
            .ok_or_else(|| eyre!("Faied to sum up rewards balance"))?;
    }

    println!("Current total balance is {total_rewards:?}");

    Ok(total_rewards)
}

async fn spawn_royalties_payment_listener(
    rpc_addr: SocketAddr,
    royalties_pk: PublicKey,
    set_filter: bool,
    expected_royalties: usize,
    need_extra_wait: bool,
) -> JoinHandle<Result<usize, eyre::Report>> {
    let handle = tokio::spawn(async move {
        let mut rpc_client = get_safenode_rpc_client(rpc_addr).await?;

        if set_filter {
            let _ = rpc_client
                .transfer_notifs_filter(Request::new(TransferNotifsFilterRequest {
                    pk: royalties_pk.to_bytes().to_vec(),
                }))
                .await?;
        }

        let _ = rpc_client
            .subscribe_to_topic(Request::new(GossipsubSubscribeRequest {
                topic: ROYALTY_TRANSFER_NOTIF_TOPIC.to_string(),
            }))
            .await?;

        let response = rpc_client
            .node_events(Request::new(NodeEventsRequest {}))
            .await?;

        let mut count = 0;
        let mut stream = response.into_inner();

        // if expected royalties is 0 or 1 we'll wait for 20s as a minimum,
        // otherwise we'll wait for 10s per expected royalty
        let secs = std::cmp::max(40, expected_royalties as u64 * 15);

        let duration = Duration::from_secs(secs);
        println!("Awaiting transfers notifs for {duration:?}...");
        if timeout(duration, async {
            while let Some(Ok(e)) = stream.next().await {
                match NodeEvent::from_bytes(&e.event) {
                    Ok(NodeEvent::TransferNotif { key, .. }) => {
                        println!("Transfer notif received for key {key:?}");
                        if key == royalties_pk {
                            count += 1;
                            println!("Received {count} royalty notifs so far");
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
    });

    // small wait to ensure that the gossipsub subscription is in place
    if need_extra_wait {
        sleep(Duration::from_secs(20)).await;
    }

    handle
}

async fn spawn_royalties_payment_client_listener(
    client: Client,
    expected_royalties: usize,
) -> Result<JoinHandle<Result<(usize, NanoTokens), eyre::Report>>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let sk = SecretKey::from_hex(sn_transfers::GENESIS_CASHNOTE_SK)?;
    let mut wallet = LocalWallet::load_from_path(&temp_dir, Some(MainSecretKey::new(sk)))?;
    let royalties_pk = NETWORK_ROYALTIES_PK.public_key();
    client.subscribe_to_topic(ROYALTY_TRANSFER_NOTIF_TOPIC.to_string())?;

    let mut events_receiver = client.events_channel();

    let handle = tokio::spawn(async move {
        let mut count = 0;

        // if expected royalties is 0 or 1 we'll wait for 20s as a minimum,
        // otherwise we'll wait for 10s per expected royalty
        let secs = std::cmp::max(40, expected_royalties as u64 * 15);
        let duration = Duration::from_secs(secs);
        tracing::info!("Awaiting transfers notifs for {duration:?}...");
        println!("Awaiting transfers notifs for {duration:?}...");
        if timeout(duration, async {
            while let Ok(event) = events_receiver.recv().await {
                let cashnote_redemptions = match event {
                    ClientEvent::GossipsubMsg { topic, msg } => {
                        // we assume it's a notification of a transfer as that's the only topic we've subscribed to
                        match try_decode_transfer_notif(&msg) {
                            Ok((key, cashnote_redemptions)) => {
                                println!("Transfer notif received for key {key:?}");
                                if key != royalties_pk {
                                    continue;
                                }
                                count += 1;
                                cashnote_redemptions
                            }
                            Err(err) => {
                                println!("GossipsubMsg received on topic '{topic}' couldn't be decoded as transfer notif: {err:?}");
                                continue;
                            },
                        }
                    },
                    _ => continue
                };

                match client
                    .verify_cash_notes_redemptions(wallet.address(), &cashnote_redemptions)
                    .await
                {
                    Ok(cash_notes) => if let Err(err) = wallet.deposit(&cash_notes) {
                        println!("Failed to deposit cash notes: {err}");
                    }
                    Err(err) => println!("At least one of the CashNoteRedemptions received is invalid, dropping them: {err:?}")
                }
            }
        })
        .await
        .is_err()
        {
            println!("Timeout after {duration:?}.");
        }

        Ok((count, wallet.balance()))
    });

    // small wait to ensure that the gossipsub subscription is in place
    sleep(Duration::from_secs(20)).await;

    Ok(handle)
}

fn try_decode_transfer_notif(msg: &[u8]) -> eyre::Result<(PublicKey, Vec<CashNoteRedemption>)> {
    let mut key_bytes = [0u8; PK_SIZE];
    key_bytes.copy_from_slice(
        msg.get(0..PK_SIZE)
            .ok_or_else(|| eyre::eyre!("msg doesn't have enough bytes"))?,
    );
    let key = PublicKey::from_bytes(key_bytes)?;
    let cashnote_redemptions: Vec<CashNoteRedemption> = rmp_serde::from_slice(&msg[PK_SIZE..])?;
    Ok((key, cashnote_redemptions))
}
