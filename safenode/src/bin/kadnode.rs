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
use eyre::{eyre, Result};
use futures::{channel::oneshot, prelude::*, StreamExt};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use safenode::{
    log::init_node_logging,
    network::{Network, NetworkEvent, NetworkSwarmLoop},
    protocol::{
        messages::{
            CreateRegister, EditRegister, Query, QueryResponse, RegisterCmd, ReplicatedData,
            Request, Response, SignedRegisterCreate, SignedRegisterEdit,
        },
        types::{
            address::ChunkAddress,
            authority::DataAuthority,
            chunk::Chunk,
            register::{Policy, Register, User},
        },
    },
    storage::DataStorage,
};
use std::{collections::BTreeSet, fs, path::PathBuf, thread, time};
use tokio::task::spawn;
use tracing::{info, warn};
use walkdir::WalkDir;
use xor_name::XorName;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    info!("Starting safenode...");

    let (mut network_api, mut network_events, network_event_loop) = NetworkSwarmLoop::new()?;
    let storage = DataStorage::new();

    // Spawn the network task for it to run in the background.
    spawn(network_event_loop.run());

    let mut api_clone = network_api.clone();
    let storage_clone = storage.clone();
    let (peer_dicovered_send, peer_dicovered_rx) = oneshot::channel();
    let mut peer_dicovered_send = Some(peer_dicovered_send);
    spawn(async move {
        loop {
            let event = match network_events.next().await {
                Some(event) => event,
                None => continue,
            };
            match event {
                NetworkEvent::RequestReceived { req, channel } => {
                    // Reply with the content of the file on incoming requests.
                    if let Request::Query(Query::GetChunk(addr)) = req {
                        let chunk = storage_clone.query_chunk(&addr).await.unwrap();
                        if let Err(err) = api_clone
                            .send_response(
                                Response::Query(QueryResponse::GetChunk(Ok(chunk))),
                                channel,
                            )
                            .await
                        {
                            warn!("Error while sending response: {err:?}");
                        }
                    }
                }
                NetworkEvent::PeerDiscovered => {
                    if let Some(sender) = peer_dicovered_send.take() {
                        if let Err(err) = sender.send(()) {
                            warn!("Error while sending through channel: {err:?}");
                        }
                    }
                }
            }
        }
    });

    // wait until we discover atleast one peer
    peer_dicovered_rx.await?;
    info!("Discovered a Peer");
    // todo: sometimes, the node might query the network before it adds a peer to the DHT. The
    // PeerDiscoverd event is triggered when it adds the peer to the DHT, but the op might fail and
    // there is no way to confirm it since `RoutingUpdate` is private/no debug impl.
    // Hence sleep for sometime before querying the network
    thread::sleep(time::Duration::from_millis(100));

    if let Some(files_path) = opt.upload_chunks {
        for entry in WalkDir::new(files_path).into_iter().flatten() {
            if entry.file_type().is_file() {
                let file = fs::read(entry.path())?;
                let chunk = Chunk::new(Bytes::from(file));
                let xor_name = *chunk.name();
                // todo: rework storage
                info!(
                    "Storing file {:?} with xorname: {xor_name:x}",
                    entry.file_name()
                );
                storage.store(&ReplicatedData::Chunk(chunk)).await?;
                // todo: data storage should not use the provider api
                network_api.announce_holding(xor_name).await?;
            }
        }
    }

    if let Some(xor_name) = opt.get_chunk {
        info!("trying to get chunk");
        let vec = hex::decode(xor_name).expect("failed to decode xorname");
        let mut xor_name = XorName::default();
        xor_name.0.copy_from_slice(vec.as_slice());

        // Locate all nodes providing the file.
        let providers = network_api.get_data_providers(xor_name).await?;
        if providers.is_empty() {
            return Err(eyre!("Could not find provider for file {xor_name}."));
        }
        // Request the content of the file from each node.
        let requests = providers.into_iter().map(|peer| {
            let mut network_api = network_api.clone();
            async move {
                let addr = ChunkAddress::new(xor_name);
                network_api
                    .send_request(Request::Query(Query::GetChunk(addr)), peer)
                    .await
            }
            .boxed()
        });
        // Await the requests, ignore the remaining once a single one succeeds.
        let resp = futures::future::select_ok(requests)
            .await
            .map_err(|_| eyre!("None of the providers returned file."))?
            .0;
        if let Response::Query(QueryResponse::GetChunk(Ok(chunk))) = resp {
            info!("got chunk {:x}", chunk.name());
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

        let cmd = RegisterCmd::Create(SignedRegisterCreate { op, auth });

        storage.store(&ReplicatedData::RegisterWrite(cmd)).await?;

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

            let cmd = RegisterCmd::Edit(SignedRegisterEdit { op, auth });

            storage.store(&ReplicatedData::RegisterWrite(cmd)).await?;
        }
    }

    // Keep the node running
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
