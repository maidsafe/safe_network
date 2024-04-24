// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{wallet::send, Client, Result};
use sn_transfers::{load_genesis_wallet, HotWallet};

/// Use the client to load the faucet wallet from the genesis Wallet.
/// With all balance transferred from the genesis_wallet to the faucet_wallet.
pub async fn fund_faucet_from_genesis_wallet(
    client: &Client,
    faucet_wallet: &mut HotWallet,
) -> Result<()> {
    println!("Loading faucet...");
    info!("Loading faucet...");

    let faucet_balance = faucet_wallet.balance();
    if !faucet_balance.is_zero() {
        println!("Faucet wallet balance: {faucet_balance}");
        debug!("Faucet wallet balance: {faucet_balance}");
        return Ok(());
    }

    println!("Loading genesis...");
    debug!("Loading genesis...");
    let genesis_wallet = load_genesis_wallet()?;

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
        error!("Could not verify the transfer from genesis: {error}. Panicking.");
        panic!("Could not verify the transfer from genesis: {error}");
    } else {
        println!("Successfully verified the transfer from genesis on the second try.");
        info!("Successfully verified the transfer from genesis on the second try.");
    }

    Ok(())
}
