// Copyright 2024 MaidSafe.net limited.
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
mod get_record_handler;
mod log_markers;
#[cfg(feature = "open-metrics")]
mod metrics;
#[cfg(feature = "open-metrics")]
mod metrics_service;
mod network_discovery;
mod record_store;
mod record_store_api;
mod replication_fetcher;
mod spends;
pub mod target_arch;
mod transfers;
mod transport;

// re-export arch dependent deps for use in the crate, or above
pub use target_arch::{interval, sleep, spawn, Instant, Interval};

pub use self::{
    cmd::{NodeIssue, SwarmLocalState},
    driver::{GetRecordCfg, NetworkBuilder, PutRecordCfg, SwarmDriver, VerificationKind},
    error::{GetRecordError, NetworkError},
    event::{MsgResponder, NetworkEvent},
    record_store::{calculate_cost_for_records, NodeRecordStore},
    transfers::{get_raw_signed_spends_from_record, get_signed_spend_from_record},
};

use self::{cmd::SwarmCmd, error::Result};
use backoff::{Error as BackoffError, ExponentialBackoff};
use futures::future::select_all;
use libp2p::{
    identity::Keypair,
    kad::{KBucketDistance, KBucketKey, Quorum, Record, RecordKey},
    multiaddr::Protocol,
    Multiaddr, PeerId,
};
use rand::Rng;
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{ChunkProof, Cmd, Nonce, Query, QueryResponse, Request, Response},
    storage::{RecordType, RetryStrategy},
    NetworkAddress, PrettyPrintKBucketKey, PrettyPrintRecordKey,
};
use sn_transfers::{MainPubkey, NanoTokens, PaymentQuote, QuotingMetrics};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot,
};

use tokio::time::Duration;
use tracing::trace;

/// The type of quote for a selected payee.
pub type PayeeQuote = (PeerId, MainPubkey, PaymentQuote);

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
/// The peer should be present among the CLOSE_GROUP_SIZE if we're fetching the close_group(peer)
/// The size has been set to 5 for improved performance.
pub const CLOSE_GROUP_SIZE: usize = 5;

/// The count of peers that will be considered as close to a record target,
/// that a replication of the record shall be sent/accepted to/by the peer.
pub const REPLICATION_PEERS_COUNT: usize = CLOSE_GROUP_SIZE + 2;

/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    // Calculate the majority of the close group size by dividing it by 2 and adding 1.
    // This ensures that the majority is always greater than half.
    CLOSE_GROUP_SIZE / 2 + 1
}

/// Max duration to wait for verification.
const MAX_WAIT_BEFORE_READING_A_PUT: Duration = Duration::from_millis(750);
/// Min duration to wait for verification
const MIN_WAIT_BEFORE_READING_A_PUT: Duration = Duration::from_millis(300);

/// Sort the provided peers by their distance to the given `NetworkAddress`.
/// Return with the closest expected number of entries if has.
#[allow(clippy::result_large_err)]
pub fn sort_peers_by_address<'a>(
    peers: &'a Vec<PeerId>,
    address: &NetworkAddress,
    expected_entries: usize,
) -> Result<Vec<&'a PeerId>> {
    sort_peers_by_key(peers, &address.as_kbucket_key(), expected_entries)
}

/// Sort the provided peers by their distance to the given `KBucketKey`.
/// Return with the closest expected number of entries if has.
#[allow(clippy::result_large_err)]
pub fn sort_peers_by_key<'a, T>(
    peers: &'a Vec<PeerId>,
    key: &KBucketKey<T>,
    expected_entries: usize,
) -> Result<Vec<&'a PeerId>> {
    // Check if there are enough peers to satisfy the request.
    // bail early if that's not the case
    if CLOSE_GROUP_SIZE > peers.len() {
        warn!("Not enough peers in the k-bucket to satisfy the request");
        return Err(NetworkError::NotEnoughPeers {
            found: peers.len(),
            required: CLOSE_GROUP_SIZE,
        });
    }

    // Create a vector of tuples where each tuple is a reference to a peer and its distance to the key.
    // This avoids multiple computations of the same distance in the sorting process.
    let mut peer_distances: Vec<(&PeerId, KBucketDistance)> = Vec::with_capacity(peers.len());

    for peer_id in peers {
        let addr = NetworkAddress::from_peer(*peer_id);
        let distance = key.distance(&addr.as_kbucket_key());
        peer_distances.push((peer_id, distance));
    }

    // Sort the vector of tuples by the distance.
    peer_distances.sort_by(|a, b| a.1.cmp(&b.1));

    // Collect the sorted peers into a new vector.
    let sorted_peers: Vec<_> = peer_distances
        .into_iter()
        .take(expected_entries)
        .map(|(peer_id, _)| peer_id)
        .collect();

    Ok(sorted_peers)
}

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct Network {
    pub swarm_cmd_sender: mpsc::Sender<SwarmCmd>,
    pub peer_id: Arc<PeerId>,
    pub root_dir_path: Arc<PathBuf>,
    keypair: Arc<Keypair>,
}

impl Network {
    /// Signs the given data with the node's keypair.
    pub fn sign(&self, msg: &[u8]) -> Result<Vec<u8>> {
        self.keypair.sign(msg).map_err(NetworkError::from)
    }

    /// Verifies a signature for the given data and the node's public key.
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        self.keypair.public().verify(msg, sig)
    }

    /// Returns the protobuf serialised PublicKey to allow messaging out for share.
    pub fn get_pub_key(&self) -> Vec<u8> {
        self.keypair.public().encode_protobuf()
    }

    /// Dial the given peer at the given address.
    /// This function will only be called for the bootstrap nodes.
    pub async fn dial(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::Dial { addr, sender });
        receiver.await?
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

    /// Returns a map where each key is the ilog2 distance of that Kbucket and each value is a vector of peers in that
    /// bucket.
    /// Does not include self
    pub async fn get_kbuckets(&self) -> Result<BTreeMap<u32, Vec<PeerId>>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetKBuckets { sender });
        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Returns the closest peers to the given `NetworkAddress` that is fetched from the local
    /// Routing Table. It is ordered by increasing distance of the peers
    /// Note self peer_id is not included in the result.
    pub async fn get_close_group_local_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetCloseGroupLocalPeers {
            key: key.clone(),
            sender,
        });

        match receiver.await {
            Ok(close_peers) => {
                // Only perform the pretty print and tracing if tracing is enabled
                if tracing::level_enabled!(tracing::Level::TRACE) {
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
                }
                Ok(close_peers)
            }
            Err(err) => {
                error!("When getting local knowledge of close peers to {key:?}, failed with error {err:?}");
                Err(NetworkError::InternalMsgChannelDropped)
            }
        }
    }

    /// Returns all the PeerId from all the KBuckets from our local Routing Table
    /// Also contains our own PeerId.
    pub async fn get_all_local_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalPeers { sender });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Returns all the PeerId from all the KBuckets from our local Routing Table
    /// Also contains our own PeerId.
    pub async fn get_closest_k_value_local_peers(&self) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestKLocalPeers { sender });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Get the Chunk existence proof from the close nodes to the provided chunk address.
    pub async fn verify_chunk_existence(
        &self,
        chunk_address: NetworkAddress,
        nonce: Nonce,
        expected_proof: ChunkProof,
        quorum: Quorum,
        retry_strategy: Option<RetryStrategy>,
    ) -> Result<()> {
        let mut total_attempts = 1;
        total_attempts += retry_strategy
            .map(|strategy| strategy.get_count())
            .unwrap_or(0);

        let pretty_key = PrettyPrintRecordKey::from(&chunk_address.to_record_key()).into_owned();
        let expected_n_verified = get_quorum_value(&quorum);

        let mut close_nodes = Vec::new();
        let mut retry_attempts = 0;
        while retry_attempts < total_attempts {
            // the check should happen before incrementing retry_attempts
            if retry_attempts % 2 == 0 {
                // Do not query the closest_peers during every re-try attempt.
                // The close_nodes don't change often and the previous set of close_nodes might be taking a while to write
                // the Chunk, so query them again incase of a failure.
                close_nodes = self.get_closest_peers(&chunk_address, true).await?;
            }
            retry_attempts += 1;
            info!(
                "Getting ChunkProof for {pretty_key:?}. Attempts: {retry_attempts:?}/{total_attempts:?}",
            );

            let request = Request::Query(Query::GetChunkExistenceProof {
                key: chunk_address.clone(),
                nonce,
            });
            let responses = self
                .send_and_get_responses(&close_nodes, &request, true)
                .await;
            let n_verified = responses
                .into_iter()
                .filter_map(|(peer, resp)| {
                    if let Ok(Response::Query(QueryResponse::GetChunkExistenceProof(Ok(proof)))) =
                        resp
                    {
                        if expected_proof.verify(&proof) {
                            debug!("Got a valid ChunkProof from {peer:?}");
                            Some(())
                        } else {
                            warn!("Failed to verify the ChunkProof from {peer:?}. The chunk might have been tampered?");
                            None
                        }
                    } else {
                        debug!("Did not get a valid response for the ChunkProof from {peer:?}");
                        None
                    }
                })
                .count();
            debug!("Got {n_verified} verified chunk existence proofs for chunk_address {chunk_address:?}");

            if n_verified >= expected_n_verified {
                return Ok(());
            }
            warn!("The obtained {n_verified} verified proofs did not match the expected {expected_n_verified} verified proofs");
            // Sleep to avoid firing queries too close to even choke the nodes further.
            let waiting_time = if retry_attempts == 1 {
                MIN_WAIT_BEFORE_READING_A_PUT
            } else {
                MIN_WAIT_BEFORE_READING_A_PUT + MIN_WAIT_BEFORE_READING_A_PUT
            };
            sleep(waiting_time).await;
        }

        Err(NetworkError::FailedToVerifyChunkProof(
            chunk_address.clone(),
        ))
    }

    /// Get the store costs from the majority of the closest peers to the provided RecordKey.
    /// Record already exists will have a cost of zero to be returned.
    ///
    /// Ignore the quote from any peers from `ignore_peers`. This is useful if we want to repay a different PeerId
    /// on failure.
    pub async fn get_store_costs_from_network(
        &self,
        record_address: NetworkAddress,
        ignore_peers: Vec<PeerId>,
    ) -> Result<PayeeQuote> {
        // The requirement of having at least CLOSE_GROUP_SIZE
        // close nodes will be checked internally automatically.
        let close_nodes = self.get_closest_peers(&record_address, true).await?;

        let request = Request::Query(Query::GetStoreCost(record_address.clone()));
        let responses = self
            .send_and_get_responses(&close_nodes, &request, true)
            .await;

        // loop over responses, generating an average fee and storing all responses along side
        let mut all_costs = vec![];
        let mut all_quotes = vec![];
        for response in responses.into_values().flatten() {
            debug!(
                "StoreCostReq for {record_address:?} received response: {:?}",
                response
            );
            match response {
                Response::Query(QueryResponse::GetStoreCost {
                    quote: Ok(quote),
                    payment_address,
                    peer_address,
                }) => {
                    all_costs.push((peer_address.clone(), payment_address, quote.clone()));
                    all_quotes.push((peer_address, quote));
                }
                Response::Query(QueryResponse::GetStoreCost {
                    quote: Err(ProtocolError::RecordExists(_)),
                    payment_address,
                    peer_address,
                }) => {
                    all_costs.push((peer_address, payment_address, PaymentQuote::zero()));
                }
                _ => {
                    error!("Non store cost response received,  was {:?}", response);
                }
            }
        }

        for peer_id in close_nodes.iter() {
            let request = Request::Cmd(Cmd::QuoteVerification {
                target: NetworkAddress::from_peer(*peer_id),
                quotes: all_quotes.clone(),
            });

            self.send_req_ignore_reply(request, *peer_id);
        }

        // Sort all_costs by the NetworkAddress proximity to record_address
        all_costs.sort_by(|(peer_address_a, _, _), (peer_address_b, _, _)| {
            record_address
                .distance(peer_address_a)
                .cmp(&record_address.distance(peer_address_b))
        });
        #[allow(clippy::mutable_key_type)]
        let ignore_peers = ignore_peers
            .into_iter()
            .map(NetworkAddress::from_peer)
            .collect::<BTreeSet<_>>();

        // Ensure we dont have any further out nodes than `close_group_majority()`
        // This should ensure that if we didnt get all responses from close nodes,
        // we're less likely to be paying a node that is not in the CLOSE_GROUP
        //
        // Also filter out the peers.
        let all_costs = all_costs
            .into_iter()
            .filter(|(peer_address, ..)| !ignore_peers.contains(peer_address))
            .take(close_group_majority())
            .collect();

        get_fees_from_store_cost_responses(all_costs)
    }

    /// Get a record from the network
    /// This differs from non-wasm32 builds as no retries are applied
    #[cfg(target_arch = "wasm32")]
    pub async fn get_record_from_network(
        &self,
        key: RecordKey,
        cfg: &GetRecordCfg,
    ) -> Result<Record> {
        let pretty_key = PrettyPrintRecordKey::from(&key);
        info!("Getting record from network of {pretty_key:?}. with cfg {cfg:?}",);
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetNetworkRecord {
            key: key.clone(),
            sender,
            cfg: cfg.clone(),
        });
        let result = receiver.await.map_err(|e| {
            error!("When fetching record {pretty_key:?}, encountered a channel error {e:?}");
            NetworkError::InternalMsgChannelDropped
        })?;

        result.map_err(NetworkError::from)
    }

    /// Get the Record from the network
    /// Carry out re-attempts if required
    /// In case a target_record is provided, only return when fetched target.
    /// Otherwise count it as a failure when all attempts completed.
    ///
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get_record_from_network(
        &self,
        key: RecordKey,
        cfg: &GetRecordCfg,
    ) -> Result<Record> {
        let retry_duration = cfg.retry_strategy.map(|strategy| strategy.get_duration());
        backoff::future::retry(
            ExponentialBackoff {
                // None sets a random duration, but we'll be terminating with a BackoffError::Permanent, so retry will
                // be disabled.
                max_elapsed_time: retry_duration,
                ..Default::default()
            },
            || async {
                let pretty_key = PrettyPrintRecordKey::from(&key);
                info!("Getting record from network of {pretty_key:?}. with cfg {cfg:?}",);
                let (sender, receiver) = oneshot::channel();
                self.send_swarm_cmd(SwarmCmd::GetNetworkRecord {
                    key: key.clone(),
                    sender,
                    cfg: cfg.clone(),
                });
                let result = receiver.await.map_err(|e| {
                error!("When fetching record {pretty_key:?}, encountered a channel error {e:?}");
                NetworkError::InternalMsgChannelDropped
            }).map_err(|err| BackoffError::Transient { err,  retry_after: None })?;

                // log the results
                match &result {
                    Ok(_) => {
                        info!("Record returned: {pretty_key:?}.");
                    }
                    Err(GetRecordError::RecordDoesNotMatch(_)) => {
                        warn!("The returned record does not match target {pretty_key:?}.");
                    }
                    Err(GetRecordError::NotEnoughCopies { expected, got, .. }) => {
                        warn!("Not enough copies ({got}/{expected}) found yet for {pretty_key:?}.");
                    }
                    // libp2p RecordNotFound does mean no holders answered.
                    // it does not actually mean the record does not exist.
                    // just that those asked did not have it
                    Err(GetRecordError::RecordNotFound) => {
                        warn!("No holder of record '{pretty_key:?}' found.");
                    }
                    Err(GetRecordError::SplitRecord { .. }) => {
                        error!("Encountered a split record for {pretty_key:?}.");
                    }
                    Err(GetRecordError::QueryTimeout) => {
                        error!("Encountered query timeout for {pretty_key:?}.");
                    }
                };

                // if we don't want to retry, throw permanent error
                if cfg.retry_strategy.is_none() {
                    if let Err(e) = result {
                        return Err(BackoffError::Permanent(NetworkError::from(e)));
                    }
                }
                if result.is_err() {
                    trace!("Getting record from network of {pretty_key:?} via backoff...");
                }
                result.map_err(|err| BackoffError::Transient {
                    err: NetworkError::from(err),
                    retry_after: None,
                })
            },
        )
        .await
    }

    /// Get the cost of storing the next record from the network
    pub async fn get_local_storecost(
        &self,
        key: RecordKey,
    ) -> Result<(NanoTokens, QuotingMetrics)> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalStoreCost { key, sender });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Notify the node receicced a payment.
    pub fn notify_payment_received(&self) {
        self.send_swarm_cmd(SwarmCmd::PaymentReceived);
    }

    /// Get `Record` from the local RecordStore
    pub async fn get_local_record(&self, key: &RecordKey) -> Result<Option<Record>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetLocalRecord {
            key: key.clone(),
            sender,
        });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Whether the target peer is considered blacklisted by self
    pub async fn is_peer_shunned(&self, target: NetworkAddress) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::IsPeerShunned { target, sender });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Put `Record` to network
    /// Optionally verify the record is stored after putting it to network
    /// If verify is on, retry multiple times within MAX_PUT_RETRY_DURATION duration.
    pub async fn put_record(&self, record: Record, cfg: &PutRecordCfg) -> Result<()> {
        let pretty_key = PrettyPrintRecordKey::from(&record.key);

        let retry_duration = cfg.retry_strategy.map(|strategy| strategy.get_duration());
        backoff::future::retry(
            ExponentialBackoff {
                // None sets a random duration, but we'll be terminating with a BackoffError::Permanent, so retry will
                // be disabled.
            max_elapsed_time: retry_duration,
            ..Default::default()
        }, || async {

            info!(
                "Attempting to PUT record with key: {pretty_key:?} to network, with cfg {cfg:?}, retrying via backoff..."
            );
            self.put_record_once(record.clone(), cfg).await.map_err(|err|
            {
                warn!("Failed to PUT record with key: {pretty_key:?} to network (retry via backoff) with error: {err:?}");

                if cfg.retry_strategy.is_some() {
                    BackoffError::Transient { err, retry_after: None }
                } else {
                    BackoffError::Permanent(err)
                }

            })
        }).await
    }

    async fn put_record_once(&self, record: Record, cfg: &PutRecordCfg) -> Result<()> {
        let record_key = record.key.clone();
        let pretty_key = PrettyPrintRecordKey::from(&record_key);
        info!(
            "Putting record of {} - length {:?} to network",
            pretty_key,
            record.value.len()
        );

        // Waiting for a response to avoid flushing to network too quick that causing choke
        let (sender, receiver) = oneshot::channel();
        if let Some(put_record_to_peers) = &cfg.use_put_record_to {
            self.send_swarm_cmd(SwarmCmd::PutRecordTo {
                peers: put_record_to_peers.clone(),
                record: record.clone(),
                sender,
                quorum: cfg.put_quorum,
            });
        } else {
            self.send_swarm_cmd(SwarmCmd::PutRecord {
                record: record.clone(),
                sender,
                quorum: cfg.put_quorum,
            });
        }

        let response = receiver.await?;

        if let Some((verification_kind, get_cfg)) = &cfg.verification {
            // Generate a random duration between MAX_WAIT_BEFORE_READING_A_PUT and MIN_WAIT_BEFORE_READING_A_PUT
            let wait_duration = rand::thread_rng()
                .gen_range(MIN_WAIT_BEFORE_READING_A_PUT..MAX_WAIT_BEFORE_READING_A_PUT);
            // Small wait before we attempt to verify.
            // There will be `re-attempts` to be carried out within the later step anyway.
            sleep(wait_duration).await;
            debug!("Attempting to verify {pretty_key:?} after we've slept for {wait_duration:?}");

            // Verify the record is stored, requiring re-attempts
            if let VerificationKind::ChunkProof {
                expected_proof,
                nonce,
            } = verification_kind
            {
                self.verify_chunk_existence(
                    NetworkAddress::from_record_key(&record_key),
                    *nonce,
                    expected_proof.clone(),
                    get_cfg.get_quorum,
                    get_cfg.retry_strategy,
                )
                .await?;
            } else {
                match self
                    .get_record_from_network(record.key.clone(), get_cfg)
                    .await
                {
                    Ok(_) => {
                        debug!("Record {pretty_key:?} verified to be stored.");
                    }
                    Err(NetworkError::GetRecordError(GetRecordError::RecordNotFound)) => {
                        warn!("Record {pretty_key:?} not found after PUT, either rejected or not yet stored by nodes when we asked");
                        return Err(NetworkError::RecordNotStoredByNodes(
                            NetworkAddress::from_record_key(&record_key),
                        ));
                    }
                    Err(e) => {
                        debug!(
                            "Failed to verify record {pretty_key:?} to be stored with error: {e:?}"
                        );
                        return Err(e);
                    }
                }
            }
        }
        response
    }

    /// Put `Record` to the local RecordStore
    /// Must be called after the validations are performed on the Record
    pub fn put_local_record(&self, record: Record) {
        trace!(
            "Writing Record locally, for {:?} - length {:?}",
            PrettyPrintRecordKey::from(&record.key),
            record.value.len()
        );
        self.send_swarm_cmd(SwarmCmd::PutLocalRecord { record })
    }

    /// Returns true if a RecordKey is present locally in the RecordStore
    pub async fn is_record_key_present_locally(&self, key: &RecordKey) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::RecordStoreHasKey {
            key: key.clone(),
            sender,
        });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
    }

    /// Returns the Addresses of all the locally stored Records
    pub async fn get_all_local_record_addresses(
        &self,
    ) -> Result<HashMap<NetworkAddress, RecordType>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalRecordAddresses { sender });

        receiver
            .await
            .map_err(|_e| NetworkError::InternalMsgChannelDropped)
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
        });
        receiver.await?
    }

    /// Send `Request` to the given `PeerId` and do _not_ await a response here.
    /// Instead the Response will be handled by the common `response_handler`
    pub fn send_req_ignore_reply(&self, req: Request, peer: PeerId) {
        let swarm_cmd = SwarmCmd::SendRequest {
            req,
            peer,
            sender: None,
        };
        self.send_swarm_cmd(swarm_cmd)
    }

    /// Send a `Response` through the channel opened by the requester.
    pub fn send_response(&self, resp: Response, channel: MsgResponder) {
        self.send_swarm_cmd(SwarmCmd::SendResponse { resp, channel })
    }

    /// Return a `SwarmLocalState` with some information obtained from swarm's local state.
    pub async fn get_swarm_local_state(&self) -> Result<SwarmLocalState> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetSwarmLocalState(sender));
        let state = receiver.await?;
        Ok(state)
    }

    pub fn trigger_interval_replication(&self) {
        self.send_swarm_cmd(SwarmCmd::TriggerIntervalReplication)
    }

    pub fn record_node_issues(&self, peer_id: PeerId, issue: NodeIssue) {
        self.send_swarm_cmd(SwarmCmd::RecordNodeIssue { peer_id, issue });
    }

    pub fn historical_verify_quotes(&self, quotes: Vec<(PeerId, PaymentQuote)>) {
        self.send_swarm_cmd(SwarmCmd::QuoteVerification { quotes });
    }

    // Helper to send SwarmCmd
    fn send_swarm_cmd(&self, cmd: SwarmCmd) {
        send_swarm_cmd(self.swarm_cmd_sender.clone(), cmd);
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
        self.send_swarm_cmd(SwarmCmd::GetClosestPeersToAddressFromNetwork {
            key: key.clone(),
            sender,
        });
        let k_bucket_peers = receiver.await?;

        // Count self in if among the CLOSE_GROUP_SIZE closest and sort the result
        let mut closest_peers = k_bucket_peers;
        // ensure we're not including self here
        if client {
            // remove our peer id from the calculations here:
            closest_peers.retain(|&x| x != *self.peer_id);
        }
        if tracing::level_enabled!(tracing::Level::TRACE) {
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
        }

        let closest_peers = sort_peers_by_address(&closest_peers, key, CLOSE_GROUP_SIZE)?;
        Ok(closest_peers.into_iter().cloned().collect())
    }

    /// Send a `Request` to the provided set of peers and wait for their responses concurrently.
    /// If `get_all_responses` is true, we wait for the responses from all the peers.
    /// NB TODO: Will return an error if the request timeouts.
    /// If `get_all_responses` is false, we return the first successful response that we get
    pub async fn send_and_get_responses(
        &self,
        peers: &[PeerId],
        req: &Request,
        get_all_responses: bool,
    ) -> BTreeMap<PeerId, Result<Response>> {
        debug!("send_and_get_responses for {req:?}");
        let mut list_of_futures = peers
            .iter()
            .map(|peer| {
                Box::pin(async {
                    let resp = self.send_request(req.clone(), *peer).await;
                    (*peer, resp)
                })
            })
            .collect::<Vec<_>>();

        let mut responses = BTreeMap::new();
        while !list_of_futures.is_empty() {
            let ((peer, resp), _, remaining_futures) = select_all(list_of_futures).await;
            let resp_string = match &resp {
                Ok(resp) => format!("{resp}"),
                Err(err) => format!("{err:?}"),
            };
            debug!("Got response from {peer:?} for the req: {req:?}, resp: {resp_string}");
            if !get_all_responses && resp.is_ok() {
                return BTreeMap::from([(peer, resp)]);
            }
            responses.insert(peer, resp);
            list_of_futures = remaining_futures;
        }

        debug!("Received all responses for {req:?}");
        responses
    }
}

/// Given `all_costs` it will return the closest / lowest cost
/// Closest requiring it to be within CLOSE_GROUP nodes
fn get_fees_from_store_cost_responses(
    mut all_costs: Vec<(NetworkAddress, MainPubkey, PaymentQuote)>,
) -> Result<PayeeQuote> {
    // sort all costs by fee, lowest to highest
    // if there's a tie in cost, sort by pubkey
    all_costs.sort_by(
        |(address_a, _main_key_a, cost_a), (address_b, _main_key_b, cost_b)| match cost_a
            .cost
            .cmp(&cost_b.cost)
        {
            std::cmp::Ordering::Equal => address_a.cmp(address_b),
            other => other,
        },
    );

    // get the lowest cost
    trace!("Got all costs: {all_costs:?}");
    let payee = all_costs
        .into_iter()
        .next()
        .ok_or(NetworkError::NoStoreCostResponses)?;
    info!("Final fees calculated as: {payee:?}");
    // we dont need to have the address outside of here for now
    let payee_id = if let Some(peer_id) = payee.0.as_peer_id() {
        peer_id
    } else {
        error!("Can't get PeerId from payee {:?}", payee.0);
        return Err(NetworkError::NoStoreCostResponses);
    };
    Ok((payee_id, payee.1, payee.2))
}

/// Get the value of the provided Quorum
pub fn get_quorum_value(quorum: &Quorum) -> usize {
    match quorum {
        Quorum::Majority => close_group_majority(),
        Quorum::All => CLOSE_GROUP_SIZE,
        Quorum::N(v) => v.get(),
        Quorum::One => 1,
    }
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

pub(crate) fn send_swarm_cmd(swarm_cmd_sender: Sender<SwarmCmd>, cmd: SwarmCmd) {
    let capacity = swarm_cmd_sender.capacity();

    if capacity == 0 {
        error!(
            "SwarmCmd channel is full. Await capacity to send: {:?}",
            cmd
        );
    }

    // Spawn a task to send the SwarmCmd and keep this fn sync
    let _handle = spawn(async move {
        if let Err(error) = swarm_cmd_sender.send(cmd).await {
            error!("Failed to send SwarmCmd: {}", error);
        }
    });
}

#[cfg(test)]
mod tests {
    use eyre::bail;

    use super::*;
    use sn_transfers::PaymentQuote;

    #[test]
    fn test_get_fee_from_store_cost_responses() -> Result<()> {
        // for a vec of different costs of CLOSE_GROUP size
        // ensure we return the CLOSE_GROUP / 2 indexed price
        let mut costs = vec![];
        for i in 1..CLOSE_GROUP_SIZE {
            let addr = MainPubkey::new(bls::SecretKey::random().public_key());
            costs.push((
                NetworkAddress::from_peer(PeerId::random()),
                addr,
                PaymentQuote::test_dummy(Default::default(), NanoTokens::from(i as u64)),
            ));
        }
        let expected_price = costs[0].2.cost.as_nano();
        let (_peer_id, _key, price) = get_fees_from_store_cost_responses(costs)?;

        assert_eq!(
            price.cost.as_nano(),
            expected_price,
            "price should be {expected_price}"
        );

        Ok(())
    }

    #[test]
    fn test_get_some_fee_from_store_cost_responses_even_if_one_errs_and_sufficient(
    ) -> eyre::Result<()> {
        // for a vec of different costs of CLOSE_GROUP size
        let responses_count = CLOSE_GROUP_SIZE as u64 - 1;
        let mut costs = vec![];
        for i in 1..responses_count {
            // push random MainPubkey and Nano
            let addr = MainPubkey::new(bls::SecretKey::random().public_key());
            costs.push((
                NetworkAddress::from_peer(PeerId::random()),
                addr,
                PaymentQuote::test_dummy(Default::default(), NanoTokens::from(i)),
            ));
            println!("price added {i}");
        }

        // this should be the lowest price
        let expected_price = costs[0].2.cost.as_nano();

        let (_peer_id, _key, price) = match get_fees_from_store_cost_responses(costs) {
            Err(_) => bail!("Should not have errored as we have enough responses"),
            Ok(cost) => cost,
        };

        assert_eq!(
            price.cost.as_nano(),
            expected_price,
            "price should be {expected_price}"
        );

        Ok(())
    }

    #[test]
    fn test_network_sign_verify() -> eyre::Result<()> {
        let (network, _, _) =
            NetworkBuilder::new(Keypair::generate_ed25519(), false, std::env::temp_dir())
                .build_client()?;
        let msg = b"test message";
        let sig = network.sign(msg)?;
        assert!(network.verify(msg, &sig));
        Ok(())
    }
}
