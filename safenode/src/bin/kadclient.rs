// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    client::{Client, ClientEvent, Error as ClientError},
    log::init_node_logging,
    protocol::{address::ChunkAddress, chunk::Chunk},
};

use bls::SecretKey;
use bytes::Bytes;
use clap::Parser;
use eyre::Result;
use std::{fs, path::PathBuf};
use tracing::info;
use walkdir::WalkDir;
use xor_name::XorName;

#[derive(Parser, Debug)]
#[clap(name = "safeclient cli")]
struct Opt {
    #[clap(long)]
    log_dir: Option<PathBuf>,

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    info!("Instantiating a SAFE client...");
    // let's build a random secret key to sign our Register ops
    let signer = SecretKey::random();
    let client = Client::new(signer)?;

    let mut client_events_rx = client.events_channel();
    if let Ok(event) = client_events_rx.recv().await {
        match event {
            ClientEvent::ConnectedToNetwork => {
                info!("Client connected to the Network");
            }
        }
    }

    let mut chunks_to_fetch = Vec::new();

    if let Some(files_path) = opt.upload_chunks {
        for entry in WalkDir::new(files_path).into_iter().flatten() {
            if entry.file_type().is_file() {
                let file = fs::read(entry.path())?;
                let chunk = Chunk::new(Bytes::from(file));
                let xorname = *chunk.name();
                info!(
                    "Storing chunk {:?} with xorname: {xorname:x}",
                    entry.file_name()
                );
                println!(
                    "Storing chunk {:?} with xorname: {xorname:x}",
                    entry.file_name()
                );
                match client.store_chunk(chunk).await {
                    Ok(()) => {
                        info!("Successfully stored chunk {xorname:?}");
                        chunks_to_fetch.push(xorname);
                    }
                    Err(error) => {
                        panic!("Did not store chunk {xorname:?} to all nodes in the close group! {error}")
                    }
                };
            }
        }
    }

    if let Some(input_str) = opt.get_chunk {
        println!("String passed in via get_chunk is {input_str}...");
        if input_str.len() == 64 {
            let vec = hex::decode(input_str).expect("Failed to decode xorname!");
            let mut xorname = XorName::default();
            xorname.0.copy_from_slice(vec.as_slice());
            chunks_to_fetch.push(xorname);
        }

        for xorname in chunks_to_fetch.iter() {
            println!("Fetching chunk {xorname:?}");
            match client.get_chunk(ChunkAddress::new(*xorname)).await {
                Ok(chunk) => info!("Successfully got chunk {}!", chunk.name()),
                Err(error) => {
                    panic!("Did not get chunk {xorname:?} from the close group! {error}")
                }
            };
        }
    }

    if let Some(reg_nickname) = opt.create_register {
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

        if let Some(entry) = opt.entry {
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

    Ok(())
}
