// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    sort_peers_by_address, MsgResponder, NetworkError, NetworkEvent, SwarmDriver, CLOSE_GROUP_SIZE,
    REPLICATION_PEERS_COUNT,
};
use itertools::Itertools;
use libp2p::{
    request_response::{self, Message},
    PeerId,
};
use rand::{rngs::OsRng, Rng};
use sn_protocol::{
    messages::{CmdResponse, Request, Response},
    storage::RecordType,
    NetworkAddress,
};

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
                    trace!("Received request {request_id:?} from peer {peer:?}, req: {request:?}");
                    // If the request is replication or quote verification,
                    // we can handle it and send the OK response here.
                    // As the handle result is unimportant to the sender.
                    match request {
                        Request::Cmd(sn_protocol::messages::Cmd::Replicate { holder, keys }) => {
                            let response = Response::Cmd(
                                sn_protocol::messages::CmdResponse::Replicate(Ok(())),
                            );
                            self.swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, response)
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?;

                            self.add_keys_to_replication_fetcher(holder, keys);
                        }
                        Request::Cmd(sn_protocol::messages::Cmd::QuoteVerification {
                            quotes,
                            ..
                        }) => {
                            let response = Response::Cmd(
                                sn_protocol::messages::CmdResponse::QuoteVerification(Ok(())),
                            );
                            self.swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, response)
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?;

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
                            self.swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, response)
                                .map_err(|_| NetworkError::InternalMsgChannelDropped)?;

                            if bad_peer == NetworkAddress::from_peer(self.self_peer_id) {
                                warn!("Peer {detected_by:?} consider us as BAD, due to {bad_behaviour:?}.");
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
                    trace!("Got response {request_id:?} from peer {peer:?}, res: {response}.");
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
                trace!("ResponseSent for request_id: {request_id:?} and peer: {peer:?}");
            }
        }
        Ok(())
    }

    fn add_keys_to_replication_fetcher(
        &mut self,
        sender: NetworkAddress,
        incoming_keys: Vec<(NetworkAddress, RecordType)>,
    ) {
        let holder = if let Some(peer_id) = sender.as_peer_id() {
            peer_id
        } else {
            warn!("Replication list sender is not a peer_id {sender:?}");
            return;
        };

        trace!(
            "Received replication list from {holder:?} of {} keys",
            incoming_keys.len()
        );

        // accept replication requests from the K_VALUE peers away,
        // giving us some margin for replication
        let closest_k_peers = self.get_closest_k_value_local_peers();
        if !closest_k_peers.contains(&holder) || holder == self.self_peer_id {
            trace!("Holder {holder:?} is self or not in replication range.");
            return;
        }

        // On receive a replication_list from a close_group peer, we undertake two tasks:
        //   1, For those keys that we don't have:
        //        fetch them if close enough to us
        //   2, For those keys that we have and supposed to be held by the sender as well:
        //        start chunk_proof check against a randomly selected chunk type record to the sender

        // For fetching, only handle those non-exist and in close range keys
        let keys_to_store =
            self.select_non_existent_records_for_replications(&incoming_keys, &closest_k_peers);

        if keys_to_store.is_empty() {
            debug!("Empty keys to store after adding to");
        } else {
            #[allow(clippy::mutable_key_type)]
            let all_keys = self
                .swarm
                .behaviour_mut()
                .kademlia
                .store_mut()
                .record_addresses_ref();
            let keys_to_fetch = self
                .replication_fetcher
                .add_keys(holder, keys_to_store, all_keys);
            if keys_to_fetch.is_empty() {
                trace!("no waiting keys to fetch from the network");
            } else {
                self.send_event(NetworkEvent::KeysToFetchForReplication(keys_to_fetch));
            }
        }

        // Only trigger chunk_proof check when received a periodical replication request.
        if incoming_keys.len() > 1 {
            let keys_to_verify = self.select_verification_data_candidates(sender);

            if keys_to_verify.is_empty() {
                debug!("No valid candidate to be checked against peer {holder:?}");
            } else {
                self.send_event(NetworkEvent::ChunkProofVerification {
                    peer_id: holder,
                    keys_to_verify,
                });
            }
        }
    }

    /// Checks suggested records against what we hold, so we only
    /// enqueue what we do not have
    fn select_non_existent_records_for_replications(
        &mut self,
        incoming_keys: &[(NetworkAddress, RecordType)],
        closest_k_peers: &Vec<PeerId>,
    ) -> Vec<(NetworkAddress, RecordType)> {
        #[allow(clippy::mutable_key_type)]
        let locally_stored_keys = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .record_addresses_ref();
        let non_existent_keys: Vec<_> = incoming_keys
            .iter()
            .filter(|(addr, record_type)| {
                let key = addr.to_record_key();
                let local = locally_stored_keys.get(&key);

                // if we have a local value of matching record_type, we don't need to fetch it
                if let Some((_, local_record_type)) = local {
                    let not_same_type = local_record_type != record_type;
                    if not_same_type {
                        // Shall only happens for Register
                        info!("Record {addr:?} has different type: local {local_record_type:?}, incoming {record_type:?}");
                    }
                    not_same_type
                } else {
                    true
                }
            })
            .collect();

        non_existent_keys
            .into_iter()
            .filter_map(|(key, record_type)| {
                if Self::is_in_close_range(&self.self_peer_id, key, closest_k_peers) {
                    Some((key.clone(), record_type.clone()))
                } else {
                    // Reduce the log level as there will always be around 40% records being
                    // out of the close range, as the sender side is using `CLOSE_GROUP_SIZE + 2`
                    // to send our replication list to provide addressing margin.
                    // Given there will normally be 6 nodes sending such list with interval of 5-10s,
                    // this will accumulate to a lot of logs with the increasing records uploaded.
                    trace!("not in close range for key {key:?}");
                    None
                }
            })
            .collect()
    }

    /// A close target doesn't falls into the close peers range:
    /// For example, a node b11111X has an RT: [(1, b1111), (2, b111), (5, b11), (9, b1), (7, b0)]
    /// Then for a target bearing b011111 as prefix, all nodes in (7, b0) are its close_group peers.
    /// Then the node b11111X. But b11111X's close_group peers [(1, b1111), (2, b111), (5, b11)]
    /// are none among target b011111's close range.
    /// Hence, the ilog2 calculation based on close_range cannot cover such case.
    /// And have to sort all nodes to figure out whether self is among the close_group to the target.
    fn is_in_close_range(
        our_peer_id: &PeerId,
        target: &NetworkAddress,
        all_peers: &Vec<PeerId>,
    ) -> bool {
        if all_peers.len() <= REPLICATION_PEERS_COUNT {
            return true;
        }

        // Margin of 2 to allow our RT being bit lagging.
        match sort_peers_by_address(all_peers, target, REPLICATION_PEERS_COUNT) {
            Ok(close_group) => close_group.contains(&our_peer_id),
            Err(err) => {
                warn!("Could not get sorted peers for {target:?} with error {err:?}");
                true
            }
        }
    }

    /// Check among all chunk type records that we have, select those close to the peer,
    /// and randomly pick one as the verification candidate.
    #[allow(clippy::mutable_key_type)]
    fn select_verification_data_candidates(&mut self, peer: NetworkAddress) -> Vec<NetworkAddress> {
        let mut closest_peers = self
            .swarm
            .behaviour_mut()
            .kademlia
            .get_closest_local_peers(&self.self_peer_id.into())
            .map(|peer| peer.into_preimage())
            .take(20)
            .collect_vec();
        closest_peers.push(self.self_peer_id);

        let target_peer = if let Some(peer_id) = peer.as_peer_id() {
            peer_id
        } else {
            error!("Target {peer:?} is not a valid PeerId");
            return vec![];
        };

        #[allow(clippy::mutable_key_type)]
        let all_keys = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .record_addresses_ref();

        // Targeted chunk type record shall be expected within the close range from our perspective.
        let mut verify_candidates: Vec<NetworkAddress> = all_keys
            .values()
            .filter_map(|(addr, record_type)| {
                if RecordType::Chunk == *record_type {
                    match sort_peers_by_address(&closest_peers, addr, CLOSE_GROUP_SIZE) {
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

        // To ensure the candidate mush have to be held by the peer,
        // we only carry out check when there are already certain amount of chunks uploaded
        // AND choose candidate from certain reduced range.
        if verify_candidates.len() > 50 {
            let index: usize = OsRng.gen_range(0..(verify_candidates.len() / 2));
            vec![verify_candidates[index].clone()]
        } else {
            vec![]
        }
    }
}
