// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod cfg;
mod files;
mod register;
mod wallet;

use clap::Parser;

pub(super) use self::{cfg::CfgCmds, files::FilesCmds, register::RegisterCmds, wallet::WalletCmds};

#[derive(Parser, Debug)]
#[clap(name = "safeclient cli")]
pub(super) enum Opt {
    #[clap(name = "cfg", subcommand)]
    Cfg(cfg::CfgCmds),
    #[clap(name = "wallet", subcommand)]
    /// Manage wallets on the SAFE Network
    Wallet(wallet::WalletCmds),
    #[clap(name = "files", subcommand)]
    /// Manage files on the SAFE Network
    Files(files::FilesCmds),
    #[clap(name = "files", subcommand)]
    /// Manage files on the SAFE Network
    Register(register::RegisterCmds),
}
