// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use std::path::PathBuf;
use std::time::Duration;

use crate::subcommands::SubCmd;
use sn_logging::{parse_log_format, LogFormat, LogOutputDest};
use sn_peers_acquisition::PeersArgs;

pub fn parse_log_output(val: &str) -> Result<LogOutputDest> {
    match val {
        "stdout" => Ok(LogOutputDest::Stdout),
        "data-dir" => {
            let dir = dirs_next::data_dir()
                .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
                .join("safe")
                .join("client")
                .join("logs");
            Ok(LogOutputDest::Path(dir))
        }
        // The path should be a directory, but we can't use something like `is_dir` to check
        // because the path doesn't need to exist. We can create it for the user.
        value => Ok(LogOutputDest::Path(PathBuf::from(value))),
    }
}

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Opt {
    /// Specify the logging output destination.
    ///
    /// Valid values are "stdout", "data-dir", or a custom path.
    ///
    /// The data directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/client/logs
    ///  - macOS: $HOME/Library/Application Support/safe/client/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\client\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, value_parser = parse_log_output, verbatim_doc_comment)]
    pub log_output_dest: Option<LogOutputDest>,

    /// Specify the logging format.
    ///
    /// Valid values are "default" or "json".
    ///
    /// If the argument is not used, the default format will be applied.
    #[clap(long, value_parser = parse_log_format, verbatim_doc_comment)]
    pub log_format: Option<LogFormat>,

    #[command(flatten)]
    pub(crate) peers: PeersArgs,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,

    /// Timeout in seconds for the CLI to wait for a data response from the network.
    #[clap(long = "timeout", global = true, value_parser = |t: &str| -> Result<Duration> { Ok(t.parse().map(Duration::from_secs)?) })]
    pub timeout: Option<Duration>,
}
