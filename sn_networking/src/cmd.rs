// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    driver::{PendingGetClosestType, SwarmDriver},
    error::{Error, Result},
    multiaddr_pop_p2p, sort_peers_by_address, GetRecordCfg, GetRecordError, MsgResponder,
    NetworkEvent, CLOSE_GROUP_SIZE, REPLICATE_RANGE,
};
use bytes::Bytes;
use libp2p::{
    kad::{store::RecordStore, Quorum, Record, RecordKey},
    swarm::dial_opts::DialOpts,
    Multiaddr, PeerId,
};
use sn_protocol::{
    messages::{Request, Response},
    storage::{RecordHeader, RecordKind, RecordType},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::NanoTokens;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};
use tokio::sync::oneshot;
use xor_name::XorName;

/// Commands to send to the Swarm
#[allow(clippy::large_enum_variant)]
pub enum SwarmCmd {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Dial {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    DialWithOpts {
        opts: DialOpts,
        sender: oneshot::Sender<Result<()>>,
    },
    // Returns all the peers from all the k-buckets from the local Routing Table.
    // This includes our PeerId as well.
    GetAllLocalPeers {
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    /// Get a map where each key is the ilog2 distance of that Kbucket and each value is a vector of peers in that
    /// bucket.
    GetKBuckets {
        sender: oneshot::Sender<BTreeMap<u32, Vec<PeerId>>>,
    },
    // Returns up to K_VALUE peers from all the k-buckets from the local Routing Table.
    // And our PeerId as well.
    GetClosestKLocalPeers {
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    // Get closest peers from the network
    GetClosestPeersToAddressFromNetwork {
        key: NetworkAddress,
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    // Get closest peers from the local RoutingTable
    GetCloseGroupLocalPeers {
        key: NetworkAddress,
        sender: oneshot::Sender<Vec<PeerId>>,
    },
    GetSwarmLocalState(oneshot::Sender<SwarmLocalState>),
    // Send Request to the PeerId.
    SendRequest {
        req: Request,
        peer: PeerId,

        // If a `sender` is provided, the requesting node will await for a `Response` from the
        // Peer. The result is then returned at the call site.
        //
        // If a `sender` is not provided, the requesting node will not wait for the Peer's
        // response. Instead we trigger a `NetworkEvent::ResponseReceived` which calls the common
        // `response_handler`
        sender: Option<oneshot::Sender<Result<Response>>>,
    },
    SendResponse {
        resp: Response,
        channel: MsgResponder,
    },
    /// Check if the local RecordStore contains the provided key
    RecordStoreHasKey {
        key: RecordKey,
        sender: oneshot::Sender<bool>,
    },
    /// Get the Addresses of all the Records held locally
    GetAllLocalRecordAddresses {
        sender: oneshot::Sender<HashMap<NetworkAddress, RecordType>>,
    },
    /// Get Record from the Kad network
    GetNetworkRecord {
        key: RecordKey,
        sender: oneshot::Sender<std::result::Result<Record, GetRecordError>>,
        cfg: GetRecordCfg,
    },
    /// GetLocalStoreCost for this node
    GetLocalStoreCost {
        sender: oneshot::Sender<NanoTokens>,
    },
    /// Get data from the local RecordStore
    GetLocalRecord {
        key: RecordKey,
        sender: oneshot::Sender<Option<Record>>,
    },
    /// Put record to network
    PutRecord {
        record: Record,
        sender: oneshot::Sender<Result<()>>,
        quorum: Quorum,
    },
    /// Put record to the local RecordStore
    PutLocalRecord {
        record: Record,
    },
    /// Remove a local record from the RecordStore
    /// Typically because the write failed
    RemoveFailedLocalRecord {
        key: RecordKey,
    },
    /// The keys added to the replication fetcher are later used to fetch the Record from network
    AddKeysToReplicationFetcher {
        holder: PeerId,
        keys: HashMap<NetworkAddress, RecordType>,
    },
    /// Subscribe to a given Gossipsub topic
    GossipsubSubscribe(String),
    /// Unsubscribe from a given Gossipsub topic
    GossipsubUnsubscribe(String),
    /// Publish a message through Gossipsub protocol
    GossipsubPublish {
        /// Topic to publish on
        topic_id: String,
        /// Raw bytes of the message to publish
        msg: Bytes,
    },
    GossipHandler,
}

/// Debug impl for SwarmCmd to avoid printing full Record, instead only RecodKey
/// and RecordKind are printed.
impl Debug for SwarmCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwarmCmd::StartListening { addr, .. } => {
                write!(f, "SwarmCmd::StartListening {{ addr: {addr:?} }}")
            }
            SwarmCmd::Dial { addr, .. } => {
                write!(f, "SwarmCmd::Dial {{ addr: {addr:?} }}")
            }
            SwarmCmd::GetNetworkRecord { key, cfg, .. } => {
                write!(
                    f,
                    "SwarmCmd::GetNetworkRecord {{ key: {:?}, cfg: {cfg:?}",
                    PrettyPrintRecordKey::from(key)
                )
            }
            SwarmCmd::PutRecord { record, .. } => {
                write!(
                    f,
                    "SwarmCmd::PutRecord {{ key: {:?} }}",
                    PrettyPrintRecordKey::from(&record.key)
                )
            }
            SwarmCmd::PutLocalRecord { record } => {
                write!(
                    f,
                    "SwarmCmd::PutLocalRecord {{ key: {:?} }}",
                    PrettyPrintRecordKey::from(&record.key)
                )
            }
            SwarmCmd::RemoveFailedLocalRecord { key } => {
                write!(
                    f,
                    "SwarmCmd::RemoveFailedLocalRecord {{ key: {:?} }}",
                    PrettyPrintRecordKey::from(key)
                )
            }
            SwarmCmd::AddKeysToReplicationFetcher { holder, keys } => {
                write!(
                    f,
                    "SwarmCmd::AddKeysToReplicationFetcher {{ holder: {holder:?}, keys_len: {:?} }}",
                    keys.len()
                )
            }
            SwarmCmd::GossipsubSubscribe(topic) => {
                write!(f, "SwarmCmd::GossipsubSubscribe({topic:?})")
            }
            SwarmCmd::GossipsubUnsubscribe(topic) => {
                write!(f, "SwarmCmd::GossipsubUnsubscribe({topic:?})")
            }
            SwarmCmd::GossipsubPublish { topic_id, msg } => {
                write!(
                    f,
                    "SwarmCmd::GossipsubPublish {{ topic_id: {topic_id:?}, msg len: {:?} }}",
                    msg.len()
                )
            }
            SwarmCmd::DialWithOpts { opts, .. } => {
                write!(f, "SwarmCmd::DialWithOpts {{ opts: {opts:?} }}")
            }
            SwarmCmd::GetClosestPeersToAddressFromNetwork { key, .. } => {
                write!(f, "SwarmCmd::GetClosestPeers {{ key: {key:?} }}")
            }
            SwarmCmd::GetClosestKLocalPeers { .. } => {
                write!(f, "SwarmCmd::GetClosestKLocalPeers")
            }
            SwarmCmd::GetCloseGroupLocalPeers { key, .. } => {
                write!(f, "SwarmCmd::GetCloseGroupLocalPeers {{ key: {key:?} }}")
            }
            SwarmCmd::GetLocalStoreCost { .. } => {
                write!(f, "SwarmCmd::GetLocalStoreCost")
            }
            SwarmCmd::GetLocalRecord { key, .. } => {
                write!(
                    f,
                    "SwarmCmd::GetLocalRecord {{ key: {:?} }}",
                    PrettyPrintRecordKey::from(key)
                )
            }
            SwarmCmd::GetAllLocalRecordAddresses { .. } => {
                write!(f, "SwarmCmd::GetAllLocalRecordAddresses")
            }
            SwarmCmd::GetAllLocalPeers { .. } => {
                write!(f, "SwarmCmd::GetAllLocalPeers")
            }
            SwarmCmd::GetKBuckets { .. } => {
                write!(f, "SwarmCmd::GetKBuckets")
            }
            SwarmCmd::GetSwarmLocalState { .. } => {
                write!(f, "SwarmCmd::GetSwarmLocalState")
            }
            SwarmCmd::RecordStoreHasKey { key, .. } => {
                write!(
                    f,
                    "SwarmCmd::RecordStoreHasKey {:?}",
                    PrettyPrintRecordKey::from(key)
                )
            }
            SwarmCmd::SendResponse { resp, .. } => {
                write!(f, "SwarmCmd::SendResponse resp: {resp:?}")
            }
            SwarmCmd::SendRequest { req, peer, .. } => {
                write!(f, "SwarmCmd::SendRequest req: {req:?}, peer: {peer:?}")
            }
            SwarmCmd::GossipHandler => {
                write!(f, "SwarmCmd::GossipHandler")
            }
        }
    }
}
/// Snapshot of information kept in the Swarm's local state
#[derive(Debug, Clone)]
pub struct SwarmLocalState {
    /// List of currently connected peers
    pub connected_peers: Vec<PeerId>,
    /// List of addresses the node is currently listening on
    pub listeners: Vec<Multiaddr>,
}

impl SwarmDriver {
    /// Checks suggested records against what we hold, so we only
    /// enqueue what we do not have
    #[allow(clippy::mutable_key_type)] // for Bytes in NetworkAddress
    fn select_non_existent_records_for_replications(
        &mut self,
        incoming_keys: &HashMap<NetworkAddress, RecordType>,
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
                    &local_record_type != record_type
                } else {
                    true
                }
            })
            .collect();

        trace!("Replicating non_existent_keys, about to check what is close: {non_existent_keys:?}");
        let closest_k_peers = self.get_closest_k_value_local_peers();

        let close_keys = non_existent_keys
            .into_iter()
            .filter_map(|(key, record_type)| {
                if self.is_in_close_range(key, &closest_k_peers) {
                    Some((key.clone(), record_type.clone()))
                } else {
                    warn!("not in close range for key {key:?}");
                    None
                }
            })
            .collect();

        trace!("Final close keys for replication: {close_keys:?}");

        close_keys
    }

    pub(crate) fn handle_cmd(&mut self, cmd: SwarmCmd) -> Result<(), Error> {
        match cmd {
            SwarmCmd::AddKeysToReplicationFetcher { holder, keys } => {
                // Only handle those non-exist and in close range keys
                let keys_to_store = self.select_non_existent_records_for_replications(&keys);
                if keys_to_store.is_empty() {
                    debug!("Empty keys to store after adding to");
                    return Ok(());
                }

                #[allow(clippy::mutable_key_type)]
                let all_keys = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .record_addresses_ref();
                let keys_to_fetch =
                    self.replication_fetcher
                        .add_keys(holder, keys_to_store, all_keys);
                if !keys_to_fetch.is_empty() {
                    self.send_event(NetworkEvent::KeysToFetchForReplication(keys_to_fetch));
                } else {
                    trace!("no waiting keys to fetch from the network");
                }
            }
            SwarmCmd::GetNetworkRecord { key, sender, cfg } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_record(key.clone());

                debug!(
                    "Record {:?} with task {query_id:?} expected to be held by {:?}",
                    PrettyPrintRecordKey::from(&key),
                    cfg.expected_holders
                );

                if self
                    .pending_get_record
                    .insert(query_id, (sender, Default::default(), cfg))
                    .is_some()
                {
                    warn!("An existing get_record task {query_id:?} got replaced");
                }
                // Logging the status of the `pending_get_record`.
                // We also interested in the status of `result_map` (which contains record) inside.
                let total_records: usize = self
                    .pending_get_record
                    .iter()
                    .map(|(_, (_, result_map, _))| result_map.len())
                    .sum();
                trace!("We now have {} pending get record attempts and cached {total_records} fetched copies",
                      self.pending_get_record.len());
            }
            SwarmCmd::GetLocalStoreCost { sender } => {
                let cost = self.swarm.behaviour_mut().kademlia.store_mut().store_cost();

                let _res = sender.send(cost);
            }
            SwarmCmd::GetLocalRecord { key, sender } => {
                let record = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .get(&key)
                    .map(|rec| rec.into_owned());
                let _ = sender.send(record);
            }
            SwarmCmd::PutRecord {
                record,
                sender,
                quorum,
            } => {
                let record_key = PrettyPrintRecordKey::from(&record.key).into_owned();
                trace!(
                    "Putting record sized: {:?} to network {:?}",
                    record.value.len(),
                    record_key
                );
                let res = match self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, quorum)
                {
                    Ok(request_id) => {
                        trace!("Sent record {record_key:?} to network. Request id: {request_id:?} to network");
                        Ok(())
                    }
                    Err(error) => {
                        error!("Error sending record {record_key:?} to network");
                        Err(Error::from(error))
                    }
                };

                if let Err(err) = sender.send(res) {
                    error!("Could not send response to PutRecord cmd: {:?}", err);
                }
            }
            SwarmCmd::PutLocalRecord { record } => {
                let key = record.key.clone();
                let record_key = PrettyPrintRecordKey::from(&key);

                let record_type = match RecordHeader::from_record(&record) {
                    Ok(record_header) => {
                        match record_header.kind {
                            RecordKind::Chunk => RecordType::Chunk,
                            RecordKind::Spend | RecordKind::Register => {
                                let content_hash = XorName::from_content(&record.value);
                                RecordType::NonChunk(content_hash)
                            }
                            RecordKind::ChunkWithPayment | RecordKind::RegisterWithPayment => {
                                error!("Record {record_key:?} with payment shall not be stored locally.");
                                return Err(Error::InCorrectRecordHeader);
                            }
                        }
                    }
                    Err(err) => {
                        error!("For record {record_key:?}, failed to parse record_header {err:?}");
                        return Err(Error::InCorrectRecordHeader);
                    }
                };

                match self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .put_verified(record, record_type.clone())
                {
                    Ok(_) => {
                        let new_keys_to_fetch = self
                            .replication_fetcher
                            .notify_about_new_put(key, record_type);
                        if !new_keys_to_fetch.is_empty() {
                            self.send_event(NetworkEvent::KeysToFetchForReplication(
                                new_keys_to_fetch,
                            ));
                        }
                    }
                    Err(err) => return Err(err.into()),
                };
            }
            SwarmCmd::RemoveFailedLocalRecord { key } => {
                self.swarm.behaviour_mut().kademlia.store_mut().remove(&key)
            }
            SwarmCmd::RecordStoreHasKey { key, sender } => {
                let has_key = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .contains(&key);
                let _ = sender.send(has_key);
            }
            SwarmCmd::GetAllLocalRecordAddresses { sender } => {
                #[allow(clippy::mutable_key_type)] // for the Bytes in NetworkAddress
                let addresses = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .record_addresses();
                let _ = sender.send(addresses);
            }

            SwarmCmd::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::Dial { addr, sender } => {
                let mut addr_copy = addr.clone();
                if let Some(peer_id) = multiaddr_pop_p2p(&mut addr_copy) {
                    // Only consider the dial peer is bootstrap node when proper PeerId is provided.
                    if let Some(kbucket) = self.swarm.behaviour_mut().kademlia.kbucket(peer_id) {
                        let ilog2 = kbucket.range().0.ilog2();
                        let peers = self.bootstrap_peers.entry(ilog2).or_default();
                        peers.insert(peer_id);
                    }
                }
                let _ = match self.dial(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::DialWithOpts { opts, sender } => {
                let _ = match self.dial_with_opts(opts) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(e.into())),
                };
            }
            SwarmCmd::GetClosestPeersToAddressFromNetwork { key, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_peers(key.as_bytes());
                let _ = self.pending_get_closest_peers.insert(
                    query_id,
                    (
                        PendingGetClosestType::FunctionCall(sender),
                        Default::default(),
                    ),
                );
            }
            SwarmCmd::GetAllLocalPeers { sender } => {
                let _ = sender.send(self.get_all_local_peers());
            }
            SwarmCmd::GetKBuckets { sender } => {
                let mut ilog2_kbuckets = BTreeMap::new();
                for kbucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
                    let range = kbucket.range();
                    if let Some(distance) = range.0.ilog2() {
                        let peers_in_kbucket = kbucket
                            .iter()
                            .map(|peer_entry| peer_entry.node.key.clone().into_preimage())
                            .collect::<Vec<PeerId>>();
                        let _ = ilog2_kbuckets.insert(distance, peers_in_kbucket);
                    } else {
                        // This shall never happen.
                        error!("bucket is ourself ???!!!");
                    }
                }
                let _ = sender.send(ilog2_kbuckets);
            }
            SwarmCmd::GetCloseGroupLocalPeers { key, sender } => {
                let key = key.as_kbucket_key();
                // calls `kbuckets.closest_keys(key)` internally, which orders the peers by
                // increasing distance
                // Note it will return all peers, heance a chop down is required.
                let closest_peers = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_local_peers(&key)
                    .map(|peer| peer.into_preimage())
                    .take(CLOSE_GROUP_SIZE)
                    .collect();

                let _ = sender.send(closest_peers);
            }
            SwarmCmd::GetClosestKLocalPeers { sender } => {
                let _ = sender.send(self.get_closest_k_value_local_peers());
            }
            SwarmCmd::SendRequest { req, peer, sender } => {
                // If `self` is the recipient, forward the request directly to our upper layer to
                // be handled.
                // `self` then handles the request and sends a response back again to itself.
                if peer == *self.swarm.local_peer_id() {
                    trace!("Sending query request to self");
                    if let Request::Query(query) = req {
                        self.send_event(NetworkEvent::QueryRequestReceived {
                            query,
                            channel: MsgResponder::FromSelf(sender),
                        });
                    } else {
                        // We should never receive a Replicate request from ourselves.
                        // we already hold this data if we do... so we can ignore
                        trace!("Replicate cmd to self received, ignoring");
                    }
                } else {
                    let request_id = self
                        .swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&peer, req);
                    trace!("Sending request {request_id:?} to peer {peer:?}");
                    let _ = self.pending_requests.insert(request_id, sender);

                    trace!("Pending Requests now: {:?}", self.pending_requests.len());
                }
            }
            SwarmCmd::SendResponse { resp, channel } => match channel {
                // If the response is for `self`, send it directly through the oneshot channel.
                MsgResponder::FromSelf(channel) => {
                    trace!("Sending response to self");
                    match channel {
                        Some(channel) => {
                            channel
                                .send(Ok(resp))
                                .map_err(|_| Error::InternalMsgChannelDropped)?;
                        }
                        None => {
                            // responses that are not awaited at the call site must be handled
                            // separately
                            self.send_event(NetworkEvent::ResponseReceived { res: resp });
                        }
                    }
                }
                MsgResponder::FromPeer(channel) => {
                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, resp)
                        .map_err(Error::OutgoingResponseDropped)?;
                }
            },
            SwarmCmd::GetSwarmLocalState(sender) => {
                let current_state = SwarmLocalState {
                    connected_peers: self.swarm.connected_peers().cloned().collect(),
                    listeners: self.swarm.listeners().cloned().collect(),
                };

                sender
                    .send(current_state)
                    .map_err(|_| Error::InternalMsgChannelDropped)?;
            }
            SwarmCmd::GossipsubSubscribe(topic_id) => {
                let topic_id = libp2p::gossipsub::IdentTopic::new(topic_id);
                if let Some(gossip) = self.swarm.behaviour_mut().gossipsub.as_mut() {
                    gossip.subscribe(&topic_id)?;
                }
            }
            SwarmCmd::GossipsubUnsubscribe(topic_id) => {
                let topic_id = libp2p::gossipsub::IdentTopic::new(topic_id);

                if let Some(gossip) = self.swarm.behaviour_mut().gossipsub.as_mut() {
                    gossip.unsubscribe(&topic_id)?;
                }
            }
            SwarmCmd::GossipsubPublish { topic_id, msg } => {
                // If we publish a Gossipsub message, we might not receive the same message on our side.
                // Hence push an event to notify that we've published a message
                if self.is_gossip_handler {
                    self.send_event(NetworkEvent::GossipsubMsgPublished {
                        topic: topic_id.clone(),
                        msg: msg.clone(),
                    });
                }
                let topic_id = libp2p::gossipsub::IdentTopic::new(topic_id);
                if let Some(gossip) = self.swarm.behaviour_mut().gossipsub.as_mut() {
                    gossip.publish(topic_id, msg)?;
                }
            }
            SwarmCmd::GossipHandler => {
                self.is_gossip_handler = true;
            }
        }

        Ok(())
    }

    // A close target doesn't falls into the close peers range:
    // For example, a node b11111X has an RT: [(1, b1111), (2, b111), (5, b11), (9, b1), (7, b0)]
    // Then for a target bearing b011111 as prefix, all nodes in (7, b0) are its close_group peers.
    // Then the node b11111X. But b11111X's close_group peers [(1, b1111), (2, b111), (5, b11)]
    // are none among target b011111's close range.
    // Hence, the ilog2 calculation based on close_range cannot cover such case.
    // And have to sort all nodes to figure out whether self is among the close_group to the target.
    fn is_in_close_range(&self, target: &NetworkAddress, all_peers: &Vec<PeerId>) -> bool {
        if all_peers.len() <= REPLICATE_RANGE {
            return true;
        }

        // Margin of 2 to allow our RT being bit lagging.
        match sort_peers_by_address(all_peers, target, REPLICATE_RANGE) {
            Ok(close_group) => close_group.contains(&&self.self_peer_id),
            Err(err) => {
                warn!("Could not get sorted peers for {target:?} with error {err:?}");
                true
            }
        }
    }
}
