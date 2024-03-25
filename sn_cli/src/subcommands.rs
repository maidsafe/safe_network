// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod acc_packet;
pub(crate) mod files;
pub(crate) mod folders;
pub(crate) mod register;
pub(crate) mod wallet;

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(super) enum SubCmd {
    #[clap(name = "wallet", subcommand)]
    /// Commands for a hot-wallet management.
    /// A hot-wallet holds the secret key, thus it can be used for signing transfers/transactions.
    Wallet(wallet::hot_wallet::WalletCmds),
    #[clap(name = "wowallet", subcommand)]
    /// Commands for watch-only wallet management
    /// A watch-only wallet holds only the public key, thus it cannot be used for signing
    /// transfers/transactions, but only to query balances and broadcast offline signed transactions.
    WatchOnlyWallet(wallet::wo_wallet::WatchOnlyWalletCmds),
    #[clap(name = "files", subcommand)]
    /// Commands for file management
    Files(files::FilesCmds),
    #[clap(name = "folders", subcommand)]
    /// Commands for folders management
    Folders(folders::FoldersCmds),
    #[clap(name = "register", subcommand)]
    /// Commands for register management
    Register(register::RegisterCmds),
}
