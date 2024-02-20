// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::subcommands::SubCmd;
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use sn_logging::{LogFormat, LogOutputDest};
use sn_peers_acquisition::PeersArgs;
use std::{path::PathBuf, time::Duration};

pub fn parse_log_output(val: &str) -> Result<LogOutputDest> {
    match val {
        "stdout" => Ok(LogOutputDest::Stdout),
        "chunks-dir" => {
            // Get the current timestamp and format it to be human readable
            let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

            // Get the chunks directory path and append the timestamp to the log file name
            let dir = dirs_next::data_dir()
                .ok_or_else(|| eyre!("could not obtain chunks directory path".to_string()))?
                .join("safe")
                .join("client")
                .join("logs")
                .join(format!("log_{timestamp}"));
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
    /// Valid values are "stdout", "chunks-dir", or a custom path.
    ///
    /// `chunks-dir` is the default value.
    ///
    /// The chunks directory location is platform specific:
    ///  - Linux: $HOME/.local/share/safe/client/logs
    ///  - macOS: $HOME/Library/Application Support/safe/client/logs
    ///  - Windows: C:\Users\<username>\AppData\Roaming\safe\client\logs
    #[allow(rustdoc::invalid_html_tags)]
    #[clap(long, value_parser = parse_log_output, verbatim_doc_comment, default_value = "chunks-dir")]
    pub log_output_dest: Option<LogOutputDest>,

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

    /// Prevent verification of chunks storage on the network.
    ///
    /// This may increase operation speed, but offers no guarantees that operations were successful.
    #[clap(global = true, long = "no-verify", short = 'x')]
    pub no_verify: bool,
}
