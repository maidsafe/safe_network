// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bincode::serialize;
use bls::SecretKey;
use bytes::Bytes;
use clap::Parser;
use eyre::Result;
use safenode::{
    client::{Client, ClientEvent},
    log::init_node_logging,
    protocol::{
        messages::{CreateRegister, EditRegister, SignedRegisterCreate, SignedRegisterEdit},
        types::{
            address::{ChunkAddress, RegisterAddress},
            authority::DataAuthority,
            chunk::Chunk,
            register::{Policy, Register, User},
        },
    },
};
use std::{collections::BTreeSet, fs, path::PathBuf};
use tracing::{info, warn};
use walkdir::WalkDir;
use xor_name::XorName;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    info!("Instantiating a SAFE client...");
    let client = Client::new()?;

    let mut client_events_rx = client.events_channel();
    if let Ok(event) = client_events_rx.recv().await {
        match event {
            ClientEvent::ConnectedToNetwork => {
                info!("Client connected to the Network");
            }
        }
    }

    if let Some(files_path) = opt.upload_chunks {
        for entry in WalkDir::new(files_path).into_iter().flatten() {
            if entry.file_type().is_file() {
                let file = fs::read(entry.path())?;
                let chunk = Chunk::new(Bytes::from(file));
                let xor_name = *chunk.name();
                info!(
                    "Storing chunk {:?} with xorname: {xor_name:x}",
                    entry.file_name()
                );
                println!(
                    "Storing chunk {:?} with xorname: {xor_name:x}",
                    entry.file_name()
                );
                match client.store_chunk(chunk).await {
                    Ok(()) => info!("Successfully stored chunk"),
                    Err(error) => {
                        warn!("Did not store chunk to all nodes in the close group! {error}")
                    }
                };
            }
        }
    }

    if let Some(xor_name) = opt.get_chunk {
        info!("Trying to get chunk {xor_name}...");
        let vec = hex::decode(xor_name).expect("Failed to decode xorname!");
        let mut xor_name = XorName::default();
        xor_name.0.copy_from_slice(vec.as_slice());

        match client.get_chunk(ChunkAddress::new(xor_name)).await {
            Ok(chunk) => info!("Successfully got chunk {}!", chunk.name()),
            Err(error) => warn!("Did not get any chunk from the close group! {error}"),
        };
    }

    if opt.create_register {
        let mut rng = rand::thread_rng();
        let xor_name = XorName::random(&mut rng);
        info!("Creating Register with xorname: {xor_name:x}");

        let sk = SecretKey::random();
        let owner = User::Key(sk.public_key());
        let policy = Policy {
            owner,
            permissions: Default::default(),
        };
        let tag = 3006;
        let op = CreateRegister {
            name: xor_name,
            tag,
            policy: policy.clone(),
        };
        let auth = DataAuthority {
            public_key: sk.public_key(),
            signature: sk.sign(serialize(&op)?),
        };

        let cmd = SignedRegisterCreate { op, auth };

        match client.create_register(cmd).await {
            Ok(()) => info!("Successfully created register {xor_name}, {tag}!"),
            Err(error) => warn!("Did not create register on all nodes in the close group! {error}"),
        };

        if let Some(entry) = opt.entry {
            let mut register = Register::new(owner, xor_name, tag, policy);
            let (_, edit) = register.write(entry.into(), BTreeSet::default())?;
            let op = EditRegister {
                address: *register.address(),
                edit,
            };
            let auth = DataAuthority {
                public_key: sk.public_key(),
                signature: sk.sign(serialize(&op)?),
            };

            let cmd = SignedRegisterEdit { op, auth };

            match client.edit_register(cmd).await {
                Ok(()) => info!("Successfully edited register {xor_name}, {tag}!"),
                Err(error) => {
                    warn!("Did not edit register on all nodes in the close group! {error}")
                }
            };
        }
    }

    if let Some(xor_name) = opt.query_register {
        info!("Trying to retrieve Register");
        let vec = hex::decode(xor_name).expect("failed to decode xorname");
        let mut xor_name = XorName::default();
        xor_name.0.copy_from_slice(vec.as_slice());
        let tag = 3006;

        let addr = RegisterAddress::new(xor_name, tag);

        match client.get_register(addr).await {
            Ok(register) => info!(
                "Successfully retrieved Register {}, {}!",
                register.name(),
                register.tag()
            ),
            Err(error) => {
                warn!("Did not retrieve Register from all nodes in the close group! {error}")
            }
        };
    }

    Ok(())
}

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
    create_register: bool,

    #[clap(long)]
    entry: Option<String>,

    #[clap(long)]
    query_register: Option<String>,
}
