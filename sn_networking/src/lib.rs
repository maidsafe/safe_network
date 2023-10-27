// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod bootstrap;
mod circular_vec;
mod cmd;
mod driver;
mod error;
mod event;
#[cfg(feature = "open-metrics")]
mod metrics;
#[cfg(feature = "open-metrics")]
mod metrics_service;
mod quorum;
mod record_store;
mod record_store_api;
mod replication_fetcher;
mod transfers;

pub use self::{
    cmd::SwarmLocalState,
    driver::{NetworkBuilder, SwarmDriver},
    error::Error,
    event::{MsgResponder, NetworkEvent},
    quorum::GetQuorum,
    record_store::NodeRecordStore,
};

use self::{cmd::SwarmCmd, driver::ExpectedHoldersList, error::Result};
use bytes::Bytes;
use futures::future::select_all;
use itertools::Itertools;
use libp2p::{
    identity::Keypair,
    kad::{KBucketKey, Record, RecordKey},
    multiaddr::Protocol,
    Multiaddr, PeerId,
};
use rand::Rng;
use sn_protocol::{
    messages::{Query, QueryResponse, Request, Response},
    storage::{RecordHeader, RecordKind, RecordType},
    NetworkAddress, PrettyPrintKBucketKey, PrettyPrintRecordKey,
};
use sn_transfers::MainPubkey;
use sn_transfers::NanoTokens;
use std::{collections::HashMap, path::PathBuf};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
/// The peer should be present among the CLOSE_GROUP_SIZE if we're fetching the close_group(peer)
/// The size has been set to 5 for improved performance.
pub const CLOSE_GROUP_SIZE: usize = 5;

/// The range of peers that will be considered as close to a record target,
/// that a replication of the record shall be sent/accepted to/by the peer.
pub const REPLICATE_RANGE: usize = CLOSE_GROUP_SIZE * 2;

/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    // Calculate the majority of the close group size by dividing it by 2 and adding 1.
    // This ensures that the majority is always greater than half.
    CLOSE_GROUP_SIZE / 2 + 1
}

/// Max duration to wait for verification
const MAX_REVERIFICATION_WAIT_TIME_S: std::time::Duration = std::time::Duration::from_millis(2000);
/// Min duration to wait for verification
const MIN_REVERIFICATION_WAIT_TIME_S: std::time::Duration = std::time::Duration::from_millis(500);
/// Number of attempts to verify a record
const VERIFICATION_ATTEMPTS: usize = 3;
/// Number of attempts to re-put a record
const PUT_RECORD_RETRIES: usize = 3;

/// Sort the provided peers by their distance to the given `NetworkAddress`.
/// Return with the closest expected number of entries if has.
#[allow(clippy::result_large_err)]
pub fn sort_peers_by_address<'a>(
    peers: &'a [PeerId],
    address: &NetworkAddress,
    expected_entries: usize,
) -> Result<Vec<&'a PeerId>> {
    sort_peers_by_key(peers, &address.as_kbucket_key(), expected_entries)
}

/// Sort the provided peers by their distance to the given `KBucketKey`.
/// Return with the closest expected number of entries if has.
#[allow(clippy::result_large_err)]
pub fn sort_peers_by_key<'a, T>(
    peers: &'a [PeerId],
    key: &KBucketKey<T>,
    expected_entries: usize,
) -> Result<Vec<&'a PeerId>> {
    // Create a vector of tuples where each tuple is a reference to a peer and its distance to the key.
    // This avoids multiple computations of the same distance in the sorting process.
    let mut peer_distances = peers
        .iter()
        .map(|peer_id| {
            let addr = NetworkAddress::from_peer(*peer_id);
            let distance = key.distance(&addr.as_kbucket_key());
            (peer_id, distance)
        })
        .collect_vec();

    // Sort the vector of tuples by the distance.
    peer_distances.sort_by(|a, b| a.1.cmp(&b.1));

    // Collect the sorted peers into a new vector.
    let sorted_peers: Vec<&PeerId> = peer_distances
        .iter()
        .take(expected_entries)
        .map(|&(peer_id, _)| peer_id)
        .collect();

    // Check if there are enough peers to satisfy the request.
    if CLOSE_GROUP_SIZE > sorted_peers.len() {
        warn!("Not enough peers in the k-bucket to satisfy the request");
        return Err(Error::NotEnoughPeers {
            found: sorted_peers.len(),
            required: CLOSE_GROUP_SIZE,
        });
    }
    Ok(sorted_peers)
}

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct Network {
    pub swarm_cmd_sender: mpsc::Sender<SwarmCmd>,
    pub peer_id: PeerId,
    pub root_dir_path: PathBuf,
    keypair: Keypair,
}

impl Network {
    /// Signs the given data with the node's keypair.
    pub fn sign(&self, msg: &[u8]) -> Result<Vec<u8>> {
        self.keypair.sign(msg).map_err(Error::from)
    }

    /// Dial the given peer at the given address.
    pub async fn dial(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::Dial { addr, sender })?;
        receiver.await?
    }

    /// Stop the continuous Kademlia Bootstrapping process
    pub fn stop_bootstrapping(&self) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::StopBootstrapping)
    }

    /// Returns the closest peers to the given `XorName`, sorted by their distance to the xor_name.
    /// Excludes the client's `PeerId` while calculating the closest peers.
    pub async fn client_get_closest_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        self.get_closest_peers(key, true).await
    }

    /// Returns the closest peers to the given `NetworkAddress`, sorted by their distance to the key.
    ///
    /// Includes our node's `PeerId` while calculating the closest peers.
    pub async fn node_get_closest_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        self.get_closest_peers(key, false).await
    }

    /// Returns the closest peers to the given `NetworkAddress` that is fetched from the local
    /// Routing Table. It is ordered by increasing distance of the peers
    /// Note self peer_id is not included in the result.
    pub async fn get_closest_local_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestLocalPeers {
            key: key.clone(),
            sender,
        })?;

        match receiver.await {
            Ok(close_peers) => {
                let close_peers_pretty_print: Vec<_> = close_peers
                    .iter()
                    .map(|peer_id| {
                        format!(
                            "{peer_id:?}({:?})",
                            PrettyPrintKBucketKey(
                                NetworkAddress::from_peer(*peer_id).as_kbucket_key()
                            )
                        )
                    })
                    .collect();

                trace!(
                    "Local knowledge of close peers to {key:?} are: {close_peers_pretty_print:?}"
                );
                Ok(close_peers)
            }
            Err(err) => {
                error!("When getting local knowledge of close peers to {key:?}, failed with error {err:?}");
                Err(Error::InternalMsgChannelDropped)
            }
        }
    }

    /// Returns all the PeerId from all the KBuckets from our local Routing Table
    /// Also contains our own PeerId.
    pub async fn get_all_local_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalPeers { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    pub async fn get_store_costs_from_network(
        &self,
        record_address: NetworkAddress,
    ) -> Result<Vec<(MainPubkey, NanoTokens)>> {
        // The requirement of having at least CLOSE_GROUP_SIZE
        // close nodes will be checked internally automatically.
        let mut close_nodes = self.get_closest_peers(&record_address, true).await?;

        // Sometimes we can get too many close node responses here.
        // (Seemingly libp2p can return more than expected)
        // We only want CLOSE_GROUP_SIZE peers at most
        close_nodes.sort_by(|a, b| {
            let a = NetworkAddress::from_peer(*a);
            let b = NetworkAddress::from_peer(*b);
            record_address
                .distance(&a)
                .cmp(&record_address.distance(&b))
        });

        close_nodes.truncate(CLOSE_GROUP_SIZE);

        let request = Request::Query(Query::GetStoreCost(record_address.clone()));
        let responses = self
            .send_and_get_responses(close_nodes, &request, true)
            .await;

        // loop over responses, generating an average fee and storing all responses along side
        let mut all_costs = vec![];
        for response in responses.into_iter().flatten() {
            debug!(
                "StoreCostReq for {record_address:?} received response: {:?}",
                response
            );
            if let Response::Query(QueryResponse::GetStoreCost {
                store_cost: Ok(cost),
                payment_address,
            }) = response
            {
                let cost_with_tolerance = NanoTokens::from((cost.as_nano() as f32 * 1.1) as u64);
                all_costs.push((payment_address, cost_with_tolerance));
            } else {
                error!("Non store cost response received,  was {:?}", response);
            }
        }

        get_fees_from_store_cost_responses(all_costs)
    }

    /// Subscribe to given gossipsub topic
    pub fn subscribe_to_topic(&self, topic_id: String) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::GossipsubSubscribe(topic_id))?;
        Ok(())
    }

    /// Unsubscribe from given gossipsub topic
    pub fn unsubscribe_from_topic(&self, topic_id: String) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::GossipsubUnsubscribe(topic_id))?;
        Ok(())
    }

    /// Publish a msg on a given topic
    pub fn publish_on_topic(&self, topic_id: String, msg: Bytes) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::GossipsubPublish { topic_id, msg })?;
        Ok(())
    }

    /// Get the Record from the network
    /// Carry out re-attempts if required
    /// In case a target_record is provided, only return when fetched target.
    /// Otherwise count it as a failure when all attempts completed.
    pub async fn get_record_from_network(
        &self,
        key: RecordKey,
        target_record: Option<Record>,
        quorum: GetQuorum,
        re_attempt: bool,
        expected_holders: ExpectedHoldersList,
    ) -> Result<Record> {
        let total_attempts = if re_attempt { VERIFICATION_ATTEMPTS } else { 1 };

        let mut verification_attempts = 0;
        let pretty_key = PrettyPrintRecordKey::from(&key);
        while verification_attempts < total_attempts {
            verification_attempts += 1;
            info!(
                "Getting record of {pretty_key:?} attempts {verification_attempts:?}/{total_attempts:?}",
            );

            let (sender, receiver) = oneshot::channel();
            self.send_swarm_cmd(SwarmCmd::GetNetworkRecord {
                key: key.clone(),
                sender,
                quorum,
                expected_holders: expected_holders.clone(),
            })?;

            match receiver
                .await
                .map_err(|_e| Error::InternalMsgChannelDropped)?
            {
                Ok(returned_record) => {
                    let header = RecordHeader::from_record(&returned_record)?;
                    let is_chunk = matches!(header.kind, RecordKind::Chunk);
                    info!("Record returned: {pretty_key:?}",);

                    // Returning OK whenever fulfill one of the followings:
                    // 1, No targeting record
                    // 2, Fetched record matches the targeting record (when not chunk, as they are content addressed)
                    //
                    // Returning mismatched error when: completed all attempts
                    if target_record.is_none()
                        || (target_record.is_some()
                            // we don't need to match the whole record if chunks, 
                            // payment data could differ, but chunks themselves'
                            // keys are from the chunk address
                            && (target_record == Some(returned_record.clone()) || is_chunk))
                    {
                        return Ok(returned_record);
                    } else if verification_attempts >= total_attempts {
                        info!("Error: Returned record does not match target");
                        return Err(Error::ReturnedRecordDoesNotMatch(
                            PrettyPrintRecordKey::from(&returned_record.key).into_owned(),
                        ));
                    }
                }
                Err(Error::RecordNotEnoughCopies(returned_record)) => {
                    debug!("Not enough copies found yet for {pretty_key:?}");
                    // Only return when completed all attempts
                    if verification_attempts >= total_attempts && matches!(quorum, GetQuorum::One) {
                        if target_record.is_none()
                            || (target_record.is_some()
                                && target_record == Some(returned_record.clone()))
                        {
                            return Ok(returned_record);
                        } else {
                            return Err(Error::ReturnedRecordDoesNotMatch(
                                PrettyPrintRecordKey::from(&returned_record.key).into_owned(),
                            ));
                        }
                    }
                }
                Err(Error::RecordNotFound) => {
                    // libp2p RecordNotFound does mean no holders answered.
                    // it does not actually mean the record does not exist.
                    // just that those asked did not have it
                    if verification_attempts >= total_attempts {
                        break;
                    }

                    warn!("No holder of record '{pretty_key:?}' found. Retrying the fetch ...",);
                }
                Err(Error::SplitRecord { result_map }) => {
                    error!("Getting record {pretty_key:?} attempts #{verification_attempts}/{total_attempts} , encountered split");

                    if verification_attempts >= total_attempts {
                        return Err(Error::SplitRecord { result_map });
                    }
                    warn!("Fetched split Record '{pretty_key:?}' from network!. Retrying...",);
                }
                Err(error) => {
                    error!("Getting record {pretty_key:?} attempts #{verification_attempts}/{total_attempts} , encountered {error:?}");

                    if verification_attempts >= total_attempts {
                        break;
                    }
                    warn!("Did not retrieve Record '{pretty_key:?}' from network!. Retrying...",);
                }
            }

            // wait for a bit before re-trying
            if re_attempt {
                tokio::time::sleep(MAX_REVERIFICATION_WAIT_TIME_S).await;
            }
        }

        Err(Error::RecordNotFound)
    }

    /// Get the cost of storing the next record from the network
    pub async fn get_local_storecost(&self) -> Result<NanoTokens> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalStoreCost { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Get `Record` from the local RecordStore
    pub async fn get_local_record(&self, key: &RecordKey) -> Result<Option<Record>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalRecord {
            key: key.clone(),
            sender,
        })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Put `Record` to network
    /// Optionally verify the record is stored after putting it to network
    /// Retry up to `PUT_RECORD_RETRIES` times if we can't verify the record is stored
    pub async fn put_record(
        &self,
        record: Record,
        verify_store: Option<Record>,
        expected_holders: ExpectedHoldersList,
    ) -> Result<()> {
        let mut retries = 0;

        // TODO: Move this put retry loop up above store cost checks so we can re-put if storecost failed.
        while retries < PUT_RECORD_RETRIES {
            info!(
                "Attempting to PUT record of {:?} to network",
                PrettyPrintRecordKey::from(&record.key)
            );

            let res = self
                .put_record_once(
                    record.clone(),
                    verify_store.clone(),
                    expected_holders.clone(),
                )
                .await;

            // if we're not verifying a record, or it's fine we can return
            if verify_store.is_none() || res.is_ok() {
                return res;
            }

            // otherwise try again
            retries += 1;
        }

        Err(Error::FailedToVerifyRecordWasStored(
            PrettyPrintRecordKey::from(&record.key).into_owned(),
        ))
    }

    async fn put_record_once(
        &self,
        record: Record,
        verify_store: Option<Record>,
        expected_holders: ExpectedHoldersList,
    ) -> Result<()> {
        let record_key = record.key.clone();
        let pretty_key = PrettyPrintRecordKey::from(&record_key);
        info!(
            "Putting record of {} - length {:?} to network",
            pretty_key,
            record.value.len()
        );

        // Waiting for a response to avoid flushing to network too quick that causing choke
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::PutRecord {
            record: record.clone(),
            sender,
        })?;
        let response = receiver.await?;

        if verify_store.is_some() || !expected_holders.is_empty() {
            // Generate a random duration between MAX_REVERIFICATION_WAIT_TIME_S and MIN_REVERIFICATION_WAIT_TIME_S
            let wait_duration = rand::thread_rng()
                .gen_range(MIN_REVERIFICATION_WAIT_TIME_S..MAX_REVERIFICATION_WAIT_TIME_S);
            // Small wait before we attempt to verify.
            // There will be `re-attempts` to be carried out within the later step anyway.
            tokio::time::sleep(wait_duration).await;
            trace!("attempting to verify {pretty_key:?}");

            // Verify the record is stored, requiring re-attempts
            self.get_record_from_network(
                record_key,
                verify_store,
                GetQuorum::All,
                true,
                expected_holders,
            )
            .await?;
        }

        response
    }

    /// Put `Record` to the local RecordStore
    /// Must be called after the validations are performed on the Record
    pub fn put_local_record(&self, record: Record) -> Result<()> {
        trace!(
            "Writing Record locally, for {:?} - length {:?}",
            PrettyPrintRecordKey::from(&record.key),
            record.value.len()
        );
        self.send_swarm_cmd(SwarmCmd::PutLocalRecord { record })
    }

    /// Remove a local record from the RecordStore after a failed write
    pub fn remove_failed_local_record(&self, key: RecordKey) -> Result<()> {
        trace!("Removing Record locally, for {:?}", key);
        self.send_swarm_cmd(SwarmCmd::RemoveFailedLocalRecord { key })
    }

    /// Returns true if a RecordKey is present locally in the RecordStore
    pub async fn is_record_key_present_locally(&self, key: &RecordKey) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::RecordStoreHasKey {
            key: key.clone(),
            sender,
        })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    /// Returns the Addresses of all the locally stored Records
    pub async fn get_all_local_record_addresses(
        &self,
    ) -> Result<HashMap<NetworkAddress, RecordType>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalRecordAddresses { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    // Add a list of keys of a holder to Replication Fetcher.
    pub fn add_keys_to_replication_fetcher(
        &self,
        holder: PeerId,
        keys: Vec<(NetworkAddress, RecordType)>,
    ) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::AddKeysToReplicationFetcher { holder, keys })
    }

    /// Send `Request` to the given `PeerId` and await for the response. If `self` is the recipient,
    /// then the `Request` is forwarded to itself and handled, and a corresponding `Response` is created
    /// and returned to itself. Hence the flow remains the same and there is no branching at the upper
    /// layers.
    pub async fn send_request(&self, req: Request, peer: PeerId) -> Result<Response> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::SendRequest {
            req,
            peer,
            sender: Some(sender),
        })?;
        receiver.await?
    }

    /// Send `Request` to the given `PeerId` and do _not_ await a response here.
    /// Instead the Response will be handled by the common `response_handler`
    pub fn send_req_ignore_reply(&self, req: Request, peer: PeerId) -> Result<()> {
        let swarm_cmd = SwarmCmd::SendRequest {
            req,
            peer,
            sender: None,
        };
        self.send_swarm_cmd(swarm_cmd)
    }

    /// Send a `Response` through the channel opened by the requester.
    pub fn send_response(&self, resp: Response, channel: MsgResponder) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::SendResponse { resp, channel })
    }

    /// Return a `SwarmLocalState` with some information obtained from swarm's local state.
    pub async fn get_swarm_local_state(&self) -> Result<SwarmLocalState> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetSwarmLocalState(sender))?;
        let state = receiver.await?;
        Ok(state)
    }

    // Helper to send SwarmCmd
    fn send_swarm_cmd(&self, cmd: SwarmCmd) -> Result<()> {
        let capacity = self.swarm_cmd_sender.capacity();

        if capacity == 0 {
            error!("SwarmCmd channel is full. Dropping SwarmCmd: {:?}", cmd);

            // Lets error out just now.
            return Err(Error::NoSwarmCmdChannelCapacity);
        }
        let cmd_sender = self.swarm_cmd_sender.clone();

        // Spawn a task to send the SwarmCmd and keep this fn sync
        let _handle = tokio::spawn(async move {
            if let Err(error) = cmd_sender.send(cmd).await {
                error!("Failed to send SwarmCmd: {}", error);
            }
        });

        Ok(())
    }

    /// Returns the closest peers to the given `XorName`, sorted by their distance to the xor_name.
    /// If `client` is false, then include `self` among the `closest_peers`
    pub async fn get_closest_peers(
        &self,
        key: &NetworkAddress,
        client: bool,
    ) -> Result<Vec<PeerId>> {
        trace!("Getting the closest peers to {key:?}");
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestPeers {
            key: key.clone(),
            sender,
        })?;
        let k_bucket_peers = receiver.await?;

        // Count self in if among the CLOSE_GROUP_SIZE closest and sort the result
        let mut closest_peers: Vec<_> = k_bucket_peers.into_iter().collect();
        if !client {
            closest_peers.push(self.peer_id);
        }

        let close_peers_pretty_print: Vec<_> = closest_peers
            .iter()
            .map(|peer_id| {
                format!(
                    "{peer_id:?}({:?})",
                    PrettyPrintKBucketKey(NetworkAddress::from_peer(*peer_id).as_kbucket_key())
                )
            })
            .collect();

        trace!("Network knowledge of close peers to {key:?} are: {close_peers_pretty_print:?}");

        let closest_peers = sort_peers_by_address(&closest_peers, key, CLOSE_GROUP_SIZE)?
            .into_iter()
            .cloned()
            .collect();
        Ok(closest_peers)
    }

    /// Send a `Request` to the provided set of peers and wait for their responses concurrently.
    /// If `get_all_responses` is true, we wait for the responses from all the peers.
    /// NB TODO: Will return an error if the request timeouts.
    /// If `get_all_responses` is false, we return the first successful response that we get
    pub async fn send_and_get_responses(
        &self,
        peers: Vec<PeerId>,
        req: &Request,
        get_all_responses: bool,
    ) -> Vec<Result<Response>> {
        debug!("send_and_get_responses for {req:?}");
        let mut list_of_futures = peers
            .iter()
            .map(|peer| Box::pin(self.send_request(req.clone(), *peer)))
            .collect::<Vec<_>>();

        let mut responses = Vec::new();
        while !list_of_futures.is_empty() {
            let (res, _, remaining_futures) = select_all(list_of_futures).await;
            let res_string = match &res {
                Ok(res) => format!("{res}"),
                Err(err) => format!("{err:?}"),
            };
            debug!("Got response for the req: {req:?}, res: {res_string}");
            if !get_all_responses && res.is_ok() {
                return vec![res];
            }
            responses.push(res);
            list_of_futures = remaining_futures;
        }

        debug!("Received all responses for {req:?}");
        responses
    }
}

/// Given `all_costs` it will return the CLOSE_GROUP majority cost.
#[allow(clippy::result_large_err)]
fn get_fees_from_store_cost_responses(
    mut all_costs: Vec<(MainPubkey, NanoTokens)>,
) -> Result<Vec<(MainPubkey, NanoTokens)>> {
    // TODO: we should make this configurable based upon data type
    // or user requirements for resilience.
    let desired_quote_count = CLOSE_GROUP_SIZE;

    // sort all costs by fee, lowest to highest
    // if there's a tie in cost, sort by pubkey
    all_costs.sort_by(|(pub_key_a, cost_a), (pub_key_b, cost_b)| {
        match cost_a.partial_cmp(cost_b) {
            Some(std::cmp::Ordering::Equal) => pub_key_a.cmp(pub_key_b),
            other => other.unwrap_or(std::cmp::Ordering::Equal),
        }
    });

    // get the first desired_quote_count of all_costs
    all_costs.truncate(desired_quote_count);

    info!(
        "Final fees calculated as: {all_costs:?}, from: {:?}",
        all_costs
    );

    Ok(all_costs)
}

/// Verifies if `Multiaddr` contains IPv4 address that is not global.
/// This is used to filter out unroutable addresses from the Kademlia routing table.
pub fn multiaddr_is_global(multiaddr: &Multiaddr) -> bool {
    !multiaddr.iter().any(|addr| match addr {
        Protocol::Ip4(ip) => {
            // Based on the nightly `is_global` method (`Ipv4Addrs::is_global`), only using what is available in stable.
            // Missing `is_shared`, `is_benchmarking` and `is_reserved`.
            ip.is_unspecified()
                | ip.is_private()
                | ip.is_loopback()
                | ip.is_link_local()
                | ip.is_documentation()
                | ip.is_broadcast()
        }
        _ => false,
    })
}

/// Pop off the `/p2p/<peer_id>`. This mutates the `Multiaddr` and returns the `PeerId` if it exists.
pub(crate) fn multiaddr_pop_p2p(multiaddr: &mut Multiaddr) -> Option<PeerId> {
    if let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() {
        // Only actually strip the last protocol if it's indeed the peer ID.
        let _ = multiaddr.pop();
        Some(peer_id)
    } else {
        None
    }
}

/// Build a `Multiaddr` with the p2p protocol filtered out.
pub(crate) fn multiaddr_strip_p2p(multiaddr: &Multiaddr) -> Multiaddr {
    multiaddr
        .iter()
        .filter(|p| !matches!(p, Protocol::P2p(_)))
        .collect()
}

#[cfg(test)]
mod tests {
    use eyre::bail;

    use super::*;

    #[test]
    fn test_get_fee_from_store_cost_responses() -> Result<()> {
        // for a vec of different costs of CLOSE_GROUP size
        // ensure we return the CLOSE_GROUP / 2 indexed price
        let mut costs = vec![];
        for i in 0..CLOSE_GROUP_SIZE {
            let addr = MainPubkey::new(bls::SecretKey::random().public_key());
            costs.push((addr, NanoTokens::from(i as u64)));
        }
        let prices = get_fees_from_store_cost_responses(costs)?;
        let total_price: u64 = prices
            .iter()
            .fold(0, |acc, (_, price)| acc + price.as_nano());

        // sum all the numbers from 0 to CLOSE_GROUP_SIZE
        let expected_price = CLOSE_GROUP_SIZE * (CLOSE_GROUP_SIZE - 1) / 2;

        assert_eq!(
            total_price, expected_price as u64,
            "price should be {}",
            expected_price
        );

        Ok(())
    }
    #[test]
    #[ignore = "we want to pay the entire CLOSE_GROUP for now"]
    fn test_get_any_fee_from_store_cost_responses_errs_if_insufficient_responses(
    ) -> eyre::Result<()> {
        // for a vec of different costs of CLOSE_GROUP size
        // ensure we return the CLOSE_GROUP / 2 indexed price
        let mut costs = vec![];
        for i in 0..(CLOSE_GROUP_SIZE / 2) - 1 {
            let addr = MainPubkey::new(bls::SecretKey::random().public_key());
            costs.push((addr, NanoTokens::from(i as u64)));
        }

        if get_fees_from_store_cost_responses(costs).is_ok() {
            bail!("Should have errored as we have too few responses")
        }

        Ok(())
    }
    #[test]
    #[ignore = "we want to pay the entire CLOSE_GROUP for now"]
    fn test_get_some_fee_from_store_cost_responses_errs_if_sufficient() -> eyre::Result<()> {
        // for a vec of different costs of CLOSE_GROUP size
        let responses_count = CLOSE_GROUP_SIZE as u64 - 1;
        let mut costs = vec![];
        for i in 0..responses_count {
            // push random MainPubkey and Nano
            let addr = MainPubkey::new(bls::SecretKey::random().public_key());
            costs.push((addr, NanoTokens::from(i)));
            println!("price added {}", i);
        }

        let prices = match get_fees_from_store_cost_responses(costs) {
            Err(_) => bail!("Should not have errored as we have enough responses"),
            Ok(cost) => cost,
        };

        let total_price: u64 = prices
            .iter()
            .fold(0, |acc, (_, price)| acc + price.as_nano());

        // sum all the numbers from 0 to CLOSE_GROUP_SIZE / 2 + 1
        let expected_price = (CLOSE_GROUP_SIZE / 2) * (CLOSE_GROUP_SIZE / 2 + 1) / 2;

        assert_eq!(
            total_price, expected_price as u64,
            "price should be {}",
            total_price
        );

        Ok(())
    }
}
