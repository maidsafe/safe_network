// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod faucet;

use clap::{Parser, Subcommand};

pub(super) use self::faucet::faucet_cmds;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(super) struct Opt {
    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
pub(super) enum SubCmd {
    #[clap(name = "faucet", subcommand)]
    /// Manage faucet on a Test SAFE Network
    Faucet(faucet::FaucetCmds),
}
