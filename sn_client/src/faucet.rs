// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{wallet::send, Client, Error, Result};
use sn_transfers::{load_genesis_wallet, HotWallet, MainPubkey, NanoTokens};

const INITIAL_FAUCET_BALANCE: NanoTokens = NanoTokens::from(1000);

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
    let genesis_balance = genesis_wallet.balance();

    let foundation_key = MainPubkey::from_hex("a4bd3f928c585a63ab6070337316c1832bffd92be8efe9b235ec1c631f03b4bb91e29bbad34994ddf9f77d9858ddb0bb")?; // DevSkim: ignore DS117838

    let (foundation_cash_note, faucet_cashnote) = {
        let foundation_balance = genesis_balance
            .checked_sub(INITIAL_FAUCET_BALANCE)
            .ok_or(Error::GenesisDisbursement);

        println!("Sending {INITIAL_FAUCET_BALANCE}  from genesis to faucet wallet..");
        debug!("Sending {INITIAL_FAUCET_BALANCE}  from genesis to faucet wallet..");

        println!("Sending {foundation_balance:?}  from genesis to foundation wallet..");
        debug!("Sending {foundation_balance:?}  from genesis to foundation wallet..");

        let foundation_cash_note = send(
            genesis_wallet,
            genesis_balance,
            foundation_key,
            client,
            true,
        )
        .await?;

        let genesis_wallet = load_genesis_wallet()?;

        let faucet_cash_note = send(
            genesis_wallet,
            genesis_balance,
            faucet_wallet.address(),
            client,
            true,
        )
        .await?;

        faucet_wallet
            .deposit_and_store_to_disk(&vec![faucet_cash_note.clone()])
            .expect("Faucet wallet shall be stored successfully.");

        (foundation_cash_note, faucet_cash_note)
    };

    println!("Faucet wallet balance: {}", faucet_wallet.balance());
    debug!("Faucet wallet balance: {}", faucet_wallet.balance());

    println!("Verifying the transfer from genesis...");
    debug!("Verifying the transfer from genesis...");
    if let Err(error) = client.verify_cashnote(&foundation_cash_note).await {
        error!("Could not verify the transfer from genesis: {error}. Panicking.");
        panic!("Could not verify the transfer from genesis: {error}");
    } else {
        println!("Successfully verified the transfer from genesis on the second try.");
        info!("Successfully verified the transfer from genesis on the second try.");
    }

    if let Err(error) = client.verify_cashnote(&faucet_cashnote).await {
        error!("Could not verify the transfer from genesis: {error}. Panicking.");
        panic!("Could not verify the transfer from genesis: {error}");
    } else {
        println!("Successfully verified the transfer from genesis on the second try.");
        info!("Successfully verified the transfer from genesis on the second try.");
    }

    Ok(())
}
