use super::wallet::send;
use crate::Client;

use sn_dbc::{Dbc, PublicAddress, Token};
use sn_transfers::dbc_genesis::{create_faucet_wallet, load_genesis_wallet};
use sn_transfers::wallet::{DepositWallet, LocalWallet, VerifyingClient, Wallet};

/// Returns a dbc with the requested number of tokens, for use by E2E test instances.
pub async fn get_tokens_from_faucet(amount: Token, to: PublicAddress, client: &Client) -> Dbc {
    send(load_faucet_wallet(client).await, amount, to, client).await
}

/// Use the client to load the faucet wallet from the genesis DBC.
pub async fn load_faucet_wallet(client: &Client) -> LocalWallet {
    let genesis_wallet = load_genesis_wallet().await;

    println!("Loading faucet...");
    let mut faucet_wallet = create_faucet_wallet().await;

    let faucet_balance = faucet_wallet.balance();
    if faucet_balance.as_nano() > 0 {
        println!("Faucet wallet balance: {faucet_balance}");
        return faucet_wallet;
    }

    // Transfer to faucet. We will transfer almost all of the genesis wallet's
    // balance to the faucet,.

    let faucet_balance = Token::from_nano(genesis_wallet.balance().as_nano());
    println!("Sending {faucet_balance} from genesis to faucet wallet..");
    let tokens = send(
        genesis_wallet,
        faucet_balance,
        faucet_wallet.address(),
        client,
    )
    .await;

    faucet_wallet.deposit(vec![tokens.clone()]);
    faucet_wallet
        .store()
        .await
        .expect("Faucet wallet shall be stored successfully.");
    println!("Faucet wallet balance: {}", faucet_wallet.balance());

    println!("Verifying the transfer from genesis...");
    if let Err(error) = client.verify(&tokens).await {
        println!("Could not verify the transfer from genesis: {error:?}");
    }

    faucet_wallet
}
