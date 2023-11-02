// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use crate::common::{
    get_client_and_wallet, random_content,
    safenode_proto::{
        safe_node_client::SafeNodeClient, GossipsubSubscribeRequest, NodeEventsRequest,
    },
};
use sn_client::WalletClient;
use sn_logging::LogBuilder;
use sn_networking::CLOSE_GROUP_SIZE;
use sn_node::NodeEvent;
use sn_transfers::{
    LocalWallet, NanoTokens, NETWORK_ROYALTIES_AMOUNT_PER_ADDR, NETWORK_ROYALTIES_PK,
};
use xor_name::XorName;

use assert_fs::TempDir;
use eyre::{eyre, Result};
use tokio::{
    task::JoinHandle,
    time::{sleep, Duration},
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
        .pay_and_upload_bytes_test(*content_addr.xorname(), chunks)
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

    println!("Paying for {} random addresses...", chunks.len());

    let handle = spawn_royalties_payment_listener("https://127.0.0.1:12001".to_string());

    let _cost = files_api
        .pay_and_upload_bytes_test(*content_addr.xorname(), chunks)
        .await?;

    println!("Random chunks stored");

    let count = handle.await??;
    println!("Number of notifications received by node: {count}");
    assert_eq!(count, CLOSE_GROUP_SIZE, "Not enough notifications received");

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

    println!("Paying for random Register address {register_addr:?} ...");

    let handle = spawn_royalties_payment_listener("https://127.0.0.1:12001".to_string());

    let _register = client
        .create_and_pay_for_register(register_addr, &mut wallet_client, true)
        .await?;
    println!("Random Register created");

    let count = handle.await??;
    println!("Number of notifications received by node: {count}");
    assert_eq!(count, CLOSE_GROUP_SIZE, "Not enough notifications received");

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

fn spawn_royalties_payment_listener(endpoint: String) -> JoinHandle<Result<usize, eyre::Report>> {
    tokio::spawn(async move {
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

        for group_index in 0..256 {
            let royalty_topic: &str = "ROYALTY_TRANSFER";
            let topic = format!("{royalty_topic}_GROUP_{group_index}");
            let _response = rpc_client
                .subscribe_to_topic(Request::new(GossipsubSubscribeRequest { topic }))
                .await?;
        }

        let response = rpc_client
            .node_events(Request::new(NodeEventsRequest {}))
            .await?;

        let mut count = 0;
        let mut stream = response.into_inner();
        let royalties_pk = NETWORK_ROYALTIES_PK.public_key();

        println!("Awaiting transfers notifs...");
        while let Some(Ok(e)) = stream.next().await {
            match NodeEvent::from_bytes(&e.event) {
                Ok(NodeEvent::TransferNotif { key, transfers: _ }) => {
                    println!("Transfer notif received for key {key:?}");
                    if key == royalties_pk {
                        count += 1;
                        if count == CLOSE_GROUP_SIZE {
                            break;
                        }
                    }
                }
                Ok(_) => { /* ignored */ }
                Err(_) => {
                    println!("Error while parsing received NodeEvent");
                }
            }
        }

        Ok(count)
    })
}
