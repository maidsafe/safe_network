// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_client::{get_tokens_from_faucet, Client};
use sn_transfers::wallet::LocalWallet;

use eyre::Result;
use sn_dbc::Token;
use std::path::Path;

pub async fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();
    Client::new(secret_key, None, None)
        .await
        .expect("Client shall be successfully created.")
}

pub async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
}

pub async fn get_client_and_wallet(root_dir: &Path, amount: u64) -> Result<(Client, LocalWallet)> {
    let wallet_balance = Token::from_nano(amount);

    let mut local_wallet = get_wallet(root_dir).await;
    let client = get_client().await;
    println!("Getting {wallet_balance} tokens from the faucet...");
    let tokens = get_tokens_from_faucet(wallet_balance, local_wallet.address(), &client).await;
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!("Verifying the transfer from faucet...");
    client.verify(&tokens).await?;
    local_wallet.deposit(vec![tokens]);
    assert_eq!(local_wallet.balance(), wallet_balance);
    println!("Tokens deposited to the wallet that'll pay for storage: {wallet_balance}.");

    Ok((client, local_wallet))
}
