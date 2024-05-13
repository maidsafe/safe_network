// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{wallet::send, Client, Error, Result};
use sn_transfers::{
    get_existing_genesis_wallet, load_genesis_wallet, HotWallet, MainPubkey, NanoTokens,
};

const INITIAL_FAUCET_BALANCE: NanoTokens = NanoTokens::from(100000000000000000);

/// Use the client to load the faucet wallet from the genesis Wallet.
/// With all balance transferred from the genesis_wallet to the faucet_wallet.
pub async fn fund_faucet_from_genesis_wallet(
    client: &Client,
    faucet_wallet: &mut HotWallet,
) -> Result<()> {
    info!("funding faucet from genesis...");

    let faucet_balance = faucet_wallet.balance();
    if !faucet_balance.is_zero() {
        return Ok(());
    }

    println!("Initiating genesis...");
    debug!("Initiating genesis...");
    let genesis_wallet = load_genesis_wallet()?;
    let genesis_balance = genesis_wallet.balance();

    let foundation_key = MainPubkey::from_hex("a4bd3f928c585a63ab6070337316c1832bffd92be8efe9b235ec1c631f03b4bb91e29bbad34994ddf9f77d9858ddb0bb")?; // DevSkim: ignore DS117838

    let (foundation_cashnote, faucet_cashnote) = {
        println!("Sending {INITIAL_FAUCET_BALANCE}  from genesis to faucet wallet..");
        debug!("Sending {INITIAL_FAUCET_BALANCE}  from genesis to faucet wallet..");

        let genesis_wallet = get_existing_genesis_wallet();
        println!("Faucet wallet balance: {}", faucet_wallet.balance());
        debug!("Faucet wallet balance: {}", faucet_wallet.balance());
        let faucet_cashnote = send(
            genesis_wallet,
            INITIAL_FAUCET_BALANCE,
            faucet_wallet.address(),
            client,
            true,
        )
        .await?;

        faucet_wallet
            .deposit_and_store_to_disk(&vec![faucet_cashnote.clone()])
            .expect("Faucet wallet shall be stored successfully.");

        // now send the money to the foundation
        let foundation_balance = genesis_balance
            .checked_sub(INITIAL_FAUCET_BALANCE)
            .ok_or(Error::GenesisDisbursement)?;

        println!("Sending {foundation_balance:?} from genesis to foundation wallet..");
        debug!("Sending {foundation_balance:?} from genesis to foundation wallet..");

        let mut genesis_wallet = get_existing_genesis_wallet();
        genesis_wallet.try_load_cash_notes()?;

        let foundation_cashnote = send(
            genesis_wallet,
            foundation_balance,
            foundation_key,
            client,
            true,
        )
        .await?;

        (foundation_cashnote, faucet_cashnote)
    };

    println!("Faucet wallet balance: {}", faucet_wallet.balance());
    debug!("Faucet wallet balance: {}", faucet_wallet.balance());

    println!("Verifying the transfer from genesis...");
    debug!("Verifying the transfer from genesis...");
    if let Err(error) = client.verify_cashnote(&foundation_cashnote).await {
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
