// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use color_eyre::Result;
use std::time::Duration;

use crate::subcommands::SubCmd;
use sn_peers_acquisition::PeersArgs;

// Please do not remove the blank lines in these doc comments.
// They are used for inserting line breaks when the help menu is rendered in the UI.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Opt {
    #[command(flatten)]
    pub(crate) peers: PeersArgs,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,

    /// Timeout in seconds for the CLI to wait for a data response from the network.
    #[clap(long = "timeout", global = true, value_parser = |t: &str| -> Result<Duration> { Ok(t.parse().map(Duration::from_secs)?) })]
    pub timeout: Option<Duration>,
}
