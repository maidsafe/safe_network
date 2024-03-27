// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::AccountPacket;
use sn_client::{
    protocol::storage::{RegisterAddress, RetryStrategy},
    transfers::MainSecretKey,
    Client, UploadCfg, BATCH_SIZE,
};

use bls::{SecretKey, SK_SIZE};
use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use dialoguer::Password;
use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
pub enum FoldersCmds {
    Init {
        /// The directory to initialise as a root folder, which can then be stored on the network (and kept in sync with).
        /// By default the current path is assumed.
        #[clap(name = "path", value_name = "PATH")]
        path: Option<PathBuf>,
        /// The hex-encoded recovery secret key for deriving addresses, encryption and signing keys, to be used by this account packet.
        #[clap(name = "recovery key")]
        root_sk: Option<String>,
    },
    Upload {
        /// The location of the file(s) to upload for creating the folder on the network.
        /// By default the current path is assumed.
        #[clap(name = "path", value_name = "PATH")]
        path: Option<PathBuf>,
        /// The batch_size to split chunks into parallel handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE, short='b')]
        batch_size: usize,
        /// Should the files be made accessible to all. (This is irreversible)
        #[clap(long, name = "make_public", default_value = "false", short = 'p')]
        make_data_public: bool,
        /// Set the strategy to use on chunk upload failure. Does not modify the spend failure retry attempts yet.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Balanced, short = 'r', help = "Sets the retry strategy on upload failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
    Download {
        /// The hex address of a folder.
        #[clap(name = "address")]
        folder_addr: String,
        /// The full local path where to download the folder. By default the current path is assumed,
        /// and the main Folder's network address will be used as the folder name.
        #[clap(name = "target folder path")]
        path: Option<PathBuf>,
        /// The batch_size for parallel downloading
        #[clap(long, default_value_t = BATCH_SIZE , short='b')]
        batch_size: usize,
        /// Set the strategy to use on downloads failure.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Quick, short = 'r', help = "Sets the retry strategy on download failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
    /// Report any changes made to local version of files/folders (this doesn't compare it with their versions stored on the network).
    Status {
        /// Path to check changes made on. By default the current path is assumed.
        #[clap(name = "path", value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Sync up local files/folders changes with their versions stored on the network.
    Sync {
        /// Path to sync with its remote version on the network. By default the current path is assumed.
        #[clap(name = "path", value_name = "PATH")]
        path: Option<PathBuf>,
        /// The batch_size to split chunks into parallel handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE, short='b')]
        batch_size: usize,
        /// Should the files be made accessible to all. (This is irreversible)
        #[clap(long, name = "make_public", default_value = "false", short = 'p')]
        make_data_public: bool,
        /// Set the strategy to use on chunk upload failure. Does not modify the spend failure retry attempts yet.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Balanced, short = 'r', help = "Sets the retry strategy on upload failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
}

pub(crate) async fn folders_cmds(
    cmds: FoldersCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        FoldersCmds::Init { path, root_sk } => {
            let path = get_path(path, None)?;
            // initialise path as a fresh new Folder with a network address derived from the root SK
            let root_sk = get_or_generate_root_sk(root_sk)?;
            let acc_packet = AccountPacket::init(client.clone(), root_dir, &path, root_sk)?;
            println!("Directoy at {path:?} initialised as a root Folder, ready to track and sync changes with the network at address: {}", acc_packet.root_folder_addr().to_hex())
        }
        FoldersCmds::Upload {
            path,
            batch_size,
            make_data_public,
            retry_strategy,
        } => {
            let path = get_path(path, None)?;
            // initialise path as a fresh new Folder with a network address derived from a random SK
            let root_sk = get_or_generate_root_sk(None)?;
            let mut acc_packet = AccountPacket::init(client.clone(), root_dir, &path, root_sk)?;

            let options = UploadCfg {
                verify_store,
                batch_size,
                retry_strategy,
                ..Default::default()
            };
            acc_packet.sync(options, make_data_public).await?;

            println!(
                "\nFolder hierarchy from {path:?} uploaded successfully at {}",
                acc_packet.root_folder_addr().to_hex()
            );
        }
        FoldersCmds::Download {
            folder_addr,
            path,
            batch_size,
            retry_strategy,
        } => {
            let address = RegisterAddress::from_hex(&folder_addr)
                .map_err(|err| eyre!("Failed to parse Folder address: {err}"))?;

            let addr_hex = address.to_hex();
            let folder_name = format!(
                "folder_{}_{}",
                &addr_hex[..6],
                &addr_hex[addr_hex.len() - 6..]
            );
            let download_folder_path = get_path(path, Some(&folder_name))?;
            println!(
                "Downloading onto {download_folder_path:?}, with batch-size {batch_size}, from {addr_hex}"            );
            debug!("Downloading onto {download_folder_path:?} from {addr_hex}");

            let _ = AccountPacket::retrieve_folders(
                client,
                root_dir,
                address,
                &download_folder_path,
                batch_size,
                retry_strategy,
            )
            .await?;
        }
        FoldersCmds::Status { path } => {
            let path = get_path(path, None)?;
            let acc_packet = AccountPacket::from_path(client.clone(), root_dir, &path)?;
            acc_packet.status()?;
        }
        FoldersCmds::Sync {
            path,
            batch_size,
            make_data_public,
            retry_strategy,
        } => {
            let path = get_path(path, None)?;
            let mut acc_packet = AccountPacket::from_path(client.clone(), root_dir, &path)?;

            let options = UploadCfg {
                verify_store,
                batch_size,
                retry_strategy,
                ..Default::default()
            };
            acc_packet.sync(options, make_data_public).await?;
        }
    }
    Ok(())
}

// Unwrap provided path, or return the current path if none was provided.
// It can optionally be provided a string to adjoin when the current dir is returned.
fn get_path(path: Option<PathBuf>, to_join: Option<&str>) -> Result<PathBuf> {
    let path = if let Some(path) = path {
        path
    } else {
        let current_dir = current_dir()?;
        to_join.map_or_else(|| current_dir.clone(), |str| current_dir.join(str))
    };
    Ok(path)
}

// Either get a hex-encoded SK entered by the user, or we generate a new one
// TODO: get/generate a mnemonic instead
fn get_or_generate_root_sk(root_sk: Option<String>) -> Result<MainSecretKey> {
    let result = match root_sk {
        Some(str) => SecretKey::from_hex(&str),
        None => {
            println!();
            println!("A recovery secret is used to derive signing/ecnryption keys and network addresses used by an Account Packet.");
            println!("The recovery secret used to initialise an Account Packet, can be used to retrieve a new replica/clone from the network even from another device or disk location.");
            println!("Thefore, please make sure you don't loose you recovery secret, and always make sure you sync up your changes with the replica on the network to not loose them.");
            println!();

            let err_msg = format!("Hex-encoded recovery secret must be {} long", 2 * SK_SIZE);
            let sk_hex = Password::new()
                .with_prompt(
                    "Please enter your recovery secret, if you don't have one, press Enter to generate one",
                )
                .allow_empty_password(true)
                .validate_with(|input: &String| -> Result<(), &str> {
                    let len = input.chars().count();
                    if len == 0 || len == 2 * SK_SIZE {
                        Ok(())
                    } else {
                        Err(&err_msg)
                    }
                })
                .interact()?;

            if sk_hex.is_empty() {
                println!("Generating your recovery secret...");
                // TODO: encrypt the recovery secret before storing it on disk, using a user provided password
                Ok(SecretKey::random())
            } else {
                SecretKey::from_hex(&sk_hex)
            }
        }
    };

    match result {
        Ok(sk) => {
            println!("Recovery secret decoded successfully!");
            // TODO: store it on disk so the user doesn't have to enter it with every cmd
            Ok(MainSecretKey::new(sk))
        }
        Err(err) => bail!("Failed to decode the recovery secret provided: {err:?}"),
    }
}
