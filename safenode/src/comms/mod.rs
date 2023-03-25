// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod error;
mod listener;
mod node_link;

pub use self::error::{Error, Result};

use self::node_link::{NodeLink, NodeLinkError};

use bytes::Bytes;
use custom_debug::Debug;
use futures::future::join_all;
use qp2p::{Endpoint, SendStream};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task,
};
use tracing::{debug, error, trace, warn};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MsgId(u64);
pub trait MsgTrait:
    Default + Send + Clone + std::fmt::Debug + Serialize + for<'a> Deserialize<'a>
{
}

impl MsgId {
    /// Generates a new `MsgId` with random content.
    pub fn new() -> Self {
        Self(rand::random())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkMsg<T> {
    pub id: MsgId,
    pub payload: T,
}

impl<T: MsgTrait> NetworkMsg<T> {
    pub fn from_bytes(value: Bytes) -> Result<Self> {
        Ok(bincode::deserialize(&value)?)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(bincode::serialize(self)?.into())
    }

    pub fn error_msg() -> NetworkMsg<T> {
        NetworkMsg {
            id: MsgId::new(),
            payload: Default::default(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct NetworkNode {
    /// Network participant address
    pub addr: SocketAddr,
}

/// Standard channel size, to allow for large swings in throughput
static STANDARD_CHANNEL_SIZE: usize = 100_000;

/// Events from the comm module.
#[derive(Debug)]
pub enum CommEvent<T> {
    /// A msg was received.
    Msg(MsgReceived<T>),
    /// A send error occurred.
    Error {
        /// The sender/recipient that failed.
        node_id: NetworkNode,
        /// The failure type.
        error: Error,
    },
}

/// A msg received on the wire.
#[derive(Debug)]
pub struct MsgReceived<T> {
    /// The socketaddr of sender of the msg.
    pub sender: SocketAddr,
    /// The msg that we received.
    pub wire_msg: NetworkMsg<T>,
    /// An optional stream to return msgs on, if
    /// this msg came on a bidi-stream.
    pub send_stream: Option<SendStream>,
}

/// Communication component of the node to interact with other nodes.
///
/// Any failed sends are tracked via `CommEvent::Error`, which will track issues for any nodes
/// in the section (otherwise ignoring failed send to out of section nodes or clients).
#[derive(Clone, Debug)]
pub struct Comm {
    our_endpoint: Endpoint,
    cmd_sender: Sender<CommCmd>,
}

impl Comm {
    /// Creates a new instance of Comm with an endpoint
    /// and starts listening to the incoming messages from other nodes.
    #[tracing::instrument(skip_all)]
    pub fn new<T: MsgTrait + 'static>() -> Result<(Self, Receiver<CommEvent<T>>)> {
        let (our_endpoint, incoming_conns) = Endpoint::builder().server()?;

        trace!("Creating comms..");
        // comm_events_receiver will be used by upper layer to receive all msgs coming in from the network
        // capacity of one as we limit w/ how many cmds we process in the upper layers.
        // any higher and we're not feeding back directly to incoming msgs...
        // (we may want some buffer here?)
        let (comm_events_sender, comm_events_receiver) = mpsc::channel(STANDARD_CHANNEL_SIZE);
        let (cmd_sender, cmd_receiver) = mpsc::channel(STANDARD_CHANNEL_SIZE);

        // listen for msgs/connections to our endpoint
        listener::listen_for_connections(comm_events_sender.clone(), incoming_conns);

        process_cmds(our_endpoint.clone(), cmd_receiver, comm_events_sender);

        Ok((
            Self {
                our_endpoint,
                cmd_sender,
            },
            comm_events_receiver,
        ))
    }

    /// The socket address of our endpoint.
    pub fn socket_addr(&self) -> SocketAddr {
        self.our_endpoint.local_addr()
    }

    /// Closes the endpoint.
    pub fn close_endpoint(&self) {
        self.our_endpoint.close()
    }

    /// Sets the available targets to be only those in the passed in set.
    pub async fn set_comm_targets(&self, targets: BTreeSet<NetworkNode>) {
        debug!("Setting targets to: {targets:?}");
        // We only remove links by calling this function,
        // No removals are made even if we failed to send using all node link's connections,
        // as it's our source of truth for known and connectable nodes.
        self.send_cmd(CommCmd::SetTargets(targets)).await;
    }

    /// Sends the payload on a new bidi-stream and pushes any response onto the comm event channel.
    #[tracing::instrument(skip(self, bytes))]
    pub async fn send_msg(&self, node_id: NetworkNode, msg_id: MsgId, bytes: Bytes) {
        self.send_cmd(CommCmd::SendAndReturnResponse {
            msg_id,
            node_id,
            bytes,
        })
        .await;
    }

    /// Sends the payload on new bidi-stream to noe and sends the response on the dst stream.
    #[tracing::instrument(skip(self, node_bytes))]
    pub async fn send_and_respond_on_stream(
        &self,
        msg_id: MsgId,
        node_bytes: BTreeMap<NetworkNode, Bytes>,
        expected_targets: usize,
        dst_stream: (NetworkNode, SendStream),
    ) {
        self.send_cmd(CommCmd::SendAndRespondOnStream {
            msg_id,
            node_bytes,
            expected_targets,
            dst_stream,
        })
        .await;
    }

    async fn send_cmd(&self, cmd: CommCmd) {
        let sender = self.cmd_sender.clone();
        let error_msg = format!("Failed to send {cmd:?} on comm cmd channel ");
        if let Err(error) = sender.send(cmd).await {
            error!("{error_msg} due to {error}.");
        }
    }
}

/// Internal comm cmds.
#[derive(custom_debug::Debug)]
enum CommCmd {
    SetTargets(BTreeSet<NetworkNode>),
    SendAndReturnResponse {
        node_id: NetworkNode,
        msg_id: MsgId,
        #[debug(skip)]
        bytes: Bytes,
    },
    SendAndRespondOnStream {
        msg_id: MsgId,
        #[debug(skip)]
        node_bytes: BTreeMap<NetworkNode, Bytes>,
        expected_targets: usize,
        dst_stream: (NetworkNode, SendStream),
    },
}

fn process_cmds<T: MsgTrait + 'static>(
    our_endpoint: Endpoint,
    mut cmd_receiver: Receiver<CommCmd>,
    comm_events: Sender<CommEvent<T>>,
) {
    let _handle = task::spawn(async move {
        let mut links = BTreeMap::<NetworkNode, NodeLink>::new();
        while let Some(cmd) = cmd_receiver.recv().await {
            trace!("Comms cmd handling: {cmd:?}");
            match cmd {
                // This is the only place that mutates `links`.
                CommCmd::SetTargets(targets) => {
                    // Drops links that are not among the targets.
                    links.retain(|p, _| targets.contains(p));
                    // Adds new links for each new target.
                    targets.iter().for_each(|node_id| {
                        if links.get(node_id).is_none() {
                            let link = NodeLink::new(*node_id, our_endpoint.clone());

                            debug!("inserting link to {node_id:?}");
                            let _ = links.insert(*node_id, link);
                        }
                    });
                }
                CommCmd::SendAndReturnResponse {
                    node_id,
                    msg_id,
                    bytes,
                } => {
                    if let Some(link) = get_link(msg_id, node_id, &links, comm_events.clone()) {
                        send_and_return_response(msg_id, link, bytes, comm_events.clone())
                    }
                }
                CommCmd::SendAndRespondOnStream {
                    msg_id,
                    node_bytes,
                    expected_targets,
                    dst_stream,
                } => {
                    let node_bytes = node_bytes
                        .into_iter()
                        .map(|(node_id, bytes)| {
                            let link = get_link(msg_id, node_id, &links, comm_events.clone());
                            (node_id, (link, bytes))
                        })
                        .collect();

                    send_and_respond_on_stream(
                        msg_id,
                        node_bytes,
                        expected_targets,
                        dst_stream,
                        comm_events.clone(),
                    )
                }
            }
        }
    });
}

fn get_link<T: MsgTrait + 'static>(
    msg_id: MsgId,
    node_id: NetworkNode,
    links: &BTreeMap<NetworkNode, NodeLink>,
    comm_events: Sender<CommEvent<T>>,
) -> Option<NodeLink> {
    debug!("Links len: {:?}", links.len());
    debug!("Trying to get {node_id:?} link in order to send: {msg_id:?}");
    match links.get(&node_id) {
        Some(link) => Some(link.clone()),
        None => {
            error!("Sending message (msg_id: {msg_id:?}) to {node_id:?} failed: unknown node.");
            send_error(
                node_id,
                Error::ConnectingToUnknownNode(node_id),
                comm_events,
            );
            None
        }
    }
}

#[tracing::instrument(skip_all)]
fn send<T: MsgTrait + 'static>(
    msg_id: MsgId,
    mut link: NodeLink,
    bytes: Bytes,
    comm_events: Sender<CommEvent<T>>,
) {
    let _handle = task::spawn(async move {
        let bytes_len = bytes.len();
        let node_id = link.node();
        trace!("Sending message bytes ({bytes_len} bytes) w/ {msg_id:?} to {node_id:?}");
        match link.send(msg_id, bytes).await {
            Ok(()) => {
                trace!("Msg {msg_id:?} sent to {node_id:?}");
            }
            Err(error) => {
                error!("Sending message (msg_id: {msg_id:?}) to {node_id:?} failed: {error}");
                send_error(node_id, Error::FailedSend(msg_id), comm_events.clone());
            }
        }
    });
}

#[tracing::instrument(skip_all)]
fn send_and_return_response<T: MsgTrait + 'static>(
    msg_id: MsgId,
    link: NodeLink,
    bytes: Bytes,
    comm_events: Sender<CommEvent<T>>,
) {
    let _handle = task::spawn(async move {
        let bytes_len = bytes.len();
        let node_id = link.node();
        trace!("Sending message bytes ({bytes_len} bytes) w/ {msg_id:?} to {node_id:?}");

        let node_response_bytes = match link.send_with_bi_return_response(bytes, msg_id).await {
            Ok(response_bytes) => {
                debug!("Node response from {node_id:?} is in for {msg_id:?}");
                response_bytes
            }
            Err(error) => {
                if let NodeLinkError::Recv(e) = error {
                    // we don't _have_ to have a response.
                    // so a recv error can be okay here...
                    return;
                }
                error!("Sending message (msg_id: {msg_id:?}) to {node_id:?} failed: {error}");
                send_error(node_id, Error::FailedSend(msg_id), comm_events.clone());
                return;
            }
        };
        match NetworkMsg::from_bytes(node_response_bytes) {
            Ok(wire_msg) => {
                listener::msg_received(wire_msg, node_id, None, comm_events.clone()).await;
            }
            Err(error) => {
                error!("Failed sending {msg_id:?} to {node_id:?}: {error:?}");
                send_error(
                    node_id,
                    Error::InvalidMsgReceived(msg_id),
                    comm_events.clone(),
                );
            }
        };
    });
}

#[tracing::instrument(skip_all)]
fn send_and_respond_on_stream<T: MsgTrait + 'static>(
    msg_id: MsgId,
    node_bytes: BTreeMap<NetworkNode, (Option<NodeLink>, Bytes)>,
    expected_targets: usize,
    dst_stream: (NetworkNode, SendStream),
    comm_events: Sender<CommEvent<T>>,
) {
    let _handle = task::spawn(async move {
        let (dst, stream) = dst_stream;

        let tasks = node_bytes
            .into_iter()
            .map(|pb| (pb, comm_events.clone()))
            .map(|((node_id, (link, bytes)), comm_events)| async move {
                let link = match link {
                    Some(link) => link,
                    None => return (node_id, Err(Error::ConnectingToUnknownNode(node_id))),
                };

                let node_response_bytes =
                    match link.send_with_bi_return_response(bytes, msg_id).await {
                        Ok(response_bytes) => response_bytes,
                        Err(error) => {
                            error!("Failed sending {msg_id:?} to {node_id:?}: {error:?}");
                            send_error(node_id, Error::FailedSend(msg_id), comm_events);
                            return (node_id, Err(Error::FailedSend(msg_id)));
                        }
                    };

                debug!("Response from node {node_id:?} is in for {msg_id:?}");
                (node_id, Ok(node_response_bytes))
            });

        let node_results: Vec<(NetworkNode, Result<Bytes>)> = join_all(tasks).await;

        let succeeded: Vec<_> = node_results
            .into_iter()
            .filter_map(|(node_id, res)| match res {
                Ok(bytes) => Some((node_id, bytes)),
                Err(error) => {
                    error!("Failed sending {msg_id:?} to {node_id:?}: {error:?}");
                    send_error(node_id, Error::FailedSend(msg_id), comm_events.clone());
                    None
                }
            })
            .collect();

        let some_failed = expected_targets > succeeded.len();
        let all_ok_equal = || succeeded.windows(2).all(|w| are_equal(&w[0].1, &w[1].1));

        let response_bytes = if some_failed || !all_ok_equal() {
            match error_response::<T>(dst) {
                None => {
                    error!("Could not send the error response to client!");
                    return;
                }
                Some(bytes) => bytes,
            }
        } else {
            match succeeded.last() {
                Some((_, bytes)) => bytes.clone(),
                _ => {
                    error!("Could not send the response to client!");
                    return;
                }
            }
        };

        send_on_stream(msg_id, response_bytes, stream).await;
    });
}

#[tracing::instrument(skip_all)]
fn send_error<T: MsgTrait + 'static>(
    node_id: NetworkNode,
    error: Error,
    comm_events: Sender<CommEvent<T>>,
) {
    let _handle = task::spawn(async move {
        let error_msg =
            format!("Failed to send error {error} of node {node_id:?} on comm event channel ");
        if let Err(err) = comm_events.send(CommEvent::Error { node_id, error }).await {
            error!("{error_msg} due to {err}.")
        }
    });
}

#[tracing::instrument(skip_all)]
async fn send_on_stream(msg_id: MsgId, bytes: Bytes, mut stream: SendStream) {
    let placeholder = Bytes::from("placeholder");
    match stream
        .send_user_msg((placeholder.clone(), placeholder.clone(), bytes))
        .await
    {
        Ok(()) => trace!("Response to {msg_id:?} sent to client."),
        Err(error) => error!("Could not send the response to {msg_id:?} to client due to {error}!"),
    }
}

fn error_response<T: MsgTrait>(_dst: NetworkNode) -> Option<Bytes> {
    let wire_msg = NetworkMsg::<T>::error_msg();
    wire_msg.to_bytes().ok()
}

#[tracing::instrument(skip_all)]
fn are_equal(a: &Bytes, b: &Bytes) -> bool {
    are_bytes_equal(a.to_vec(), b.to_vec())
}

#[tracing::instrument(skip_all)]
fn are_bytes_equal(one: Vec<u8>, other: Vec<u8>) -> bool {
    if one.len() != other.len() {
        return false;
    }
    for (a, b) in one.into_iter().zip(other) {
        if a != b {
            return false;
        }
    }
    true
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     use sn_interface::{
//         messaging::{
//             data::{ClientMsg, DataQuery},
//             ClientAuth, NetworkNode, MsgId, MsgKind,
//         },
//         types::{ChunkAddress, Keypair, NodeId},
//     };

//     use assert_matches::assert_matches;
//     use eyre::Result;
//     use futures::future;
//     use std::{net::Ipv4Addr, time::Duration};
//     use tokio::{
//         net::UdpSocket,
//         sync::mpsc::{self, Receiver},
//         time,
//     };

//     const TIMEOUT: Duration = Duration::from_secs(1);

//     #[tokio::test]
//     async fn successful_send() -> Result<()> {
//         let (comm, _rx) = Comm::new(local_addr(), None)?;

//         let (node0, mut rx0) = new_node_id().await?;
//         let (node1, mut rx1) = new_node_id().await?;

//         // add nodes as known members
//         comm.set_comm_targets([node0, node1].into());

//         let node0_msg = new_test_msg(dst(node0))?;
//         let node1_msg = new_test_msg(dst(node1))?;

//         comm.send_out_bytes(node0, node0_msg.msg_id(), node0_msg.serialize()?);
//         comm.send_out_bytes(node1, node1_msg.msg_id(), node1_msg.serialize()?);

//         if let Some(bytes) = rx0.recv().await {
//             assert_eq!(NetworkMsg::from_bytes(bytes)?, node0_msg);
//         }

//         if let Some(bytes) = rx1.recv().await {
//             assert_eq!(NetworkMsg::from_bytes(bytes)?, node1_msg);
//         }

//         Ok(())
//     }

//     #[tokio::test]
//     async fn failed_send() -> Result<()> {
//         let (comm, mut rx) = Comm::new(local_addr(), None)?;

//         let invalid_dst = get_invalid_node().await?;
//         let invalid_addr = invalid_dst.addr();
//         let msg = new_test_msg(dst(invalid_dst))?;
//         comm.send_out_bytes(invalid_dst, msg.msg_id(), msg.serialize()?);

//         if let Some(CommEvent::Error { node_id, error }) = rx.recv().await {
//             // the node is still not set as a known member thus it should have failed
//             assert_matches!(error, Error::ConnectingToUnknownNode(_));
//             assert_eq!(node_id.addr(), invalid_addr);
//         }

//         // let's add the node as a known member and check again
//         comm.set_comm_targets([invalid_dst].into());

//         comm.send_out_bytes(invalid_dst, msg.msg_id(), msg.serialize()?);

//         if let Some(CommEvent::Error { node_id, error }) = rx.recv().await {
//             assert_matches!(error, Error::FailedSend(_));
//             assert_eq!(node_id.addr(), invalid_addr);
//         }

//         Ok(())
//     }

//     #[tokio::test]
//     async fn send_after_reconnect() -> Result<()> {
//         let (send_comm, _rx) = Comm::new(local_addr(), None)?;

//         let (recv_endpoint, mut incoming_connections) =
//             Endpoint::builder().addr(local_addr()).server()?;
//         let recv_addr = recv_endpoint.local_addr();
//         let name = xor_name::rand::random();
//         let node_id = NetworkNode::new(name, recv_addr);
//         let msg0 = new_test_msg(dst(node_id))?;

//         // add node as a known member
//         send_comm.set_comm_targets([node_id].into());

//         send_comm.send_out_bytes(node_id, msg0.msg_id(), msg0.serialize()?);

//         let mut msg0_received = false;

//         // Receive one message and disconnect from the node
//         {
//             if let Some((_, mut incoming_msgs)) = incoming_connections.next().await {
//                 if let Some(msg) = time::timeout(TIMEOUT, incoming_msgs.next()).await?? {
//                     assert_eq!(NetworkMsg::from_bytes(msg.0)?, msg0);
//                     msg0_received = true;
//                 }
//                 // connection dropped here
//             }
//             assert!(msg0_received);
//         }

//         let msg1 = new_test_msg(dst(node_id))?;
//         send_comm.send_out_bytes(node_id, msg1.msg_id(), msg1.serialize()?);

//         let mut msg1_received = false;

//         if let Some((_, mut incoming_msgs)) = incoming_connections.next().await {
//             if let Some(msg) = time::timeout(TIMEOUT, incoming_msgs.next()).await?? {
//                 assert_eq!(NetworkMsg::from_bytes(msg.0)?, msg1);
//                 msg1_received = true;
//             }
//         }

//         assert!(msg1_received);

//         Ok(())
//     }

//     #[tokio::test]
//     async fn incoming_connection_lost() -> Result<()> {
//         let (comm0, mut rx0) = Comm::new(local_addr(), None)?;
//         let addr0 = comm0.socket_addr();

//         let (comm1, _rx1) = Comm::new(local_addr(), None)?;

//         let node_id = NetworkNode::new(xor_name::rand::random(), addr0);
//         let msg = new_test_msg(dst(node_id))?;

//         // add node as a known member
//         comm1.set_comm_targets([node_id].into());

//         // Send a message to establish the connection
//         comm1.send_out_bytes(node_id, msg.msg_id(), msg.serialize()?);

//         assert_matches!(rx0.recv().await, Some(CommEvent::Msg(MsgReceived { .. })));

//         // Drop `comm1` to cause connection lost.
//         drop(comm1);

//         assert_matches!(time::timeout(TIMEOUT, rx0.recv()).await, Err(_));

//         Ok(())
//     }

//     fn dst(node_id: NetworkNode) -> NetworkNode {
//         NetworkNode {
//             name: node_id.name(),
//             section_key: bls::SecretKey::random().public_key(),
//         }
//     }

//     fn new_test_msg(dst: NetworkNode) -> Result<NetworkMsg> {
//         let src_keypair = Keypair::new_ed25519();

//         let query = DataQuery::GetChunk(ChunkAddress(xor_name::rand::random()));
//         let query = ClientMsg::Query(query);
//         let payload = NetworkMsg::serialize_msg_payload(&query)?;

//         let auth = ClientAuth {
//             public_key: src_keypair.public_key(),
//             signature: src_keypair.sign(&payload),
//         };

//         Ok(NetworkMsg::new_msg(
//             MsgId::new(),
//             payload,
//             MsgKind::Client {
//                 auth,
//                 is_spend: false,
//                 query_index: None,
//             },
//             dst,
//         ))
//     }

//     async fn new_node_id() -> Result<(NetworkNode, Receiver<Bytes>)> {
//         let (endpoint, mut incoming_connections) =
//             Endpoint::builder().addr(local_addr()).server()?;
//         let addr = endpoint.local_addr();

//         let (tx, rx) = mpsc::channel(1);

//         let _handle = tokio::task::spawn(async move {
//             while let Some((_, mut incoming_messages)) = incoming_connections.next().await {
//                 while let Ok(Some(msg)) = incoming_messages.next().await {
//                     let _ = tx.send(msg.0).await;
//                 }
//             }
//         });

//         Ok((NetworkNode::new(xor_name::rand::random(), addr), rx))
//     }

//     async fn get_invalid_node() -> Result<NetworkNode> {
//         let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await?;
//         let addr = socket.local_addr()?;

//         // Keep the socket alive to keep the address bound, but don't read/write to it so any
//         // attempt to connect to it will fail.
//         let _handle = tokio::task::spawn(async move {
//             debug!("get invalid participant");
//             future::pending::<()>().await;
//             let _ = socket;
//         });

//         Ok(NetworkNode::new(xor_name::rand::random(), addr))
//     }

//     fn local_addr() -> SocketAddr {
//         (Ipv4Addr::LOCALHOST, 0).into()
//     }
// }
