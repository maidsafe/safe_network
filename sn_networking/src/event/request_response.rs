// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    cmd::NetworkSwarmCmd, log_markers::Marker, sort_peers_by_address_and_limit, MsgResponder,
    NetworkError, NetworkEvent, SwarmDriver, CLOSE_GROUP_SIZE,
};
use libp2p::{
    kad::RecordKey,
    request_response::{self, Message},
    PeerId,
};
use rand::{rngs::OsRng, thread_rng, Rng};
use sn_protocol::{
    messages::{CmdResponse, Request, Response},
    storage::RecordType,
    NetworkAddress,
};
use std::collections::HashMap;

impl SwarmDriver {
    /// Forwards `Request` to the upper layers using `Sender<NetworkEvent>`. Sends `Response` to the peers
    pub(super) fn handle_req_resp_events(
        &mut self,
        event: request_response::Event<Request, Response>,
    ) -> Result<(), NetworkError> {
        match event {
            request_response::Event::Message { message, peer } => match message {
                Message::Request {
                    request,
                    channel,
                    request_id,
                    ..
                } => {
                    debug!("Received request {request_id:?} from peer {peer:?}, req: {request:?}");
                    // If the request is replication or quote verification,
                    // we can handle it and send the OK response here.
                    // As the handle result is unimportant to the sender.
                    match request {
                        Request::Cmd(sn_protocol::messages::Cmd::Replicate { holder, keys }) => {
                            let response = Response::Cmd(
                                sn_protocol::messages::CmdResponse::Replicate(Ok(())),
                            );

                            self.queue_network_swarm_cmd(NetworkSwarmCmd::SendResponse {
                                resp: response,
                                channel: MsgResponder::FromPeer(channel),
                            });

                            self.add_keys_to_replication_fetcher(holder, keys);
                        }
                        Request::Cmd(sn_protocol::messages::Cmd::QuoteVerification {
                            quotes,
                            ..
                        }) => {
                            let response = Response::Cmd(
                                sn_protocol::messages::CmdResponse::QuoteVerification(Ok(())),
                            );
                            self.queue_network_swarm_cmd(NetworkSwarmCmd::SendResponse {
                                resp: response,
                                channel: MsgResponder::FromPeer(channel),
                            });

                            // The keypair is required to verify the quotes,
                            // hence throw it up to Network layer for further actions.
                            let quotes = quotes
                                .iter()
                                .filter_map(|(peer_address, quote)| {
                                    peer_address
                                        .as_peer_id()
                                        .map(|peer_id| (peer_id, quote.clone()))
                                })
                                .collect();
                            self.send_event(NetworkEvent::QuoteVerification { quotes })
                        }
                        Request::Cmd(sn_protocol::messages::Cmd::PeerConsideredAsBad {
                            detected_by,
                            bad_peer,
                            bad_behaviour,
                        }) => {
                            let response = Response::Cmd(
                                sn_protocol::messages::CmdResponse::PeerConsideredAsBad(Ok(())),
                            );

                            self.queue_network_swarm_cmd(NetworkSwarmCmd::SendResponse {
                                resp: response,
                                channel: MsgResponder::FromPeer(channel),
                            });

                            let (Some(detected_by), Some(bad_peer)) =
                                (detected_by.as_peer_id(), bad_peer.as_peer_id())
                            else {
                                error!("Could not get PeerId from detected_by or bad_peer NetworkAddress {detected_by:?}, {bad_peer:?}");
                                return Ok(());
                            };

                            if bad_peer == self.self_peer_id {
                                warn!("Peer {detected_by:?} consider us as BAD, due to {bad_behaviour:?}.");
                                self.record_metrics(Marker::FlaggedAsBadNode {
                                    flagged_by: &detected_by,
                                });

                                // TODO: shall we terminate self after received such notifications
                                //       from the majority close_group nodes around us?
                            } else {
                                error!("Received a bad_peer notification from {detected_by:?}, targeting {bad_peer:?}, which is not us.");
                            }
                        }
                        Request::Query(query) => {
                            self.send_event(NetworkEvent::QueryRequestReceived {
                                query,
                                channel: MsgResponder::FromPeer(channel),
                            })
                        }
                    }
                }
                Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("Got response {request_id:?} from peer {peer:?}, res: {response}.");
                    if let Some(sender) = self.pending_requests.remove(&request_id) {
                        // The sender will be provided if the caller (Requester) is awaiting for a response
                        // at the call site.
                        // Else the Request was just sent to the peer and the Response was
                        // meant to be handled in another way and is not awaited.
                        match sender {
                            Some(sender) => sender
                                .send(Ok(response))
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?,
                            None => {
                                if let Response::Cmd(CmdResponse::Replicate(Ok(()))) = response {
                                    // Nothing to do, response was fine
                                    // This only exists to ensure we dont drop the handle and
                                    // exit early, potentially logging false connection woes
                                } else {
                                    // responses that are not awaited at the call site must be handled
                                    // separately
                                    self.send_event(NetworkEvent::ResponseReceived {
                                        res: response,
                                    });
                                }
                            }
                        }
                    } else {
                        warn!("Tried to remove a RequestId from pending_requests which was not inserted in the first place.
                            Use Cmd::SendRequest with sender:None if you want the Response to be fed into the common handle_response function");
                    }
                }
            },
            request_response::Event::OutboundFailure {
                request_id,
                error,
                peer,
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    match sender {
                        Some(sender) => {
                            sender
                                .send(Err(error.into()))
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?;
                        }
                        None => {
                            warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                            return Err(NetworkError::ReceivedResponseDropped(request_id));
                        }
                    }
                } else {
                    warn!("RequestResponse: OutboundFailure for request_id: {request_id:?} and peer: {peer:?}, with error: {error:?}");
                    return Err(NetworkError::ReceivedResponseDropped(request_id));
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
                debug!("ResponseSent for request_id: {request_id:?} and peer: {peer:?}");
            }
        }
        Ok(())
    }

    fn add_keys_to_replication_fetcher(
        &mut self,
        sender: NetworkAddress,
        incoming_keys: Vec<(NetworkAddress, RecordType)>,
    ) {
        let peers = self.get_all_local_peers_excluding_self();
        let our_peer_id = self.self_peer_id;

        let holder = if let Some(peer_id) = sender.as_peer_id() {
            peer_id
        } else {
            warn!("Replication list sender is not a peer_id {sender:?}");
            return;
        };

        debug!(
            "Received replication list from {holder:?} of {} keys",
            incoming_keys.len()
        );

        // accept replication requests from all peers known peers within our GetRange
        if !peers.contains(&holder) || holder == our_peer_id {
            trace!("Holder {holder:?} is self or not in replication range.");
            return;
        }

        // On receive a replication_list from a close_group peer, we undertake two tasks:
        //   1, For those keys that we don't have:
        //        fetch them if close enough to us
        //   2, For those keys that we have and supposed to be held by the sender as well:
        //        start chunk_proof check against a randomly selected chunk type record to the sender
        //   3, For those spends that we have that differ in the hash, we fetch the other version
        //         and update our local copy.
        let all_keys = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .record_addresses_ref()
            .clone();

        let more_than_one_key = incoming_keys.len() > 1;
        let keys_to_fetch =
            self.replication_fetcher
                .add_keys(holder, incoming_keys, &all_keys, &peers);

        if keys_to_fetch.is_empty() {
            debug!("no waiting keys to fetch from the network");
        } else {
            self.send_event(NetworkEvent::KeysToFetchForReplication(keys_to_fetch));
        }

        // Only trigger chunk_proof check based every X% of the time
        let mut rng = thread_rng();
        // 5% probability
        if more_than_one_key && rng.gen_bool(0.05) {
            let event_sender = self.event_sender.clone();
            let peers_clone = peers.clone();
            let all_keys_clone = all_keys.clone();
            let sender_clone = sender.clone();
            let _handle = tokio::spawn(async move {
                let keys_to_verify = Self::select_verification_data_candidates(
                    &peers_clone,
                    &all_keys_clone,
                    &sender_clone,
                );

                if keys_to_verify.is_empty() {
                    debug!("No valid candidate to be checked against peer {holder:?}");
                } else if let Err(error) = event_sender
                    .send(NetworkEvent::ChunkProofVerification {
                        peer_id: holder,
                        keys_to_verify,
                    })
                    .await
                {
                    error!("SwarmDriver failed to send event: {}", error);
                }
            });

            // In additon to verify the sender, we also verify a random close node.
            // This is to avoid malicious node escaping the check by never send a replication_list.
            // With further reduced probability to 1% (5% * 20%)
            if rng.gen_bool(0.2) {
                let event_sender = self.event_sender.clone();
                let close_group_peers = sort_peers_by_address_and_limit(
                    &peers,
                    &NetworkAddress::from_peer(our_peer_id),
                    CLOSE_GROUP_SIZE,
                )
                .unwrap_or_default();

                loop {
                    let index: usize = OsRng.gen_range(0..close_group_peers.len());
                    let candidate_peer_id = *close_group_peers[index];
                    let candidate = NetworkAddress::from_peer(*close_group_peers[index]);
                    if sender != candidate {
                        let _handle = tokio::spawn(async move {
                            let keys_to_verify = Self::select_verification_data_candidates(
                                &peers, &all_keys, &sender,
                            );

                            if keys_to_verify.is_empty() {
                                debug!("No valid candidate to be checked against peer {candidate_peer_id:?}");
                            } else if let Err(error) = event_sender
                                .send(NetworkEvent::ChunkProofVerification {
                                    peer_id: candidate_peer_id,
                                    keys_to_verify,
                                })
                                .await
                            {
                                error!("SwarmDriver failed to send event: {}", error);
                            }
                        });

                        break;
                    }
                }
            }
        }
    }

    /// Check among all chunk type records that we have, select those close to the peer,
    /// and randomly pick one as the verification candidate.
    fn select_verification_data_candidates(
        all_peers: &Vec<PeerId>,
        all_keys: &HashMap<RecordKey, (NetworkAddress, RecordType)>,
        peer: &NetworkAddress,
    ) -> Vec<NetworkAddress> {
        let target_peer = if let Some(peer_id) = peer.as_peer_id() {
            peer_id
        } else {
            error!("Target {peer:?} is not a valid PeerId");
            return vec![];
        };

        // Targeted chunk type record shall be expected within the close range from our perspective.
        let mut verify_candidates: Vec<NetworkAddress> = all_keys
            .values()
            .filter_map(|(addr, record_type)| {
                if RecordType::Chunk == *record_type {
                    // Here we take the actual closest, as this is where we want to be
                    // strict about who does have the data...
                    match sort_peers_by_address_and_limit(all_peers, addr, CLOSE_GROUP_SIZE) {
                        Ok(close_group) => {
                            if close_group.contains(&&target_peer) {
                                Some(addr.clone())
                            } else {
                                None
                            }
                        }
                        Err(err) => {
                            warn!("Could not get sorted peers for {addr:?} with error {err:?}");
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect();

        verify_candidates.sort_by_key(|a| peer.distance(a));

        verify_candidates
    }
}
