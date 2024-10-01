// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod file;
mod register;
mod vault;

use clap::Subcommand;
use color_eyre::Result;

use crate::opt::Opt;

#[derive(Subcommand, Debug)]
pub enum SubCmd {
    /// Operations related to file handling.
    File {
        #[command(subcommand)]
        command: FileCmd,
    },

    /// Operations related to register management.
    Register {
        #[command(subcommand)]
        command: RegisterCmd,
    },

    /// Operations related to vault management.
    Vault {
        #[command(subcommand)]
        command: VaultCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum FileCmd {
    /// Estimate cost to upload a file.
    Cost {
        /// The file to estimate cost for.
        file: String,
    },

    /// Upload a file and pay for it.
    Upload {
        /// The file to upload.
        file: String,
    },

    /// Download a file from the given address.
    Download {
        /// The address of the file to download.
        addr: String,
        /// The destination file path.
        dest_file: String,
    },

    /// List previous uploads
    List,
}

#[derive(Subcommand, Debug)]
pub enum RegisterCmd {
    /// Estimate cost to register a name.
    Cost {
        /// The name to register.
        name: String,
    },

    /// Create a new register with the given name and value.
    Create {
        /// The name of the register.
        name: String,
        /// The value to store in the register.
        value: String,
    },

    /// Edit an existing register.
    Edit {
        /// The name of the register.
        name: String,
        /// The new value to store in the register.
        value: String,
    },

    /// Get the value of a register.
    Get {
        /// The name of the register.
        name: String,
    },

    /// List previous registers
    List,
}

#[derive(Subcommand, Debug)]
pub enum VaultCmd {
    /// Estimate cost to create a vault.
    Cost,

    /// Create a vault at a deterministic address based on your `SECRET_KEY`.
    Create,

    /// Sync vault with the network, including registers and files.
    Sync,
}

pub async fn handle_subcommand(opt: Opt) -> Result<()> {
    let peers = crate::utils::get_peers(opt.peers).await?;
    let cmd = opt.command;

    match cmd {
        SubCmd::File { command } => match command {
            FileCmd::Cost { file } => file::cost(&file, peers).await,
            FileCmd::Upload { file } => file::upload(&file, peers).await,
            FileCmd::Download { addr, dest_file } => file::download(&addr, &dest_file, peers).await,
            FileCmd::List => file::list(peers),
        },
        SubCmd::Register { command } => match command {
            RegisterCmd::Cost { name } => register::cost(&name, peers).await,
            RegisterCmd::Create { name, value } => register::create(&name, &value, peers).await,
            RegisterCmd::Edit { name, value } => register::edit(&name, &value, peers).await,
            RegisterCmd::Get { name } => register::get(&name, peers).await,
            RegisterCmd::List => register::list(peers),
        },
        SubCmd::Vault { command } => match command {
            VaultCmd::Cost => vault::cost(peers),
            VaultCmd::Create => vault::create(peers),
            VaultCmd::Sync => vault::sync(peers),
        },
    }
}
