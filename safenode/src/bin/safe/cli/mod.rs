// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
mod files;
mod register;
mod wallet;

use clap::{Parser, Subcommand};
use libp2p::Multiaddr;

pub(super) use self::{files::files_cmds, register::register_cmds, wallet::wallet_cmds};
use safenode::peers_acquisition::{parse_peer_addr, SAFE_PEERS_ENV};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(super) struct Opt {
    /// Peer(s) to use for bootstrap, supports either a 'multiaddr' or a socket address like `1.2.3.4:1234`.
    ///
    /// A multiaddr looks like '/ip4/1.2.3.4/tcp/1200/tcp/p2p/12D3KooWRi6wF7yxWLuPSNskXc6kQ5cJ6eaymeMbCRdTnMesPgFx'
    /// where `1.2.3.4` is the IP, `1200` is the port and the (optional) last part is the peer ID.
    ///
    /// This argument can be provided multiple times to connect to multiple peers.
    #[clap(long = "peer", value_name = "multiaddr", env = SAFE_PEERS_ENV, value_delimiter = ',', value_parser = parse_peer_addr)]
    pub peers: Vec<Multiaddr>,

    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

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
