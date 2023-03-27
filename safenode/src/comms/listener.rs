use crate::comms::{NetworkMsg, NetworkNode};

// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
use super::{CommEvent, MsgReceived, MsgTrait};

use qp2p::{Connection, ConnectionIncoming, IncomingConnections};
use tokio::{sync::mpsc::Sender, task};
use tracing::{debug, error, trace, warn};

#[tracing::instrument(skip_all)]
pub(crate) fn listen_for_connections<T: MsgTrait + 'static>(
    comm_events_sender: Sender<CommEvent<T>>,
    mut incoming_connections: IncomingConnections,
) {
    let _handle = task::spawn(async move {
        while let Some((connection, incoming_msgs)) = incoming_connections.next().await {
            trace!(
                "IncomingConnection: from {:?} with connection_id {}",
                connection.remote_address(),
                connection.id()
            );

            let _handle = task::spawn(listen_for_msgs(
                comm_events_sender.clone(),
                connection,
                incoming_msgs,
            ));
        }
    });
}

#[tracing::instrument(skip_all)]
pub(crate) async fn listen_for_msgs<T: MsgTrait>(
    comm_events: Sender<CommEvent<T>>,
    conn: Connection,
    mut incoming_msgs: ConnectionIncoming,
) {
    let conn_id = conn.id();
    let remote_address = conn.remote_address();

    while let Some(result) = incoming_msgs.next_with_stream().await.transpose() {
        match result {
            Ok((msg_bytes, send_stream)) => {
                let stream_info = if let Some(stream) = &send_stream {
                    format!(" on {}", stream.id())
                } else {
                    "".to_string()
                };
                debug!(
                    "New msg arrived over conn_id={conn_id} from {remote_address:?}{stream_info}"
                );
                // ignore safe network related stuff, just get the payload
                let payload = msg_bytes.0 .2;
                let wire_msg = match NetworkMsg::from_bytes(payload) {
                    Ok(wire_msg) => wire_msg,
                    Err(error) => {
                        // TODO: should perhaps rather drop this connection.. as it is a spam vector
                        debug!("Failed to deserialize message received from {remote_address:?}{stream_info}: {error:?}");
                        continue;
                    }
                };

                let src = NetworkNode {
                    addr: remote_address,
                };
                let msg_id = wire_msg.id;
                debug!(
                        "Msg {msg_id:?} received, over conn_id={conn_id}, from: {src:?}{stream_info} was: {wire_msg:?}"
                    );

                msg_received(wire_msg, src, send_stream, comm_events.clone()).await;
            }
            Err(error) => {
                trace!("Issue on connection {conn_id} with {remote_address}: {error:?}");
            }
        }
    }

    trace!(%conn_id, %remote_address, "ConnectionClosed");
}

pub(crate) async fn msg_received<T: MsgTrait>(
    wire_msg: NetworkMsg<T>,
    sender: NetworkNode,
    send_stream: Option<qp2p::SendStream>,
    comm_events: Sender<CommEvent<T>>,
) {
    let msg_id = wire_msg.id;
    let msg_event = CommEvent::Msg(MsgReceived {
        sender: sender.addr,
        wire_msg,
        response_stream: send_stream,
    });

    // handle the message first
    if let Err(error) = comm_events.send(msg_event).await {
        error!("Error pushing msg {msg_id:?} onto internal msg handling channel: {error:?}");
    }
}
