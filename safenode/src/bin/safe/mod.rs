// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod cli;

use self::cli::{files_cmds, register_cmds, wallet_cmds, Opt, SubCmd};
use clap::Parser;
use eyre::Result;

use safenode::client::Client;
use safenode::git_hash;
use safenode::{log::init_node_logging, peers_acquisition::peers_from_opts_or_env};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    // For client, default to log to std::out
    // This is ruining the log output for the CLI. Needs to be fixed.
    let tmp_dir = std::env::temp_dir();
    let log_appender_guard = init_node_logging(&Some(tmp_dir.join("safe-client")))?;

    info!("Full client logs will be written to {:?}", tmp_dir);
    println!("Instantiating a SAFE client...");
    println!("Current build's git commit hash: {}", git_hash::git_hash());

    let secret_key = bls::SecretKey::random();
    let peers = peers_from_opts_or_env(&opt.peers)?;
    let root_dir = get_client_dir().await?;

    let client = Client::new(secret_key, Some(peers)).await?;

    match opt.cmd {
        SubCmd::Wallet(cmds) => wallet_cmds(cmds, &client, &root_dir).await?,
        SubCmd::Files(cmds) => files_cmds(cmds, client.clone(), &root_dir).await?,
        SubCmd::Register(cmds) => register_cmds(cmds, &client).await?,
    };

    drop(log_appender_guard);
    Ok(())
}

async fn get_client_dir() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("client");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}
