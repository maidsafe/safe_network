// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode_proto::safe_node_client::SafeNodeClient;
use safenode_proto::{
    NodeEventsRequest, NodeInfoRequest, RestartRequest, StopRequest, UpdateRequest,
};
use tonic::Request;

use clap::Parser;
use eyre::Result;
use std::{net::SocketAddr, time::Duration};
use tokio_stream::StreamExt;
use xor_name::{XorName, XOR_NAME_LEN};

// this would include code generated from .proto file
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

#[derive(Parser, Debug)]
#[clap(name = "safenode RPC client")]
struct Opt {
    /// Address of the node's RPC service, e.g. 127.0.0.1:12001.
    addr: SocketAddr,
    /// subcommands
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Debug)]
enum Cmd {
    /// Retrieve information about the node itself
    #[clap(name = "info")]
    Info,
    /// Start listening for node events.
    /// Note this blocks the app and it will print events as they are broadcasted by the node
    #[clap(name = "events")]
    Events,
    /// Restart the node after the specified delay
    #[clap(name = "restart")]
    Restart {
        /// Delay in milliseconds before restartng the node
        #[clap(default_value = "0")]
        delay_millis: u64,
    },
    /// Stop the node after the specified delay
    #[clap(name = "stop")]
    Stop {
        /// Delay in milliseconds before stopping the node
        #[clap(default_value = "0")]
        delay_millis: u64,
    },
    /// Update to latest `safenode` released version, and restart it
    #[clap(name = "update")]
    Update {
        /// Delay in milliseconds before updating and restarting the node
        #[clap(default_value = "0")]
        delay_millis: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let addr = opt.addr;

    match opt.cmd {
        Cmd::Info => node_info(addr).await,
        Cmd::Events => node_events(addr).await,
        Cmd::Restart { delay_millis } => node_restart(addr, delay_millis).await,
        Cmd::Stop { delay_millis } => node_stop(addr, delay_millis).await,
        Cmd::Update { delay_millis } => node_update(addr, delay_millis).await,
    }
}

pub async fn node_info(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("http://{addr}");
    let mut client = SafeNodeClient::connect(endpoint.clone()).await?;
    let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
    let node_info = response.get_ref();
    let node_id = xorname_from_bytes(&node_info.node_id);

    println!("Node info received:");
    println!("===================");
    println!("RPC endpoint: {endpoint}");
    println!("Node Id: {node_id:?}");
    println!("Logs dir: {}", node_info.log_dir);
    println!("Binary version: {}", node_info.bin_version);
    println!(
        "Time since last restart: {:?}",
        Duration::from_secs(node_info.uptime_secs)
    );

    Ok(())
}

pub async fn node_events(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("http://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let response = client
        .node_events(Request::new(NodeEventsRequest {}))
        .await?;

    println!("Listening to node events... (press Ctrl+C to exit)");
    let mut stream = response.into_inner();
    while let Some(Ok(e)) = stream.next().await {
        println!("New event received: {}", e.event);
    }

    Ok(())
}

pub async fn node_restart(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("http://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .restart(Request::new(RestartRequest { delay_millis }))
        .await?;
    println!(
        "Node successfully received the request to restart in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

pub async fn node_stop(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("http://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .stop(Request::new(StopRequest { delay_millis }))
        .await?;
    println!(
        "Node successfully received the request to stop in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

pub async fn node_update(addr: SocketAddr, delay_millis: u64) -> Result<()> {
    let endpoint = format!("http://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;
    let _response = client
        .update(Request::new(UpdateRequest { delay_millis }))
        .await?;
    println!(
        "Node successfully received the request to try to update in {:?}",
        Duration::from_millis(delay_millis)
    );
    Ok(())
}

fn xorname_from_bytes(bytes: &[u8]) -> XorName {
    let mut xorname = [0u8; XOR_NAME_LEN];
    bytes.iter().enumerate().for_each(|(i, b)| xorname[i] = *b);
    XorName(xorname)
}
