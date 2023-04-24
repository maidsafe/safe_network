// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(unused)]

use crate::{
    client::{Client, WalletClient},
    domain::{
        dbc_genesis::{create_genesis, GENESIS_DBC_SK},
        wallet::{DepositWallet, LocalWallet, Wallet},
    },
};

use sn_dbc::{Dbc, MainKey, PublicAddress, Token};

use std::{
    path::{Path, PathBuf},
    sync::Once,
};

// Initialise faucet for tests, this is run only once, even if called multiple times.
fn init() {
    static INIT_LOGGER: Once = Once::new();
    INIT_LOGGER.call_once(|| {
        futures::executor::block_on(init_faucet());
    });
}

pub(super) fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();
    Client::new(secret_key).expect("Client shall be successfully created.")
}

pub(super) async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
}

/// Returns a dbc with the requested number of tokens, for use by E2E test instances.
pub(super) async fn get_tokens_from_faucet(
    amount: Token,
    to: PublicAddress,
    client: &Client,
) -> Dbc {
    init();
    send(faucet_wallet().await, amount, to, client).await
}

pub(super) async fn send(
    from: LocalWallet,
    amount: Token,
    to: PublicAddress,
    client: &Client,
) -> Dbc {
    if amount.as_nano() == 0 {
        panic!("Amount must be more than zero.");
    }

    let mut wallet_client = WalletClient::new(client.clone(), from);
    let new_dbc = wallet_client
        .send(amount, to)
        .await
        .expect("Tokens shall be successfully sent.");

    let mut wallet = wallet_client.into_wallet();
    wallet
        .store()
        .await
        .expect("Wallet shall be successfully stored.");
    wallet
        .store_created_dbc(new_dbc.clone())
        .await
        .expect("Created dbc shall be successfully stored.");

    new_dbc
}

async fn faucet_wallet() -> LocalWallet {
    let root_dir = get_faucet_dir().await;
    LocalWallet::load_from(&root_dir)
        .await
        .expect("Faucet wallet shall be created successfully.")
}

async fn init_faucet() {
    println!("Creating genesis...");
    let genesis = create_genesis().expect("Genesis shall be created successfully.");
    let mut genesis_wallet = genesis_wallet().await;
    genesis_wallet.deposit(vec![genesis]);
    genesis_wallet
        .store()
        .await
        .expect("Genesis wallet shall be stored successfully.");
    let genesis_balance = genesis_wallet.balance();
    println!("Genesis wallet balance: {genesis_balance}");

    // Transfer to faucet.

    let client = get_client();
    // As this will potentially be used by multiple test instances many many times over,
    // we'll only send a small amount.
    let faucet_balance = Token::from_nano(genesis_balance.as_nano() / 1000);
    let mut faucet_wallet = faucet_wallet().await;
    let tokens = send(
        genesis_wallet,
        faucet_balance,
        faucet_wallet.address(),
        &client,
    )
    .await;

    faucet_wallet.deposit(vec![tokens]);
    faucet_wallet
        .store()
        .await
        .expect("Faucet wallet shall be stored successfully.");
    println!("Faucet wallet balance: {}", faucet_wallet.balance());
}

async fn genesis_wallet() -> LocalWallet {
    let root_dir = get_genesis_dir().await;
    let wallet_dir = root_dir.join("wallet");
    tokio::fs::create_dir_all(&wallet_dir)
        .await
        .expect("Genesis wallet path to be successfully created.");

    let secret_key = bls::SecretKey::from_hex(GENESIS_DBC_SK)
        .expect("Genesis key hex shall be successfully parsed.");
    let main_key = MainKey::new(secret_key);
    let main_key_path = wallet_dir.join("main_key");
    tokio::fs::write(main_key_path, hex::encode(main_key.to_bytes()))
        .await
        .expect("Genesis key hex shall be successfully stored.");

    LocalWallet::load_from(&root_dir)
        .await
        .expect("Faucet wallet shall be created successfully.")
}

// We need deterministic and fix path for the faucet wallet.
// Otherwise the test instances will not be able to find the same faucet instance.
async fn get_faucet_dir() -> PathBuf {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("test_faucet");
    tokio::fs::create_dir_all(home_dirs.as_path())
        .await
        .expect("Faucet test path to be successfully created.");
    home_dirs
}

// We need deterministic and fix path for the faucet wallet.
// Otherwise the test instances will not be able to find the same faucet instance.
async fn get_genesis_dir() -> PathBuf {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("test_genesis");
    tokio::fs::create_dir_all(home_dirs.as_path())
        .await
        .expect("Genesis test path to be successfully created.");
    home_dirs
}
