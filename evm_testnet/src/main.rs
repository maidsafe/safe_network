// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use evmlib::common::Address;
use evmlib::testnet::Testnet;

/// A tool to start a local Ethereum node.
#[derive(Debug, Parser)]
#[clap(version, author, verbatim_doc_comment)]
struct Args {
    /// Address that will receive the chunk payments royalties.
    #[clap(long, short)]
    royalties_wallet: Address,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    start_node(args.royalties_wallet).await;
}

async fn start_node(royalties_wallet: Address) {
    let testnet = Testnet::new(royalties_wallet).await;
    println!("*************************");
    println!("* Ethereum node started *");
    println!("*************************");
    print_testnet_details(&testnet);
    keep_alive(testnet).await;
    println!("Ethereum node stopped.");
}

fn print_testnet_details(testnet: &Testnet) {
    let network = testnet.to_network();
    println!("RPC URL: {}", network.rpc_url());
    println!("Payment token address: {}", network.payment_token_address());
    println!(
        "Chunk payments address: {}",
        network.chunk_payments_address()
    );
}

async fn keep_alive<T>(variable: T) {
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Received Ctrl-C, stopping...");
                break;
            }
        }
    }

    drop(variable);
}
