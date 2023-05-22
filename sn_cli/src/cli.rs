// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use libp2p::Multiaddr;

use crate::subcommands::SubCmd;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Opt {
    /// Nodes we dial at start to help us get connected to the network. Can be specified multiple times.
    #[clap(long = "peer")]
    pub peers: Vec<Multiaddr>,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}
