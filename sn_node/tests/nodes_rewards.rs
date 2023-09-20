// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use common::{get_client_and_wallet, random_content};

use sn_client::WalletClient;
use sn_protocol::{
    storage::{ChunkAddress, RegisterAddress},
    NetworkAddress,
};
use sn_transfers::wallet::LocalWallet;
use sn_transfers::NanoTokens;
use xor_name::XorName;

use assert_fs::TempDir;
use eyre::{eyre, Result};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn nodes_rewards_for_storing_chunks() -> Result<()> {
    let paying_wallet_balance = 10_000_000_000_333;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, content_bytes, _content_addr, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    println!("Paying for {} random addresses...", chunks.len());

    let cost = wallet_client
        .pay_for_storage(
            chunks
                .into_iter()
                .map(|(name, _)| NetworkAddress::ChunkAddress(ChunkAddress::new(name))),
            true,
        )
        .await?;

    let prev_rewards_balance = current_rewards_balance()?;
    let expected_rewards_balance = prev_rewards_balance
        .checked_add(cost)
        .ok_or_else(|| eyre!("Failed to sum up rewards balance"))?;

    files_api.upload_with_payments(content_bytes, true).await?;

    verify_rewards(expected_rewards_balance).await?;

    Ok(())
}

async fn verify_rewards(expected_rewards_balance: Token) -> Result<()> {
    let mut iteration = 0;
    let mut cur_rewards_history = Vec::new();

    while iteration < 10 {
        iteration += 1;
        let new_rewards_balance = current_rewards_balance()?;
        if expected_rewards_balance == new_rewards_balance {
            return Ok(());
        }
        cur_rewards_history.push(new_rewards_balance);
        sleep(Duration::from_secs(1)).await;
    }
    panic!("Network doesn't get expected reward {expected_rewards_balance:?} after {iteration} iterations, history is {cur_rewards_history:?}");
}

#[tokio::test]
async fn nodes_rewards_for_storing_registers() -> Result<()> {
    let paying_wallet_balance = 10_000_000_000_444;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    println!("Paying for random Register address...");
    let mut rng = rand::thread_rng();
    let owner_pk = client.signer_pk();
    let register_addr = XorName::random(&mut rng);

    let cost = wallet_client
        .pay_for_storage(
            std::iter::once(NetworkAddress::RegisterAddress(RegisterAddress::new(
                register_addr,
                owner_pk,
            ))),
            true,
        )
        .await?;

    let prev_rewards_balance = current_rewards_balance()?;
    let expected_rewards_balance = prev_rewards_balance
        .checked_add(cost)
        .ok_or_else(|| eyre!("Failed to sum up rewards balance"))?;

    let _register = client
        .create_register(register_addr, &mut wallet_client, false)
        .await?;

    verify_rewards(expected_rewards_balance).await?;

    Ok(())
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

    Ok(total_rewards)
}
