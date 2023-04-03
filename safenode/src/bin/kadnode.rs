// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use assert_fs::TempDir;
use async_std::task::spawn;
use bytes::Bytes;
use clap::Parser;
use eyre::{eyre, Result};
use futures::{prelude::*, StreamExt};
use safenode::{
    log::init_node_logging,
    network::{self, EventLoop},
    storage::{
        chunks::{Chunk, ChunkAddress},
        DataStorage,
    },
};
use std::{fs, path::PathBuf};
use tracing::info;
use walkdir::WalkDir;
use xor_name::XorName;

#[async_std::main]
async fn main() -> Result<()> {
    let _log_appender_guard = init_node_logging(&None)?;
    let opt = Opt::parse();

    let (mut network_client, mut network_events, network_event_loop) = EventLoop::new().await?;
    let temp_dir = TempDir::new()?;
    let storage = DataStorage::new(&temp_dir);

    // Spawn the network task for it to run in the background.
    spawn(network_event_loop.run());

    // In case a listen address was provided use it, otherwise listen on any
    // address.
    // match opt.listen_adress {
    //     Some(addr) => network_client
    //         .start_listening(addr)
    //         .await
    //         .expect("Listening not to fail."),
    //     None => network_client
    //         .start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
    //         .await
    //         .expect("Listening not to fail."),
    // };

    // In case the user provided an address of a peer on the CLI, dial it.
    // if let Some(addr) = opt.peer {
    //     let peer_id = match addr.iter().last() {
    //         Some(Protocol::P2p(hash)) => PeerId::from_multihash(hash).expect("Valid hash."),
    //         _ => return Err("Expect peer multiaddr to contain peer ID.".into()),
    //     };
    //     network_client
    //         .dial(peer_id, addr)
    //         .await
    //         .expect("Dial to succeed");
    // }
    //
    if let Some(files_path) = opt.upload_chunks {
        for entry in WalkDir::new(files_path) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    let file = fs::read(entry.path())?;
                    let chunk = Chunk::new(Bytes::from(file));
                    let xor_name = chunk.name();
                    info!(
                        "Storing file {:?} with xorname: {xor_name:x}",
                        entry.file_name()
                    );
                    storage.store(&chunk).await?;
                    // store the name as key in the network
                    // Advertise oneself as a provider of the file on the DHT.
                    network_client.store_chunk(*xor_name).await;
                }
            }
        }
    }

    let mut client_clone = network_client.clone();
    spawn(async move {
        loop {
            match network_events.next().await {
                // Reply with the content of the file on incoming requests.
                Some(network::Event::InboundChunkRequest { xor_name, channel }) => {
                    let addr = ChunkAddress(xor_name);
                    let chunk = storage.query(&addr).await.unwrap();
                    client_clone
                        .respond_chunk(chunk.value().to_vec(), channel)
                        .await;
                }
                None => continue,
            }
        }
    });

    thread::sleep(time::Duration::from_secs(1));
    if let Some(xor_name) = opt.get_chunk {
        info!("trying to get chunk");
        let vec = hex::decode(xor_name).expect("failed to decode xorname");
        let mut xor_name = XorName::default();
        xor_name.0.copy_from_slice(vec.as_slice());

        // Locate all nodes providing the file.
        let providers = network_client.get_chunk_providers(xor_name).await;
        if providers.is_empty() {
            return Err(eyre!("Could not find provider for file {xor_name}."));
        }
        // Request the content of the file from each node.
        let requests = providers.into_iter().map(|p| {
            let mut network_client = network_client.clone();
            async move { network_client.request_chunk(p, xor_name).await }.boxed()
        });
        // Await the requests, ignore the remaining once a single one succeeds.
        let file_content = futures::future::select_ok(requests)
            .await
            .map_err(|_| eyre!("None of the providers returned file."))?
            .0;
        let chunk = Chunk::new(Bytes::from(file_content));
        info!("got chunk {:x}", chunk.name());

        // std::io::stdout().write_all(&file_content)?;
    }

    use std::{thread, time};
    loop {
        thread::sleep(time::Duration::from_millis(100));
    }
    Ok(())

    // match opt.argument {
    //     // Providing a file.
    //     CliArgument::Provide { path, name } => {
    //     }
    //     // Locating and getting a file.
    //     CliArgument::Get { name } => {

    //     }
    // }
}

#[derive(Parser, Debug)]
#[clap(name = "safenode cli")]
struct Opt {
    #[clap(long)]
    upload_chunks: Option<PathBuf>,

    #[clap(long)]
    get_chunk: Option<String>,
}
