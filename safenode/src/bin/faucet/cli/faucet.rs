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
    client::Client,
    domain::{
        dbc_genesis::{get_tokens_from_faucet, load_faucet_wallet},
        wallet::parse_public_address,
    },
};

use sn_dbc::Token;

use clap::Parser;
use eyre::Result;

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
        FaucetCmds::Genesis => {
            let _wallet = load_faucet_wallet(client).await;
        }
        FaucetCmds::Send { amount, to } => {
            let to = parse_public_address(to)?;
            use std::str::FromStr;
            let amount = Token::from_str(&amount)?;
            if amount.as_nano() == 0 {
                println!("Invalid format or zero amount passed in. Nothing sent.");
                return Ok(());
            }

            let dbc = get_tokens_from_faucet(amount, to, client).await;
            let dbc_hex = dbc.to_hex()?;
            println!("{dbc_hex}");
        }
    }
    Ok(())
}
