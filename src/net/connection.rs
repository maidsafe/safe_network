//! A message-oriented API wrapping the underlying QUIC library (`quinn`).

use super::{
    error::{ConnectionError, RecvError, SendError, StreamError},
    wire_msg::{WireMsg, UsrMsgBytes},
};
use quinn::VarInt;
use std::{fmt, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{trace, warn};

// Error reason for closing a connection when triggered manually by qp2p apis
const QP2P_CLOSED_CONNECTION: &str = "The connection was closed intentionally by qp2p.";

type IncomingMsg = Result<(WireMsg, Option<SendStream>), RecvError>;

/// The sending API for a connection.
pub struct Connection {
    pub(crate) inner: quinn::Connection,
}
impl Drop for Connection {
    fn drop(&mut self) {
        warn!(
            "Connection handle dropped, thus closing it, conn_id={}",
            self.id()
        );
        self.inner.close(VarInt::from_u32(0), b"lost interest");
    }
}

impl Connection {
    pub(crate) fn new(connection: quinn::Connection) -> Connection {
        Self { inner: connection }
    }

    // /// Returns `Some(...)` if the connection is closed.
    // pub fn close_reason(&self) -> Option<ConnectionError> {
    //     self.inner.close_reason().map(|e| e.into())
    // }

    /// A stable identifier for the connection.
    ///
    /// This ID will not change for the lifetime of the connection to a given ip.
    ///
    /// The ID pulls the internal conneciton id and concats with the SocketAddr of
    /// the peer. So this _should_ be unique per peer (without IP spoofing).
    ///
    pub fn id(&self) -> String {
        build_conn_id(&self.inner)
    }

    /// The address of the remote peer.
    pub fn remote_address(&self) -> SocketAddr {
        self.inner.remote_address()
    }

    pub async fn accept_bi(&self) -> Result<(quinn::SendStream, quinn::RecvStream), ConnectionError> {
        self.inner.accept_bi().await.map_err(ConnectionError::from)
    }

    pub async fn accept_uni(&self) -> Result<quinn::RecvStream, ConnectionError> {
        self.inner.accept_uni().await.map_err(ConnectionError::from)
    }

    /// Send a message to the peer.
    ///
    /// The message will be sent on a unidirectional QUIC stream, meaning the application is
    /// responsible for correlating any anticipated responses from incoming streams.
    ///
    /// The priority will be `0`.
    pub async fn send(&self, user_msg_bytes: UsrMsgBytes) -> Result<(), SendError> {
        self.send_with(user_msg_bytes, 0).await
    }

    /// Open a unidirection stream to the peer.
    ///
    /// Messages sent over the stream will arrive at the peer in the order they were sent.
    pub async fn open_uni(&self) -> Result<SendStream, ConnectionError> {
        let send_stream = self.inner.open_uni().await?;
        Ok(SendStream::new(send_stream, self.id()))
    }

    /// Open a bidirectional stream to the peer.
    ///
    /// Bidirectional streams allow messages to be sent in both directions. This can be useful to
    /// automatically correlate response messages, for example.
    ///
    /// Messages sent over the stream will arrive at the peer in the order they were sent.
    pub async fn open_bi(&self) -> Result<(SendStream, RecvStream), ConnectionError> {
        let (send_stream, recv_stream) = self.inner.open_bi().await?;
        let conn_id = self.id();
        Ok((
            SendStream::new(send_stream, conn_id.clone()),
            RecvStream::new(recv_stream, conn_id),
        ))
    }

    /// Close the connection immediately.
    ///
    /// This is not a graceful close - pending operations will fail immediately with
    /// [`ConnectionError::Closed`]`(`[`Close::Local`]`)`, and data on unfinished streams is not
    /// guaranteed to be delivered.
    pub fn close(&self, reason: Option<String>) {
        let reason = reason.unwrap_or_else(|| QP2P_CLOSED_CONNECTION.to_string());
        warn!("Closing connection witn conn_id={}", self.id());
        self.inner.close(0u8.into(), &reason.into_bytes());
    }

    /// Opens a uni-directional stream and sends message on it using the given priority.
    pub async fn send_with(
        &self,
        user_msg_bytes: UsrMsgBytes,
        priority: i32,
    ) -> Result<(), SendError> {
        let mut send_stream = self.open_uni().await.map_err(SendError::ConnectionLost)?;
        send_stream.set_priority(priority);

        send_stream.send_user_msg(user_msg_bytes).await?;

        // We try to make sure the stream is gracefully closed and the bytes get sent, but if it
        // was already closed (perhaps by the peer) then we ignore the error.
        // TODO: we probably shouldn't ignore the error...
        send_stream.finish().await.or_else(|err| match err {
            SendError::StreamLost(StreamError::Stopped(_)) => Ok(()),
            _ => Err(err),
        })?;

        Ok(())
    }
}

// Helper to build a connection identifier string
fn build_conn_id(conn: &quinn::Connection) -> String {
    format!("{}{}", conn.remote_address(), conn.stable_id())
}

fn listen_on_uni_streams(connection: quinn::Connection, tx: Sender<IncomingMsg>) {
    let conn_id = build_conn_id(&connection);

    let _handle = tokio::spawn(async move {
        trace!("Connection {conn_id}: listening for incoming uni-streams");

        loop {
            // Wait for an incoming stream.
            let uni = connection.accept_uni().await.map_err(ConnectionError::from);
            let recv = match uni {
                Ok(recv) => recv,
                Err(err) => {
                    // In case of a connection error, there is not much we can do.
                    trace!(
                        "Connection {conn_id}: failure when awaiting incoming uni-streams: {err:?}"
                    );
                    // WARNING: This might block!
                    let _ = tx.send(Err(RecvError::ConnectionLost(err))).await;
                    break;
                }
            };
            trace!("Connection {conn_id}: incoming uni-stream accepted");

            let tx = tx.clone();

            // Make sure we are able to process multiple streams in parallel.
            let _handle = tokio::spawn(async move {
                let reserved_sender = match tx.reserve().await {
                    Ok(p) => p,
                    Err(error) => {
                        tracing::error!(
                            "Could not reserve sender for new conn msg read: {error:?}"
                        );
                        return;
                    }
                };

                let msg = WireMsg::read_from_stream(recv).await;

                // Send away the msg or error
                reserved_sender.send(msg.map(|r| (r, None)));
            });
        }

        trace!("Connection {conn_id}: stopped listening for uni-streams");
    });
}

#[allow(clippy::type_complexity)]
fn listen_on_bi_streams(connection: quinn::Connection, tx: Sender<IncomingMsg>) {
    let conn_id = build_conn_id(&connection);

    let _handle = tokio::spawn(async move {
        trace!("Connection {conn_id}: listening for incoming bi-streams");

        loop {
            // Wait for an incoming stream.
            let bi = connection.accept_bi().await.map_err(ConnectionError::from);
            let (send, recv) = match bi {
                Ok(recv) => recv,
                Err(err) => {
                    // In case of a connection error, there is not much we can do.
                    trace!(
                        "Connection {conn_id}: failure when awaiting incoming bi-streams: {err:?}"
                    );
                    // WARNING: This might block!
                    let _ = tx.send(Err(RecvError::ConnectionLost(err))).await;
                    break;
                }
            };
            trace!("Connection {conn_id}: incoming bi-stream accepted");

            let tx = tx.clone();
            let conn_id = conn_id.clone();

            // Make sure we are able to process multiple streams in parallel.
            let _handle = tokio::spawn(async move {
                let reserved_sender = match tx.reserve().await {
                    Ok(p) => p,
                    Err(error) => {
                        tracing::error!(
                            "Could not reserve sender for new conn msg read: {error:?}"
                        );
                        return;
                    }
                };
                let msg = WireMsg::read_from_stream(recv).await;

                // Pass the stream, so it can be used to respond to the user message.
                let msg = msg.map(|msg| (msg, Some(SendStream::new(send, conn_id.clone()))));
                // Send away the msg or error
                reserved_sender.send(msg);
                trace!("Incoming new msg on conn_id={conn_id} sent to user in upper layer");
            });
        }

        trace!("Connection {conn_id}: stopped listening for bi-streams");
    });
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("id", &self.id())
            .field("remote_address", &self.remote_address())
            .finish_non_exhaustive()
    }
}

/// Identifier for a stream within a particular connection
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StreamId {
    stream_id: quinn::StreamId,
    conn_id: String,
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let initiator = if self.stream_id.initiator().is_client() {
            "initiator"
        } else {
            "acceptor"
        };
        let dir = self.stream_id.dir();
        write!(
            f,
            "{initiator} {dir:?}directional stream {}@{}",
            self.stream_id.index(),
            self.conn_id
        )
    }
}

///
#[derive(Debug)]
pub struct ConnectionIncoming(Receiver<IncomingMsg>);
impl ConnectionIncoming {
    /// Get the next message sent by the peer, over any stream.
    pub async fn next(&mut self) -> Result<Option<WireMsg>, RecvError> {
        if let Some((bytes, _opt)) = self.next_with_stream().await? {
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    /// Get the next message sent by the peer, over any stream along with the stream to respond with.
    pub async fn next_with_stream(
        &mut self,
    ) -> Result<Option<(WireMsg, Option<SendStream>)>, RecvError> {
        self.0.recv().await.transpose()
    }
}

/// The sending API for a QUIC stream.
pub struct SendStream {
    conn_id: String,
    inner: quinn::SendStream,
}

impl SendStream {
    fn new(inner: quinn::SendStream, conn_id: String) -> Self {
        Self { conn_id, inner }
    }

    /// Get the identity of this stream
    pub fn id(&self) -> StreamId {
        StreamId {
            stream_id: self.inner.id(),
            conn_id: self.conn_id.clone(),
        }
    }

    /// Set the priority of the send stream.
    ///
    /// Every send stream has an initial priority of 0. Locally buffered data from streams with
    /// higher priority will be transmitted before data from streams with lower priority. Changing
    /// the priority of a stream with pending data may only take effect after that data has been
    /// transmitted. Using many different priority levels per connection may have a negative impact
    /// on performance.
    pub fn set_priority(&self, priority: i32) {
        // quinn returns `UnknownStream` error if the stream does not exist. We ignore it, on the
        // basis that operations on the stream will fail instead (and the effect of setting priority
        // or not is only observable if the stream exists).
        let _ = self.inner.set_priority(priority);
    }

    /// Send a message over the stream to the peer.
    ///
    /// Messages sent over the stream will arrive at the peer in the order they were sent.
    pub async fn send_user_msg(&mut self, user_msg_bytes: UsrMsgBytes) -> Result<(), SendError> {
        WireMsg(user_msg_bytes)
            .write_to_stream(&mut self.inner)
            .await
    }

    /// Shut down the send stream gracefully.
    ///
    /// The returned future will complete once the peer has acknowledged all sent data.
    pub async fn finish(&mut self) -> Result<(), SendError> {
        self.inner.finish().await?;
        Ok(())
    }
}

impl fmt::Debug for SendStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SendStream").finish_non_exhaustive()
    }
}

/// The receiving API for a bidirectional QUIC stream.
pub struct RecvStream {
    conn_id: String,
    inner: quinn::RecvStream,
}

impl RecvStream {
    fn new(inner: quinn::RecvStream, conn_id: String) -> Self {
        Self { conn_id, inner }
    }

    /// Get the identity of this stream
    pub fn id(&self) -> StreamId {
        StreamId {
            stream_id: self.inner.id(),
            conn_id: self.conn_id.clone(),
        }
    }

    /// Parse the message sent by the peer over this stream.
    pub async fn read(self) -> Result<UsrMsgBytes, RecvError> {
        self.read_wire_msg().await.map(|v| v.0)
    }

    pub(crate) async fn read_wire_msg(self) -> Result<WireMsg, RecvError> {
        WireMsg::read_from_stream(self.inner).await
    }
}
