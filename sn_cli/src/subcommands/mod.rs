// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
pub(crate) mod files;
pub(crate) mod register;
pub(crate) mod wallet;

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(super) enum SubCmd {
    #[clap(name = "wallet", subcommand)]
    /// Manage wallets on the SAFE Network
    Wallet(wallet::WalletCmds),
    #[clap(name = "files", subcommand)]
    /// Manage files on the SAFE Network
    Files(files::FilesCmds),
    #[clap(name = "register", subcommand)]
    /// Manage files on the SAFE Network
    Register(register::RegisterCmds),
}
