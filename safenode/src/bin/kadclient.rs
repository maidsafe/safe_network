// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod cli;

use cli::{CfgCmds, FilesCmds, Opt, RegisterCmds, WalletCmds};

use safenode::{
    client::{Client, ClientEvent, Error as ClientError, Files, WalletClient},
    log::init_node_logging,
    protocol::{
        address::ChunkAddress,
        wallet::{parse_public_address, DepositWallet, LocalWallet, Wallet},
    },
};

use sn_dbc::{Dbc, Token};

use bytes::Bytes;
use clap::Parser;
use eyre::Result;
use std::{fs, path::PathBuf};
use tracing::{info, warn};
use walkdir::WalkDir;
use xor_name::XorName;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();

    if let Opt::Cfg(cmds) = &opt {
        cfg_cmds(cmds).await?;
    }

    info!("Instantiating a SAFE client...");

    let secret_key = bls::SecretKey::random();
    let client = Client::new(secret_key)?;

    let mut client_events_rx = client.events_channel();
    if let Ok(event) = client_events_rx.recv().await {
        match event {
            ClientEvent::ConnectedToNetwork => {
                info!("Client connected to the Network");
            }
        }
    }

    match opt {
        Opt::Cfg(cmds) => cfg_cmds(&cmds).await?,
        Opt::Wallet(cmds) => wallet_cmds(cmds, &client).await?,
        Opt::Files(cmds) => files_cmds(cmds, client.clone()).await?,
        Opt::Register(cmds) => register_cmds(cmds, &client).await?,
    };

    Ok(())
}

/// ------------------------------------------------------------------------------------
/// ------------------------------- Cfg ------------------------------------------------

async fn cfg_cmds(cfg: &CfgCmds) -> Result<()> {
    match cfg {
        CfgCmds::Logs { log_dir } => {
            let _log_appender_guard = init_node_logging(log_dir)?;
        }
    }
    Ok(())
}

/// ------------------------------------------------------------------------------------
/// ------------------------------- Wallet ---------------------------------------------

async fn wallet_cmds(cmds: WalletCmds, client: &Client) -> Result<()> {
    match cmds {
        WalletCmds::Deposit {
            dbc_dir,
            wallet_dir,
        } => deposit(dbc_dir, wallet_dir).await?,
        WalletCmds::Send {
            amount,
            to,
            wallet_dir,
        } => send(amount, to, wallet_dir, client).await?,
    }
    Ok(())
}

async fn deposit(dbc_dir: PathBuf, wallet_dir: PathBuf) -> Result<()> {
    let mut wallet = LocalWallet::load_from(&wallet_dir).await?;

    let mut deposits = vec![];

    for entry in WalkDir::new(dbc_dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file_name = entry.file_name();
            info!("Reading deposited tokens from {file_name:?}.");
            println!("Reading deposited tokens from {file_name:?}.");

            let dbc_data = fs::read_to_string(entry.path())?;
            let dbc = match Dbc::from_hex(dbc_data.trim()) {
                Ok(dbc) => dbc,
                Err(_) => {
                    warn!(
                        "This file does not appear to have valid hex-encoded DBC data. \
                        Skipping it."
                    );
                    println!(
                        "This file does not appear to have valid hex-encoded DBC data. \
                        Skipping it."
                    );
                    continue;
                }
            };

            deposits.push(dbc);
        }
    }

    let previous_balance = wallet.balance();
    wallet.deposit(deposits);
    let new_balance = wallet.balance();
    let deposited = previous_balance.as_nano() - new_balance.as_nano();

    if deposited > 0 {
        if let Err(err) = wallet.store().await {
            warn!("Failed to store deposited amount: {:?}", err);
            println!("Failed to store deposited amount: {:?}", err);
        } else {
            info!("Deposited {:?}.", sn_dbc::Token::from_nano(deposited));
            println!("Deposited {:?}.", sn_dbc::Token::from_nano(deposited));
        }
    } else {
        info!("Nothing deposited.");
        println!("Nothing deposited.");
    }

    Ok(())
}

async fn send(amount: String, to: String, wallet_dir: PathBuf, client: &Client) -> Result<()> {
    let address = parse_public_address(to)?;
    let amount = parse_tokens_amount(&amount);

    if amount.as_nano() == 0 {
        return Ok(());
    }

    let wallet = LocalWallet::load_from(&wallet_dir).await?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);
    match wallet_client.send(amount, address).await {
        Ok(_new_dbcs) => {
            info!("Sent {amount:?} to {address:?}");
            println!("Sent {amount:?} to {address:?}");
            let wallet = wallet_client.into_wallet();
            let new_balance = wallet.balance();

            if let Err(err) = wallet.store().await {
                warn!("Failed to store wallet: {err:?}");
                println!("Failed to store wallet: {err:?}");
            } else {
                info!("Successfully stored wallet with new balance {new_balance:?}.");
                println!("Successfully stored wallet with new balance {new_balance:?}.");
            }
        }
        Err(err) => {
            warn!("Failed to send {amount:?} to {address:?} due to {err:?}.");
            println!("Failed to send {amount:?} to {address:?} due to {err:?}.");
        }
    }

    Ok(())
}

fn parse_tokens_amount(amount_str: &str) -> Token {
    use std::str::FromStr;
    match Token::from_str(amount_str) {
        Ok(amount) => return amount,
        Err(err) => match err {
            sn_dbc::Error::ExcessiveTokenValue => {
                warn!("Invalid amount to send: {amount_str:?}, it exceeds the maximum possible value.");
                println!("Invalid amount to send: {amount_str:?}, it exceeds the maximum possible value.");
            }
            sn_dbc::Error::LossOfTokenPrecision => {
                warn!("Invalid amount to send: '{amount_str}', the minimum possible amount is one nano token (0.000000001).");
                println!("Invalid amount to send: '{amount_str}', the minimum possible amount is one nano token (0.000000001).");
            }
            sn_dbc::Error::FailedToParseToken(msg) => {
                warn!("Invalid amount to send: '{amount_str}': {msg}.");
                println!("Invalid amount to send: '{amount_str}': {msg}.");
            }
            other_err => {
                warn!("Invalid amount to send: '{amount_str}': {other_err:?}.");
                println!("Invalid amount to send: '{amount_str}': {other_err:?}.");
            }
        },
    }

    Token::from_nano(0)
}

/// ------------------------------------------------------------------------------------
/// ------------------------------- Files ----------------------------------------------

async fn files_cmds(cmds: FilesCmds, client: Client) -> Result<()> {
    let file_api: Files = Files::new(client);
    match cmds {
        FilesCmds::Upload { files_path } => upload_files(files_path, &file_api).await?,
        FilesCmds::Download { file_names_path } => {
            download_files(file_names_path, &file_api).await?
        }
    };
    Ok(())
}

async fn upload_files(files_path: PathBuf, file_api: &Files) -> Result<()> {
    let file_names_path = files_path.join("uploaded_files/file_names.txt");
    let mut chunks_to_fetch = Vec::new();

    for entry in WalkDir::new(files_path).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            let file_name = entry.file_name();

            info!("Storing file {file_name:?} of {} bytes..", bytes.len());
            println!("Storing file {file_name:?}.");

            match file_api.upload(bytes).await {
                Ok(address) => {
                    info!("Successfully stored file to {address:?}");
                    chunks_to_fetch.push(*address.name());
                }
                Err(error) => {
                    panic!(
                        "Did not store file {file_name:?} to all nodes in the close group! {error}"
                    )
                }
            };
        }
    }

    let content = bincode::serialize(&chunks_to_fetch)?;
    tokio::fs::create_dir_all(file_names_path.as_path()).await?;
    fs::write(file_names_path, content)?;

    Ok(())
}

async fn download_files(file_names_dir: PathBuf, file_api: &Files) -> Result<()> {
    for entry in WalkDir::new(file_names_dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            let file_name = entry.file_name();

            info!("Loading file xornames from {file_name:?}");
            println!("Loading file xornames from {file_name:?}");
            let chunks_to_fetch: Vec<XorName> = bincode::deserialize(&bytes)?;

            for xorname in chunks_to_fetch.iter() {
                info!("Downloading file {xorname:?}");
                println!("Downloading file {xorname:?}");
                match file_api.read_bytes(ChunkAddress::new(*xorname)).await {
                    Ok(bytes) => info!("Successfully got file {xorname} of {} bytes!", bytes.len()),
                    Err(error) => {
                        panic!("Did not get file {xorname:?} from the network! {error}")
                    }
                };
            }
        }
    }

    Ok(())
}

/// ------------------------------------------------------------------------------------
/// ------------------------------- Registers ------------------------------------------

async fn register_cmds(cmds: RegisterCmds, client: &Client) -> Result<()> {
    match cmds {
        RegisterCmds::Create { name } => create_register(name, client).await?,
        RegisterCmds::Edit { name, entry } => edit_register(name, entry, client).await?,
        RegisterCmds::Get { names } => get_registers(names, client).await?,
    }
    Ok(())
}

async fn create_register(name: String, client: &Client) -> Result<()> {
    let tag = 3006;
    let xorname = XorName::from_content(name.as_bytes());
    println!("Creating Register with '{name}' at xorname: {xorname:x} and tag {tag}");

    let _register = match client.create_register(xorname, tag).await {
        Ok(register) => {
            println!("Successfully created register '{name}' at {xorname:?}, {tag}!");
            register
        }
        Err(error) => {
            panic!("Did not create register '{name}' on all nodes in the close group! {error}")
        }
    };
    Ok(())
}

async fn edit_register(name: String, entry: String, client: &Client) -> Result<()> {
    let tag = 3006;
    let xorname = XorName::from_content(name.as_bytes());
    println!("Trying to retrieve Register from {xorname:?}, {tag}");

    match client.get_register(xorname, tag).await {
        Ok(mut register) => {
            println!(
                "Successfully retrieved Register '{name}' from {}, {}!",
                register.name(),
                register.tag()
            );
            println!("Editing Register '{name}' with: {entry}");
            match register.write(entry.as_bytes()).await {
                Ok(()) => {}
                Err(ref err @ ClientError::ContentBranchDetected(ref branches)) => {
                    println!(
                        "We need to merge {} branches in Register entries: {err}",
                        branches.len()
                    );
                    register.write_merging_branches(entry.as_bytes()).await?;
                }
                Err(err) => return Err(err.into()),
            }
        }
        Err(error) => {
            panic!("Did not retrieve Register '{name}' from all nodes in the close group! {error}")
        }
    }

    Ok(())
}

async fn get_registers(names: Vec<String>, client: &Client) -> Result<()> {
    let tag = 3006;
    for name in names {
        println!("Register nickname passed in via --query-register is '{name}'...");
        let xorname = XorName::from_content(name.as_bytes());

        println!("Trying to retrieve Register from {xorname:?}, {tag}");

        match client.get_register(xorname, tag).await {
            Ok(register) => println!(
                "Successfully retrieved Register '{name}' from {}, {}!",
                register.name(),
                register.tag()
            ),
            Err(error) => {
                panic!(
                    "Did not retrieve Register '{name}' from all nodes in the close group! {error}"
                )
            }
        }
    }

    Ok(())
}
