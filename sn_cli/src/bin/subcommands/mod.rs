// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(crate) mod files;
pub(crate) mod folders;
pub(crate) mod register;
pub(crate) mod wallet;

use clap::Parser;
use clap::Subcommand;
use color_eyre::Result;
use sn_logging::{LogFormat, LogOutputDest};
use sn_peers_acquisition::PeersArgs;
use std::time::Duration;

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Opt {
    /// Specify the logging output destination.
    ///
    /// Valid values are "stdout", "data-dir", or a custom path.
    ///
    /// `data-dir` is the default value.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/client/logs
    ///  - macOS: $HOME/Library/Application Support/safe/client/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\client\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, value_parser = LogOutputDest::parse_from_str, verbatim_doc_comment, default_value = "data-dir")]
    pub log_output_dest: LogOutputDest,

    /// Specify the logging format.
    ///
    /// Valid values are "default" or "json".
    ///
    /// If the argument is not used, the default format will be applied.
    #[clap(long, value_parser = LogFormat::parse_from_str, verbatim_doc_comment)]
    pub log_format: Option<LogFormat>,

    #[command(flatten)]
    pub(crate) peers: PeersArgs,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,

    /// The maximum duration to wait for a connection to the network before timing out.
    #[clap(long = "timeout", global = true, value_parser = |t: &str| -> Result<Duration> { Ok(t.parse().map(Duration::from_secs)?) })]
    pub connection_timeout: Option<Duration>,

    /// Prevent verification of data storage on the network.
    ///
    /// This may increase operation speed, but offers no guarantees that operations were successful.
    #[clap(global = true, long = "no-verify", short = 'x')]
    pub no_verify: bool,
}

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
