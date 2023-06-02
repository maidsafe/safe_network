// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod codec;

pub(crate) use codec::{MsgCodec, MsgProtocol};

use crate::{error::Error, MsgResponder, NetworkEvent, SwarmDriver};

use libp2p::request_response::{self, Message};
use sn_protocol::messages::{Request, Response};
use tracing::{trace, warn};

impl SwarmDriver {
    /// Forwards `Request` to the upper layers using `Sender<NetworkEvent>`. Sends `Response` to the peers
    pub async fn handle_msg(
        &mut self,
        event: request_response::Event<Request, Response>,
    ) -> Result<(), Error> {
        match event {
            request_response::Event::Message { message, .. } => match message {
                Message::Request {
                    request,
                    channel,
                    request_id,
                    ..
                } => {
                    trace!("Received request with id: {request_id:?}, req: {request:?}");
                    if let Err(error) = self
                        .event_sender
                        .send(NetworkEvent::RequestReceived {
                            req: request,
                            channel: MsgResponder::FromPeer(channel),
                        })
                        .await
                    {
                        error!("Failed in notify {request_id:?} NetworkEvent: {error:?}");
                    }
                }
                Message::Response {
                    request_id,
                    response,
                } => {
                    trace!("Got response for id: {request_id:?}, res: {response}.");
                    if let Some(sender) = self.pending_requests.remove(&request_id) {
                        sender
                            .send(Ok(response))
                            .map_err(|_| Error::InternalMsgChannelDropped)?;
                    } else {
                        self.handle_response(response)?;
                    }
                }
            },
            request_response::Event::OutboundFailure {
                request_id,
                error,
                peer,
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    sender
                        .send(Err(error.into()))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                } else {
                    warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                    return Err(Error::ReceivedResponseDropped(request_id));
                }
            }
            request_response::Event::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                warn!("RequestResponse: InboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
            }
            request_response::Event::ResponseSent { peer, request_id } => {
                trace!("ResponseSent for request_id: {request_id:?} and peer: {peer:?}");
            }
        }
        Ok(())
    }
}
