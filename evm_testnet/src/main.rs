// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use evmlib::common::{Address, Amount};
use evmlib::testnet::Testnet;
use evmlib::wallet::{balance_of_gas_tokens, balance_of_tokens, Wallet};
use std::str::FromStr;

/// A tool to start a local Ethereum node.
#[derive(Debug, Parser)]
#[clap(version, author, verbatim_doc_comment)]
struct Args {
    /// Wallet that will hold ~all gas funds and payment tokens.
    #[clap(long, short)]
    genesis_wallet: Option<Address>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    start_node(args.genesis_wallet).await;
}

async fn start_node(genesis_wallet: Option<Address>) {
    let testnet = Testnet::new().await;

    println!("*************************");
    println!("* Ethereum node started *");
    println!("*************************");

    // Transfer all gas and payment tokens to the genesis wallet.
    if let Some(genesis) = genesis_wallet {
        transfer_funds(&testnet, genesis).await;
    }

    print_testnet_details(&testnet, genesis_wallet).await;
    keep_alive(testnet).await;

    println!("Ethereum node stopped.");
}

async fn transfer_funds(testnet: &Testnet, genesis_wallet: Address) {
    let wallet =
        Wallet::new_from_private_key(testnet.to_network(), &testnet.default_wallet_private_key())
            .expect("Could not init deployer wallet");

    let token_amount = wallet
        .balance_of_tokens()
        .await
        .expect("Could not get balance of tokens");

    // Transfer all payment tokens.
    let _ = wallet.transfer_tokens(genesis_wallet, token_amount).await;

    let gas_amount = wallet
        .balance_of_gas_tokens()
        .await
        .expect("Could not get balance of gas tokens");

    let sub_amount = Amount::from_str("1000000000000000000").expect("Could not parse sub amount");

    // Transfer almost all gas. Save some gas for this tx.
    let _ = wallet
        .transfer_gas_tokens(genesis_wallet, gas_amount - sub_amount)
        .await;
}

async fn print_testnet_details(testnet: &Testnet, genesis_wallet: Option<Address>) {
    let network = testnet.to_network();

    println!("RPC URL: {}", network.rpc_url());
    println!("Payment token address: {}", network.payment_token_address());
    println!("Data payments address: {}", network.data_payments_address());
    println!(
        "Deployer wallet private key: {}",
        testnet.default_wallet_private_key()
    );

    if let Some(genesis) = genesis_wallet {
        let tokens = balance_of_tokens(genesis, &network)
            .await
            .unwrap_or(Amount::MIN);

        let gas = balance_of_gas_tokens(genesis, &network)
            .await
            .unwrap_or(Amount::MIN);

        println!("Genesis wallet balance (atto): (tokens: {tokens}, gas: {gas})");
    }
}

async fn keep_alive<T>(variable: T) {
    let _ = tokio::signal::ctrl_c().await;
    println!("Received Ctrl-C, stopping...");
    drop(variable);
}
