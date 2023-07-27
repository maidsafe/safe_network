use std::time::Duration;

use super::wallet::send;
use crate::Client;

use sn_dbc::{Dbc, PublicAddress, Token};
use sn_transfers::dbc_genesis::{create_faucet_wallet, load_genesis_wallet};
use sn_transfers::wallet::LocalWallet;

/// Returns a dbc with the requested number of tokens, for use by E2E test instances.
pub async fn get_tokens_from_faucet(amount: Token, to: PublicAddress, client: &Client) -> Dbc {
    send(
        load_faucet_wallet_from_genesis_wallet(client).await,
        amount,
        to,
        client,
    )
    .await
}

/// Use the client to load the faucet wallet from the genesis Wallet.
/// With all balance transferred from the genesis_wallet to the faucet_wallet.
pub async fn load_faucet_wallet_from_genesis_wallet(client: &Client) -> LocalWallet {
    println!("Loading genesis...");
    let genesis_wallet = load_genesis_wallet().await;

    println!("Loading faucet...");
    let mut faucet_wallet = create_faucet_wallet().await;

    let faucet_balance = faucet_wallet.balance();
    if !faucet_balance.is_zero() {
        println!("Faucet wallet balance: {faucet_balance}");
        return faucet_wallet;
    }

    // Transfer to faucet. We will transfer almost all of the genesis wallet's
    // balance to the faucet,.

    let faucet_balance = genesis_wallet.balance();
    println!("Sending {faucet_balance} from genesis to faucet wallet..");
    let dbc = send(
        genesis_wallet,
        faucet_balance,
        faucet_wallet.address(),
        client,
    )
    .await;

    faucet_wallet.deposit(vec![dbc.clone()]);
    faucet_wallet
        .store()
        .await
        .expect("Faucet wallet shall be stored successfully.");
    println!("Faucet wallet balance: {}", faucet_wallet.balance());

    println!("Verifying the transfer from genesis...");
    if let Err(error) = client.verify(&dbc).await {
        println!("Could not verify the transfer from genesis, retrying after 20 secs...");
        println!("The error was: {error:?}");
        tokio::time::sleep(Duration::from_secs(20)).await;
        if let Err(error) = client.verify(&dbc).await {
            println!("Could not verify the transfer from genesis: {error:?}");
        } else {
            println!("Successfully verified the transfer from genesis on the second try.");
        }
    }

    faucet_wallet
}
