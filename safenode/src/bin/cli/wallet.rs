// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
pub enum WalletCmds {
    Deposit {
        /// Tries to load a hex encoded `Dbc` from the
        /// given path and deposit it to the wallet.
        #[clap(name = "dbc-dir")]
        dbc_dir: PathBuf,
        /// The location of the wallet file.
        #[clap(name = "wallet-dir")]
        wallet_dir: PathBuf,
    },
    Send {
        /// This shall be the number of nanos to send.
        /// Necessary if the `send_to` argument has been given.
        #[clap(name = "amount")]
        amount: String,
        /// This must be a hex-encoded `PublicAddress`.
        #[clap(name = "to")]
        to: String,
        /// The location of the wallet file.
        #[clap(name = "wallet-dir")]
        wallet_dir: PathBuf,
    },
}
