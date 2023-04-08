// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    log::init_node_logging,
    network::Network,
    node::{Client, Node, NodeEvent},
    protocol::{
        messages::{CreateRegister, EditRegister, SignedRegisterCreate, SignedRegisterEdit},
        types::{
            address::ChunkAddress,
            authority::DataAuthority,
            chunk::Chunk,
            register::{Policy, Register, User},
        },
    },
};

use bincode::serialize;
use bls::SecretKey;
use bytes::Bytes;
use clap::Parser;
use eyre::{eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use std::{collections::BTreeSet, fs, path::PathBuf, thread, time};
use tracing::{info, warn};
use walkdir::WalkDir;
use xor_name::XorName;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    info!("Starting a node...");
    let (node, node_events_channel) = Node::run().await?;

    let mut node_events_rx = node_events_channel.subscribe();
    if let Ok(event) = node_events_rx.recv().await {
        match event {
            NodeEvent::ConnectedToNetwork => {
                info!("Connected to the Network");
            }
        }
    }

    let run_node = {
        opt.upload_chunks.is_none()
            && !opt.create_register
            && opt.entry.is_none()
            && opt.get_chunk.is_none()
    };

    if run_node {
        // Do not process client tasks.
        // but keep the node running.
        loop {
            thread::sleep(time::Duration::from_millis(100));
        }
    }

    info!("Instantiating a client...");
    let client = Client::new(node);

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

    // Keep the client running.
    loop {
        thread::sleep(time::Duration::from_millis(100));
    }
}

#[derive(Parser, Debug)]
#[clap(name = "safenode cli")]
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
}

// Todo: Implement node bootstrapping to connect to peers from outside the local network
#[allow(dead_code)]
async fn bootstrap_node(network_api: &mut Network, addr: Multiaddr) -> Result<()> {
    let peer_id = match addr.iter().last() {
        Some(Protocol::P2p(hash)) => PeerId::from_multihash(hash).expect("Valid hash."),
        _ => return Err(eyre!("Expect peer multiaddr to contain peer ID.")),
    };
    network_api
        .dial(peer_id, addr)
        .await
        .expect("Dial to succeed");
    Ok(())
}
