// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{MsgId, NetworkNode, Result};

use bytes::Bytes;
use qp2p::{Connection, Endpoint};

use custom_debug::Debug;
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, instrument, trace, warn};

type ConnId = String;

/// These retries are how may _new_ connection attempts do we make.
/// If we fail all of these, an error is raised, which in turn
/// kicks off fault tracking for section nodes.
const MAX_SENDJOB_RETRIES: usize = 3;

const CONN_RETRY_WAIT: Duration = Duration::from_millis(100);

/// A link to a node in our network.
///
/// Using the link will open a connection if there is none there.
/// The link is a way to keep connections to a node in one place
/// and use them efficiently; converge to a single one regardless of concurrent
/// comms initiation between the nodes, and so on.
/// The link shall be kept around as long as the node is deemed worth to keep contact with.
#[derive(Clone)]
pub(crate) struct NodeLink {
    node: NetworkNode,
    endpoint: Endpoint,
    connections: NodeConnections,
}

type NodeConnections = Arc<DashMap<ConnId, Arc<Connection>>>;

impl NodeLink {
    pub(crate) fn new(node: NetworkNode, endpoint: Endpoint) -> Self {
        Self {
            node,
            endpoint,
            connections: NodeConnections::default(),
        }
    }

    pub(crate) fn node(&self) -> NetworkNode {
        self.node
    }

    /// Sends out a UsrMsg on a bidi connection and awaits response bytes.
    /// As such this may be long running if response is returned slowly.
    /// When sending a msg to a node, if it fails with an existing
    /// cached connection, it will keep retrying till it either:
    /// a. finds another cached connection which it succeeded with,
    /// b. or it cleaned them all up from the cache creating a new connection
    ///    to the node as last attempt.
    pub(crate) async fn send_with_bi_return_response(
        &self,
        bytes: Bytes,
        msg_id: MsgId,
    ) -> Result<Bytes, NodeLinkError> {
        let node = self.node;
        trace!(
            "Sending {msg_id:?} via a bi-stream to {node:?}, we have {} cached connections.",
            self.connections.len()
        );
        let mut attempt = 0;
        loop {
            let conn = self
                .connections
                .iter()
                .next()
                .map(|entry| entry.value().clone());

            let (conn, is_last_attempt) = if let Some(conn) = conn {
                trace!(
                    "Sending {msg_id:?} via bi-di-stream over existing connection {}, attempt #{attempt}.",
                    conn.id()
                );
                (conn, false)
            } else {
                trace!("Sending {msg_id:?} via bi-di-stream over new connection to {node:?}, attempt #{attempt}.");
                let conn =
                    create_connection(node, &self.endpoint, self.connections.clone(), msg_id)
                        .await?;
                (conn, true)
            };

            attempt += 1;

            let conn_id = conn.id();
            trace!("Connection {conn_id} got to {node:?} for {msg_id:?}");
            let (mut send_stream, recv_stream) = match conn.open_bi().await {
                Ok(bi_stream) => bi_stream,
                Err(err) => {
                    error!("{msg_id:?} Error opening bi-stream over {conn_id}: {err:?}");
                    // remove that broken conn
                    let _conn = self.connections.remove(&conn_id);
                    match is_last_attempt {
                        true => {
                            error!("Last attempt reached for {msg_id:?}, erroring out...");
                            break Err(NodeLinkError::Connection(err));
                        }
                        false => {
                            // tiny wait for comms/dashmap to cope with removal
                            sleep(CONN_RETRY_WAIT).await;
                            continue;
                        }
                    }
                }
            };

            let stream_id = send_stream.id();
            trace!("bidi {stream_id} opened for {msg_id:?} to {node:?}");
            send_stream.set_priority(10);
            let sn_bytes_format_wrap = (Default::default(), Default::default(), bytes.clone());
            if let Err(err) = send_stream.send_user_msg(sn_bytes_format_wrap).await {
                error!("Error sending bytes for {msg_id:?} over {stream_id}: {err:?}");
                // remove that broken conn
                let _conn = self.connections.remove(&conn_id);
                match is_last_attempt {
                    true => break Err(NodeLinkError::Send(err)),
                    false => {
                        // tiny wait for comms/dashmap to cope with removal
                        sleep(CONN_RETRY_WAIT).await;
                        continue;
                    }
                }
            }

            trace!("{msg_id:?} sent on {stream_id} to {node:?}");

            // unblock + move finish off thread as it's not strictly related to the sending of the msg.
            let stream_id_clone = stream_id.clone();
            let _handle = tokio::spawn(async move {
                // Attempt to gracefully terminate the stream.
                // If this errors it does _not_ mean our message has not been sent
                let result = send_stream.finish().await;
                trace!("{msg_id:?} finished {stream_id_clone} to {node:?}: {result:?}");
            });

            match recv_stream.read().await {
                Ok((_sn, _bytes, response)) => break Ok(response),
                Err(err) => {
                    error!("Error receiving response to {msg_id:?} from {node:?} over {stream_id}: {err:?}");
                    let _conn = self.connections.remove(&conn_id);
                    if is_last_attempt {
                        break Err(NodeLinkError::Recv(err));
                    }

                    // tiny wait for comms/dashmap to cope with removal
                    sleep(CONN_RETRY_WAIT).await;
                }
            }
        }
    }

    #[instrument(skip(self, bytes))]
    pub(crate) async fn send(&mut self, msg_id: MsgId, bytes: Bytes) -> Result<(), NodeLinkError> {
        let mut connection_retries = 0;

        let node = self.node;

        loop {
            trace!("Sending to {node:?} over connection: {msg_id:?}");

            if connection_retries > MAX_SENDJOB_RETRIES {
                let error_to_report = NodeLinkError::MaxRetriesReached(MAX_SENDJOB_RETRIES);
                debug!("{error_to_report}: {msg_id:?}");
                return Err(error_to_report);
            }

            // Keep this connection creation/retrieval as blocking.
            // This avoids us making many many connection attempts to the same node.
            //
            // If a valid connection exists, retrieval is fast.
            //
            // Attempt to get a connection or make one to another node.
            // if there's no successful connection, we requeue the job after a wait
            // incase there's been a delay adding the connection to Comms
            let conn = match self.get_or_connect(msg_id).await {
                Ok(conn) => conn,
                Err(error) => {
                    error!("Error when attempting to send {msg_id:?} to node. Job will be reenqueued for another attempt after a small timeout: {error:?}");

                    // only increment connection attempts if our connections set is empty
                    // and so we'll be trying to create a fresh connection
                    if self.connections.is_empty() {
                        connection_retries += 1;
                    }

                    // we await here in case the connection is fresh and has not yet been added
                    sleep(CONN_RETRY_WAIT).await;
                    continue;
                }
            };

            let conn_id = conn.id();
            debug!("Connection got for sendjob: {msg_id:?}, with conn_id: {conn_id:?}");

            let send_resp =
                Self::send_with_connection(conn, bytes.clone(), self.connections.clone()).await;

            match send_resp {
                Ok(()) => {
                    return Ok(());
                }
                Err(err) => {
                    if err.is_local_close() {
                        let conns_count = self.connections.len();
                        error!("Node connection dropped when trying to send {msg_id:?} (we still have {conns_count:?} connections): {err:?}");
                        // we can retry if we've more connections!
                        if conns_count <= 1 {
                            debug!(
                                "No connections left on this session to {node:?}, terminating session.",
                            );
                            connection_retries += 1;
                        }
                    }

                    warn!(
                        "Transient error while attempting to send, re-trying job {msg_id:?} {err:?}. Connection id was {conn_id:?}"
                    );

                    // we await here in case the connection is fresh and has not yet been added
                    sleep(CONN_RETRY_WAIT).await;
                }
            }
        }
    }

    // Gets an existing connection or creates a new one
    async fn get_or_connect(&mut self, msg_id: MsgId) -> Result<Arc<Connection>, NodeLinkError> {
        let node = self.node;
        trace!("{msg_id:?} Grabbing a connection to {node:?} from cached set.");

        let conn = self
            .connections
            .iter()
            .next()
            .map(|entry| entry.value().clone());
        if let Some(conn) = conn {
            trace!("{msg_id:?} Connection found to {node:?}");
            Ok(conn)
        } else {
            trace!("{msg_id:?} No connection found to {node:?}, creating a new one.");
            create_connection(node, &self.endpoint, self.connections.clone(), msg_id).await
        }
    }

    /// Send a message to the node using the given connection.
    #[instrument(skip_all)]
    async fn send_with_connection(
        conn: Arc<Connection>,
        bytes: Bytes,
        connections: NodeConnections,
    ) -> Result<(), NodeLinkError> {
        let conn_id = conn.id();
        let conns_count = connections.len();
        trace!("We have {conns_count} open connections to node {conn_id}.");

        let sn_bytes_format_wrap = (Default::default(), Default::default(), bytes);
        conn.send_with(sn_bytes_format_wrap, 0 /* priority */).await.map_err(|error| {
            error!(
                "Error sending out msg... We have {conns_count} open connections to node {conn_id}: {error:?}",
            );
            // clean up failing connections at once, no nead to leak it outside of here
            // next send (e.g. when retrying) will use/create a new connection
            // Timeouts etc should register instantly so we should clean those up fair fast
            let _ = connections.remove(&conn_id);

            debug!("Connection removed from session: {conn_id}");
            // dont close just let the conn timeout incase msgs are coming in...
            // it's removed from our node tracking, so won't be used again for sending.
            NodeLinkError::Send(error)
        })
    }
}

async fn create_connection(
    node: NetworkNode,
    endpoint: &Endpoint,
    connections: NodeConnections,
    msg_id: MsgId,
) -> Result<Arc<Connection>, NodeLinkError> {
    debug!("{msg_id:?} create conn attempt to {node:?}");
    let (conn, _) = endpoint
        .connect_to(&node.addr)
        .await
        .map_err(NodeLinkError::Connection)?;

    trace!(
        "{msg_id:?}: ConnectionOpened to {} (id: {})",
        conn.remote_address(),
        conn.id()
    );

    let conn_id = conn.id();
    debug!("Inserting connection into node link: {conn_id}");

    let conn = Arc::new(conn);
    let _ = connections.insert(conn_id.clone(), conn.clone());
    debug!("Connection INSERTED into node link: {conn_id}");

    Ok(conn)
}

/// Errors that can be returned from `Comm::send_to_one`.
#[derive(Debug, Error)]
pub(crate) enum NodeLinkError {
    #[error("Failed to connect: {0:?}")]
    Connection(qp2p::ConnectionError),
    #[error("Failed to send a message: {0:?}")]
    Send(qp2p::SendError),
    #[error("Failed to receive a message: {0:?}")]
    Recv(qp2p::RecvError),
    #[error("Max number of attempts ({0}) to send msg to the node has been reached")]
    MaxRetriesReached(usize),
}

impl NodeLinkError {
    fn is_local_close(&self) -> bool {
        matches!(
            self,
            NodeLinkError::Connection(qp2p::ConnectionError::Closed(qp2p::Close::Local))
                | NodeLinkError::Send(qp2p::SendError::ConnectionLost(
                    qp2p::ConnectionError::Closed(qp2p::Close::Local)
                ))
        )
    }
}
