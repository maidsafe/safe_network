// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{wallet::send, Client, Error, Result};
use sn_transfers::{load_genesis_wallet, HotWallet, NanoTokens, FOUNDATION_PK};

const INITIAL_FAUCET_BALANCE: NanoTokens = NanoTokens::from(900000000000000000);

/// Use the client to load the faucet wallet from the genesis Wallet.
/// With all balance transferred from the genesis_wallet to the faucet_wallet.
pub async fn fund_faucet_from_genesis_wallet(
    client: &Client,
    faucet_wallet: &mut HotWallet,
) -> Result<()> {
    faucet_wallet.try_load_cash_notes()?;
    let faucet_balance = faucet_wallet.balance();
    if !faucet_balance.is_zero() {
        println!(
            "Faucet wallet existing balance: {}",
            faucet_wallet.balance()
        );
        debug!(
            "Faucet wallet existing balance: {}",
            faucet_wallet.balance()
        );

        return Ok(());
    }

    info!("funding faucet from genesis...");

    // Confirm Genesis not used yet
    if client.is_genesis_spend_present().await {
        warn!("Faucet can't get funded from genesis, genesis is already spent!");
        println!("Faucet can't get funded from genesis, genesis is already spent!");
        panic!("Faucet can't get funded from genesis, genesis is already spent!");
    }

    println!("Initiating genesis...");
    debug!("Initiating genesis...");
    let genesis_wallet = load_genesis_wallet()?;
    let genesis_balance = genesis_wallet.balance();

    let (foundation_cashnote, faucet_cashnote) = {
        println!("Sending {INITIAL_FAUCET_BALANCE}  from genesis to faucet wallet..");
        debug!("Sending {INITIAL_FAUCET_BALANCE}  from genesis to faucet wallet..");

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

        let genesis_wallet = load_genesis_wallet()?;

        let foundation_cashnote = send(
            genesis_wallet,
            foundation_balance,
            *FOUNDATION_PK,
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
        error!("Could not verify the transfer from genesis to foundation: {error}. Panicking.");
        panic!("Could not verify the transfer from genesis to foundation: {error}");
    } else {
        println!(
            "Successfully verified the transfer from genesis to foundation on the second try."
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            // write the foundation cashnote to disk
            let root_dir = faucet_wallet.api().wallet_dir();

            let foundation_transfer_path = root_dir.join("foundation_disbursement.transfer");

            debug!("Writing cash note to: {foundation_transfer_path:?}");

            let transfer =
                sn_transfers::Transfer::transfer_from_cash_note(&foundation_cashnote)?.to_hex()?;

            if let Err(error) = std::fs::write(foundation_transfer_path, transfer) {
                error!("Could not write the foundation transfer to disk: {error}.");
                return Err(Error::from(error));
            }
        }

        info!("Successfully verified the transfer from genesis to foundation on the second try.");
    }

    if let Err(error) = client.verify_cashnote(&faucet_cashnote).await {
        error!("Could not verify the transfer from genesis to faucet: {error}. Panicking.");
        panic!("Could not verify the transfer from genesis to faucet: {error}");
    } else {
        println!("Successfully verified the transfer from genesis to faucet on the second try.");
        info!("Successfully verified the transfer from genesis to faucet on the second try.");
    }

    Ok(())
}
