use super::{wallet::send, Result};
use crate::Client;

use sn_transfers::LocalWallet;
use sn_transfers::{create_faucet_wallet, load_genesis_wallet};
use sn_transfers::{CashNote, MainPubkey, NanoTokens};

/// Returns a cash_note with the requested number of tokens, for use by E2E test instances.
/// Note this will create a faucet having a Genesis balance
pub async fn get_tokens_from_faucet(
    amount: NanoTokens,
    to: MainPubkey,
    client: &Client,
) -> Result<CashNote> {
    send(
        load_faucet_wallet_from_genesis_wallet(client).await?,
        amount,
        to,
        client,
        // we should not need to wait for this
        true,
    )
    .await
}

/// Use the client to load the faucet wallet from the genesis Wallet.
/// With all balance transferred from the genesis_wallet to the faucet_wallet.
pub async fn load_faucet_wallet_from_genesis_wallet(client: &Client) -> Result<LocalWallet> {
    println!("Loading faucet...");
    info!("Loading faucet...");
    let mut faucet_wallet = create_faucet_wallet();

    let faucet_balance = faucet_wallet.balance();
    if !faucet_balance.is_zero() {
        println!("Faucet wallet balance: {faucet_balance}");
        debug!("Faucet wallet balance: {faucet_balance}");
        return Ok(faucet_wallet);
    }

    println!("Loading genesis...");
    debug!("Loading genesis...");
    let genesis_wallet = load_genesis_wallet()?;

    // Transfer to faucet. We will transfer almost all of the genesis wallet's
    // balance to the faucet,.

    let faucet_balance = genesis_wallet.balance();
    println!("Sending {faucet_balance} from genesis to faucet wallet..");
    debug!("Sending {faucet_balance} from genesis to faucet wallet..");
    let cash_note = send(
        genesis_wallet,
        faucet_balance,
        faucet_wallet.address(),
        client,
        true,
    )
    .await?;

    faucet_wallet
        .deposit_and_store_to_disk(&vec![cash_note.clone()])
        .expect("Faucet wallet shall be stored successfully.");
    println!("Faucet wallet balance: {}", faucet_wallet.balance());
    debug!("Faucet wallet balance: {}", faucet_wallet.balance());

    println!("Verifying the transfer from genesis...");
    debug!("Verifying the transfer from genesis...");
    if let Err(error) = client.verify_cashnote(&cash_note).await {
        error!("Could not verify the transfer from genesis: {error:?}. Panicking.");
        panic!("Could not verify the transfer from genesis: {error:?}");
    } else {
        println!("Successfully verified the transfer from genesis on the second try.");
        info!("Successfully verified the transfer from genesis on the second try.");
    }

    Ok(faucet_wallet)
}
