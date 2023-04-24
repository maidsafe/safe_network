// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! The `faucet` subcommand is a testnet only command.
//! It is used to create the genesis DBC and deposit it to the faucet local wallet.
//! The idea is to then receive a public address from a community member, and send
//! a small amount of tokens to them. This will allow them to send tokens to others,
//! and start using the whole feature of token transfers on the network.
//!
//! The faucet will be managed by a simple web interface, probably using aws lambda + flask mvc or some such.
//!
//! Pattern for calling cli:
//! `cargo run --bin faucet --release -- faucet send -- [amount per request] [public address hex]`
//!
//! Example of calling cli:
//! `cargo run --bin faucet --release -- faucet send -- 100 9b0f0e917cd7fe75cad2196c4bea7ed1873deec85692e402be287a7068b2c3f7b6795fb7fa10a141f01a5d69b8ddc0a40000000000000001`

use safenode::{
    client::{Client, WalletClient},
    domain::{
        dbc_genesis::create_genesis,
        wallet::{parse_public_address, DepositWallet, LocalWallet, Wallet},
    },
};

use sn_dbc::Token;

use clap::Parser;
use eyre::Result;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub enum FaucetCmds {
    /// Create the genesis DBC and deposit it to the faucet local wallet.
    Genesis,
    Send {
        /// This shall be the number of nanos to send.
        /// Necessary if the `to` argument has been given.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `PublicAddress`.
        #[clap(name = "to")]
        to: String,
    },
}

pub(crate) async fn faucet_cmds(cmds: FaucetCmds, client: &Client) -> Result<()> {
    match cmds {
        FaucetCmds::Genesis => genesis().await?,
        FaucetCmds::Send { amount, to } => send(amount, to, client).await?,
    }
    Ok(())
}

async fn genesis() -> Result<()> {
    let genesis = create_genesis()?;
    let root_dir = get_faucet_dir().await?;
    let mut wallet = LocalWallet::load_from(&root_dir).await?;

    let previous_balance = wallet.balance();

    wallet.deposit(vec![genesis]);

    let new_balance = wallet.balance();
    let deposited = previous_balance.as_nano() - new_balance.as_nano();

    if deposited > 0 {
        if let Err(err) = wallet.store().await {
            println!("Failed to store deposited amount: {:?}", err);
        } else {
            println!("Deposited {:?}.", sn_dbc::Token::from_nano(deposited));
        }
    } else {
        println!("Nothing deposited.");
    }

    Ok(())
}

async fn send(amount: String, to: String, client: &Client) -> Result<()> {
    let address = parse_public_address(to)?;

    use std::str::FromStr;
    let amount = Token::from_str(&amount)?;
    if amount.as_nano() == 0 {
        println!("Invalid format or zero amount passed in. Nothing sent.");
        return Ok(());
    }

    let root_dir = get_faucet_dir().await?;
    let wallet = LocalWallet::load_from(&root_dir).await?;

    let mut wallet_client = WalletClient::new(client.clone(), wallet);

    match wallet_client.send(amount, address).await {
        Ok(new_dbc) => {
            println!("Sent {amount:?} to {address:?}");
            let mut wallet = wallet_client.into_wallet();
            let new_balance = wallet.balance();

            if let Err(err) = wallet.store().await {
                println!("Failed to store wallet: {err:?}");
            } else {
                println!("Successfully stored wallet with new balance {new_balance:?}.");
            }

            wallet.store_created_dbc(new_dbc).await?;
            println!("Successfully stored new dbc to wallet dir. It can now be sent to the recipient, using any channel of choice.");
        }
        Err(err) => {
            println!("Failed to send {amount:?} to {address:?} due to {err:?}.");
        }
    }

    Ok(())
}

async fn get_faucet_dir() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("faucet");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}
