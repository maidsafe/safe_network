// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use crate::common::safenode_proto::{
    safe_node_client::SafeNodeClient, GossipsubPublishRequest, GossipsubSubscribeRequest,
    GossipsubUnsubscribeRequest, NodeEventsRequest,
};
use eyre::Result;
use sn_logging::LogBuilder;
use sn_node::NodeEvent;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tonic::Request;

const NODE_COUNT: u8 = 25;
const NODES_SUBSCRIBED: u8 = NODE_COUNT / 2; // 12 out of 25 nodes will be subscribers
const TEST_CYCLES: u8 = 20;

#[tokio::test]
async fn msgs_over_gossipsub() -> Result<()> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("msgs_over_gossipsub");
    let all_nodes_addrs: Vec<_> = (0..NODE_COUNT)
        .map(|i| {
            (
                i,
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001 + i as u16),
            )
        })
        .collect();

    for c in 0..TEST_CYCLES {
        let topic = format!("TestTopic-{}", rand::random::<u64>());
        println!("Testing cicle {}/{TEST_CYCLES} - topic: {topic}", c + 1);
        println!("============================================================");

        // get a random subset of NODES_SUBSCRIBED out of NODE_COUNT nodes to subscribe to the topic
        let mut rng = rand::thread_rng();
        let random_subs_nodes: Vec<_> =
            rand::seq::index::sample(&mut rng, NODE_COUNT.into(), NODES_SUBSCRIBED.into())
                .iter()
                .map(|i| all_nodes_addrs[i])
                .collect();

        let mut subs_handles = vec![];
        for (node_index, addr) in random_subs_nodes.clone() {
            // request current node to subscribe to the topic
            println!("Node #{node_index} ({addr}) subscribing to {topic} ...");
            node_subscribe_to_topic(addr, topic.clone()).await?;

            let handle = tokio::spawn(async move {
                let endpoint = format!("https://{addr}");
                let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
                let response = rpc_client
                    .node_events(Request::new(NodeEventsRequest {}))
                    .await?;

                let mut count = 0;

                let _ = timeout(Duration::from_millis(30000), async {
                    let mut stream = response.into_inner();
                    while let Some(Ok(e)) = stream.next().await {
                        match NodeEvent::from_bytes(&e.event) {
                            Ok(NodeEvent::GossipsubMsg { topic, msg }) => {
                                println!(
                                    "Msg received on node #{node_index} '{topic}': {}",
                                    String::from_utf8(msg.to_vec()).unwrap()
                                );
                                count += 1;
                                if count == NODE_COUNT - NODES_SUBSCRIBED {
                                    break;
                                }
                            }
                            Ok(_) => { /* ignored */ }
                            Err(_) => {
                                println!("Error while parsing received NodeEvent");
                            }
                        }
                    }
                })
                .await;

                Ok::<u8, eyre::Error>(count)
            });

            subs_handles.push((node_index, addr, handle));
        }

        tokio::time::sleep(Duration::from_millis(3000)).await;

        // have all other nodes to publish each a different msg to that same topic
        let mut other_nodes = all_nodes_addrs.clone();
        other_nodes.retain(|node| random_subs_nodes.iter().all(|n| n != node));
        other_nodes_to_publish_on_topic(other_nodes, topic.clone()).await?;

        for (node_index, addr, handle) in subs_handles.into_iter() {
            let count = handle.await??;
            println!("Messages received by node {node_index}: {count}");
            assert_eq!(
                count,
                NODE_COUNT - NODES_SUBSCRIBED,
                "Not enough messages received by node at index {}",
                node_index
            );
            node_unsubscribe_from_topic(addr, topic.clone()).await?;
        }
    }

    Ok(())
}

async fn node_subscribe_to_topic(addr: SocketAddr, topic: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

    // subscribe to given topic
    let _response = rpc_client
        .subscribe_to_topic(Request::new(GossipsubSubscribeRequest { topic }))
        .await?;

    Ok(())
}

async fn node_unsubscribe_from_topic(addr: SocketAddr, topic: String) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

    // unsubscribe from given topic
    let _response = rpc_client
        .unsubscribe_from_topic(Request::new(GossipsubUnsubscribeRequest { topic }))
        .await?;

    Ok(())
}

async fn other_nodes_to_publish_on_topic(
    nodes: Vec<(u8, SocketAddr)>,
    topic: String,
) -> Result<()> {
    for (node_index, addr) in nodes {
        let msg = format!("TestMsgOnTopic-{topic}-from-{node_index}");

        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
        println!("Node {node_index} to publish on {topic} message: {msg}");

        let _response = rpc_client
            .publish_on_topic(Request::new(GossipsubPublishRequest {
                topic: topic.clone(),
                msg: msg.into(),
            }))
            .await?;
    }

    Ok(())
}
