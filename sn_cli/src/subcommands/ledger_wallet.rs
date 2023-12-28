// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::ledger::LedgerSntWallet;

use clap::Parser;
use color_eyre::{eyre::bail, Result};
use sn_client::Client;
use sn_transfers::{
    Hash, InputLedger, MainPubkey, NanoTokens, OutputLedger, SpendLedger, TransactionLedger,
};
use std::{path::Path, str::FromStr};

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser, Debug)]
pub enum LedgerCmds {
    /// Get info about the SNT wallet from Ledger device.
    Info {},
    /// Get a wallet address derived by Ledger device (based on EIP-2334).
    Address {
        /// Account number/index (this is used to generate the EIP-2334 path)
        #[clap(name = "account", default_value = "0")]
        account: Option<u32>,
    },
    /// Send a transfer signed with Ledger device.
    Send {
        /// The number of SafeNetworkTokens to send.
        #[clap(name = "amount")]
        amount: String,
        /// Hex-encoded public address of the recipient.
        #[clap(name = "to")]
        to: String,
        /// Account to use as source of the funds to send from (this is used to generate the EIP2334 path)
        #[clap(name = "account", default_value = "0")]
        account: Option<u32>,
    },
}

pub(crate) async fn ledger_cmds_without_client(cmds: &LedgerCmds) -> Result<()> {
    let mut ledger_wallet = LedgerSntWallet::new().await?;
    match cmds {
        LedgerCmds::Info {} => ledger_wallet.app_info().await,
        LedgerCmds::Address { account } => {
            let pk = ledger_wallet.get_addr(*account).await?;
            println!("Address: {}", pk.to_hex());
            Ok(())
        }
        // TODO: receive connect client for this cmd
        LedgerCmds::Send {
            amount,
            to,
            account,
        } => {
            send(
                amount, to, *account, /*, client, root_dir, verify_store*/
            )
            .await
        }
    }
}

async fn send(
    amount: &str,
    to: &str,
    account: Option<u32>, //client: &Client,
                          //root_dir: &Path,
                          //verify_store: bool,
) -> Result<()> {
    let mut ledger_wallet = LedgerSntWallet::new().await?;

    let amount = match NanoTokens::from_str(amount) {
        Ok(amount) => amount,
        Err(err) => {
            println!("The amount cannot be parsed. Nothing sent.");
            return Err(err.into());
        }
    };
    let to = match MainPubkey::from_hex(to) {
        Ok(to) => to,
        Err(err) => {
            println!("Error while parsing the recipient's 'to' key: {err:?}");
            return Err(err.into());
        }
    };

    // TODO: derive a new unique pub key
    let to_pk = to.public_key();
    println!("Destination pk: {}", to_pk.to_hex());

    // TODO: generate Spend using sn-transfers API

    let input = InputLedger {
        unique_pubkey: [0; 48],
        amount,
    };
    let output = OutputLedger {
        unique_pubkey: to_pk.to_bytes(),
        amount,
    };
    let tx = TransactionLedger {
        inputs: [input.clone()],
        outputs: [output.clone()],
    };
    let spend = SpendLedger {
        unique_pubkey: to_pk.to_bytes(),
        spent_tx: tx.clone(),
        reason: Hash::default(),
        token: amount,
        parent_tx: tx,
    };

    let (ledger_signature, derived_pk) = ledger_wallet.sign_spend(account, &spend).await?;

    println!("Signature: {:?}", ledger_signature.to_bytes());

    println!(
        "Verified: {}",
        derived_pk.verify(&ledger_signature, spend.to_bytes())
    );

    // TODO: build SignedSpend and send it to the network

    Ok(())
}
