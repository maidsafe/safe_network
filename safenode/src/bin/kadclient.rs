// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

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
use dirs_next::home_dir;
use eyre::Result;
use libp2p::PeerId;
use std::{fs, path::PathBuf, thread, time};
use tracing::{info, warn};
use walkdir::WalkDir;
use xor_name::{XorName, XOR_NAME_LEN};

#[derive(Parser, Debug)]
#[clap(name = "safeclient cli")]
struct Opt {
    #[clap(long)]
    log_dir: Option<PathBuf>,
    /// The location of the wallet file.
    #[clap(long)]
    wallet_dir: Option<PathBuf>,
    /// Tries to load a hex encoded `Dbc` from the
    /// given path and deposit it to the wallet.
    #[clap(long)]
    deposit: Option<PathBuf>,
    /// This must be a hex-encoded `PublicAddress`.
    #[clap(long)]
    send_to: Option<String>,
    /// This shall be the number of nanos to send.
    /// Necessary if the `send_to` argument has been given.
    #[clap(long)]
    send_amount: Option<String>,

    #[clap(long)]
    upload_chunks: Option<PathBuf>,

    #[clap(long)]
    get_chunk: Option<String>,

    #[clap(long)]
    create_register: Option<String>,

    #[clap(long)]
    entry: Option<String>,

    #[clap(long)]
    query_register: Vec<String>,

    #[clap(long)]
    repeated: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    info!("Instantiating a SAFE client...");

    let secret_key = bls::SecretKey::random();
    let client = Client::new(secret_key)?;
    let file_api: Files = Files::new(client.clone());

    let mut client_events_rx = client.events_channel();
    if let Ok(event) = client_events_rx.recv().await {
        match event {
            ClientEvent::ConnectedToNetwork => {
                info!("Client connected to the Network");
            }
        }
    }

    wallet(&opt, &client).await?;
    files(&opt, file_api).await?;
    registers(&opt, client).await?;

    Ok(())
}

async fn wallet(opt: &Opt, client: &Client) -> Result<()> {
    let wallet_dir = opt.wallet_dir.clone().unwrap_or(get_client_dir().await?);
    let mut wallet = LocalWallet::load_from(&wallet_dir).await?;

    if let Some(deposit_path) = &opt.deposit {
        let mut deposits = vec![];

        for entry in WalkDir::new(deposit_path).into_iter().flatten() {
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
    }

    if let Some(hex) = &opt.send_to {
        let address = parse_public_address(hex)?;

        let amount_str = match &opt.send_amount {
            Some(amount_str) => amount_str,
            None => panic!("An amount is expected when an address has been specified!"),
        };
        use std::str::FromStr;
        let amount = Token::from_str(amount_str)?;

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
    }

    Ok(())
}

async fn get_client_dir() -> Result<PathBuf> {
    let mut home_dirs = home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("client");
    tokio::fs::create_dir_all(home_dirs.as_path()).await?;
    Ok(home_dirs)
}

async fn files(opt: &Opt, file_api: Files) -> Result<()> {
    let mut chunks_to_fetch = Vec::new();

    if let Some(files_path) = &opt.upload_chunks {
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
                        panic!("Did not store file {file_name:?} to all nodes in the close group! {error}")
                    }
                };
            }
        }
    }

    if let Some(input_str) = &opt.get_chunk {
        println!("String passed in via get_chunk is {input_str}...");
        if input_str.len() == 64 {
            let vec = hex::decode(input_str).expect("Failed to decode xorname!");
            let mut xorname = XorName::default();
            xorname.0.copy_from_slice(vec.as_slice());
            chunks_to_fetch.push(xorname);
        }

        for xorname in chunks_to_fetch.iter() {
            println!("Downloading file {xorname:?}");
            match file_api.read_bytes(ChunkAddress::new(*xorname)).await {
                Ok(bytes) => info!("Successfully got file {xorname} of {} bytes!", bytes.len()),
                Err(error) => {
                    panic!("Did not get file {xorname:?} from the network! {error}")
                }
            };
        }
    }

    Ok(())
}

async fn registers(opt: &Opt, client: Client) -> Result<()> {
    if let Some(reg_nickname) = &opt.create_register {
        let xorname = XorName::from_content(reg_nickname.as_bytes());
        let tag = 3006;
        println!("Creating Register with '{reg_nickname}' at xorname: {xorname:x} and tag {tag}");

        let mut reg_replica = match client.create_register(xorname, tag).await {
            Ok(replica) => {
                info!("Successfully created register '{reg_nickname}' at {xorname:?}, {tag}!");
                replica
            }
            Err(error) => panic!(
                "Did not create register '{reg_nickname}' on all nodes in the close group! {error}"
            ),
        };

        if let Some(entry) = &opt.entry {
            println!("Editing Register '{reg_nickname}' with: {entry}");
            match reg_replica.write(entry.as_bytes()).await {
                Ok(()) => {}
                Err(ref err @ ClientError::ContentBranchDetected(ref branches)) => {
                    println!(
                        "We need to merge {} branches in Register entries: {err}",
                        branches.len()
                    );
                    reg_replica.write_merging_branches(entry.as_bytes()).await?;
                }
                Err(err) => return Err(err.into()),
            }
        }
    }

    if !opt.query_register.is_empty() {
        let tag = 3006;
        for reg_nickname in opt.query_register.iter() {
            println!("Register nickname passed in via --query-register is '{reg_nickname}'...");
            let xorname = XorName::from_content(reg_nickname.as_bytes());

            println!("Trying to retrieve Register from {xorname:?}, {tag}");

            match client.get_register(xorname, tag).await {
                Ok(register) => println!(
                    "Successfully retrieved Register '{reg_nickname}' from {}, {}!",
                    register.name(),
                    register.tag()
                ),
                Err(error) => {
                    panic!("Did not retrieve Register '{reg_nickname}' from all nodes in the close group! {error}")
                }
            }
        }
    }

    // Repeatedly try to get_closest targeting one of the node for every 10s
    if opt.repeated {
        let dst = xor_name::rand::random();
        let closest = client.get_closest(dst).await;
        let target = if closest.is_empty() {
            panic!("Client cann't find closest nodes to {dst:?}");
        } else {
            let target = to_xorname(closest[0]);
            println!("Converted PeerId {:?} to {target:?}", closest[0]);
            target
        };

        let mut iteration = 0;
        loop {
            iteration += 1;
            thread::sleep(time::Duration::from_secs(10));
            println!("Iteration({iteration}) trying to get closest of {target:?}");
            let closest = client.get_closest(target).await;
            if closest.is_empty() {
                panic!("Client cann't find closest nodes to {target:?}");
            } else {
                println!("Client got closest {closest:?}");
            }
        }
    }

    Ok(())
}

fn to_xorname(peer_id: PeerId) -> XorName {
    let mut xorname_bytes = [0u8; XOR_NAME_LEN];
    let peer_id_bytes = peer_id.to_bytes();
    xorname_bytes.copy_from_slice(&peer_id_bytes[0..32]);
    XorName(xorname_bytes)
}
