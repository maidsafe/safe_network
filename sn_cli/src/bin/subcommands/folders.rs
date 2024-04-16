// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::AccountPacket;

use sn_client::{
    protocol::storage::RetryStrategy, transfers::MainSecretKey, Client, UploadCfg, BATCH_SIZE,
};

use bls::{SecretKey, SK_SIZE};
use clap::Parser;
use color_eyre::{eyre::bail, Result};
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
    Download {
        /// The full local path where to download the folder. By default the current path is assumed,
        /// and the main Folder's network address will be used as the folder name.
        #[clap(name = "target folder path")]
        path: Option<PathBuf>,
        /// The hex-encoded recovery secret key for deriving addresses, encryption and signing keys, to be used by this account packet.
        #[clap(name = "recovery key")]
        root_sk: Option<String>,
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
            let root_sk = get_recovery_secret_sk(root_sk, true)?;
            let acc_packet = AccountPacket::init(client.clone(), root_dir, &path, &root_sk, None)?;
            println!("Directory at {path:?} initialised as a root Folder, ready to track and sync changes with the network at address: {}", acc_packet.root_folder_addr().to_hex())
        }
        FoldersCmds::Download {
            path,
            root_sk,
            batch_size,
            retry_strategy,
        } => {
            let root_sk = get_recovery_secret_sk(root_sk, false)?;
            let root_sk_hex = root_sk.main_pubkey().to_hex();
            let download_folder_name = format!(
                "folder_{}_{}",
                &root_sk_hex[..6],
                &root_sk_hex[root_sk_hex.len() - 6..]
            );
            let download_folder_path = get_path(path, Some(&download_folder_name))?;
            println!("Downloading onto {download_folder_path:?}, with batch-size {batch_size}");
            debug!("Downloading onto {download_folder_path:?}");

            let _ = AccountPacket::retrieve_folders(
                client,
                root_dir,
                &root_sk,
                None,
                &download_folder_path,
                batch_size,
                retry_strategy,
            )
            .await?;
        }
        FoldersCmds::Status { path } => {
            let path = get_path(path, None)?;
            let acc_packet = AccountPacket::from_path(client.clone(), root_dir, &path, None)?;
            acc_packet.status()?;
        }
        FoldersCmds::Sync {
            path,
            batch_size,
            make_data_public,
            retry_strategy,
        } => {
            let path = get_path(path, None)?;
            let mut acc_packet = AccountPacket::from_path(client.clone(), root_dir, &path, None)?;

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

// Either get a hex-encoded SK entered by the user, or generate a new one
// TODO: get/generate a mnemonic instead
fn get_recovery_secret_sk(
    root_sk: Option<String>,
    gen_new_recovery_secret: bool,
) -> Result<MainSecretKey> {
    let result = if let Some(str) = root_sk {
        SecretKey::from_hex(&str)
    } else {
        let prompt_msg = if gen_new_recovery_secret {
            println!(
                "\n\nA recovery secret is required to derive signing/encryption keys, and network addresses, \
                used by an Account Packet."
            );
            println!(
                "The recovery secret used to initialise an Account Packet, can be used to retrieve and restore \
                a new replica/clone from the network, onto any local path and even onto another device.\n"
            );

            "Please enter your recovery secret for this new Account Packet,\nif you don't have one, \
            press [Enter] to generate one"
        } else {
            "Please enter your recovery secret"
        };

        let err_msg = format!("Hex-encoded recovery secret must be {} long", 2 * SK_SIZE);
        let sk_hex = Password::new()
            .with_prompt(prompt_msg)
            .allow_empty_password(gen_new_recovery_secret)
            .validate_with(|input: &String| -> Result<(), &str> {
                let len = input.chars().count();
                if len == 0 || len == 2 * SK_SIZE {
                    Ok(())
                } else {
                    Err(&err_msg)
                }
            })
            .interact()?;

        println!();
        if sk_hex.is_empty() {
            println!("Generating your recovery secret...");
            let sk = SecretKey::random();
            println!("\n*** Recovery secret generated ***\n{}", sk.to_hex());
            println!();
            println!(
                "Please *MAKE SURE YOU DON'T LOOSE YOU RECOVERY SECRET*, and always sync up local changes \
                made to your Account Packet with the remote replica on the network to not loose them either.\n"
            );

            Ok(sk)
        } else {
            SecretKey::from_hex(&sk_hex)
        }
    };

    match result {
        Ok(sk) => Ok(MainSecretKey::new(sk)),
        Err(err) => bail!("Failed to decode the recovery secret: {err:?}"),
    }
}
