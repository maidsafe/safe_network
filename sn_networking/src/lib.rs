// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod circular_vec;
mod cmd;
mod driver;
mod error;
mod event;
mod record_store;
mod record_store_api;
mod replication_fetcher;

pub use self::{
    cmd::SwarmLocalState,
    driver::SwarmDriver,
    error::Error,
    event::{MsgResponder, NetworkEvent},
};

use self::{cmd::SwarmCmd, error::Result};
use futures::future::select_all;
use itertools::Itertools;
use libp2p::{
    identity::Keypair,
    kad::{KBucketKey, Record, RecordKey},
    multiaddr::Protocol,
    Multiaddr, PeerId,
};
use sn_dbc::PublicAddress;
use sn_dbc::Token;
use sn_protocol::{
    messages::{Query, QueryResponse, Request, Response},
    storage::{RecordHeader, RecordKind},
    NetworkAddress, PrettyPrintRecordKey,
};
use std::{collections::HashSet, path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};
use tracing::warn;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
/// The peer should be present among the CLOSE_GROUP_SIZE if we're fetching the close_group(peer)
pub const CLOSE_GROUP_SIZE: usize = 8;

/// Majority of a given group (i.e. > 1/2).
#[inline]
pub const fn close_group_majority() -> usize {
    CLOSE_GROUP_SIZE / 2 + 1
}

/// Duration to wait for verification
const REVERIFICATION_WAIT_TIME_S: std::time::Duration = std::time::Duration::from_secs(3);
/// Number of attempts to verify a record
const VERIFICATION_ATTEMPTS: usize = 3;
/// Number of attempts to re-put a record
const PUT_RECORD_RETRIES: usize = 3;

/// Sort the provided peers by their distance to the given `NetworkAddress`.
/// Return with the closest expected number of entries if has.
#[allow(clippy::result_large_err)]
pub fn sort_peers_by_address(
    peers: Vec<PeerId>,
    address: &NetworkAddress,
    expected_entries: usize,
) -> Result<Vec<PeerId>> {
    sort_peers_by_key(peers, &address.as_kbucket_key(), expected_entries)
}

/// Sort the provided peers by their distance to the given `KBucketKey`.
/// Return with the closest expected number of entries if has.
#[allow(clippy::result_large_err)]
pub fn sort_peers_by_key<T>(
    mut peers: Vec<PeerId>,
    key: &KBucketKey<T>,
    expected_entries: usize,
) -> Result<Vec<PeerId>> {
    peers.sort_by(|a, b| {
        let a = NetworkAddress::from_peer(*a);
        let b = NetworkAddress::from_peer(*b);
        key.distance(&a.as_kbucket_key())
            .cmp(&key.distance(&b.as_kbucket_key()))
    });
    let peers: Vec<PeerId> = peers.iter().take(expected_entries).cloned().collect();

    if CLOSE_GROUP_SIZE > peers.len() {
        warn!("Not enough peers in the k-bucket to satisfy the request");
        return Err(Error::NotEnoughPeers {
            found: peers.len(),
            required: CLOSE_GROUP_SIZE,
        });
    }
    Ok(peers)
}

#[derive(Clone)]
/// API to interact with the underlying Swarm
pub struct Network {
    pub swarm_cmd_sender: mpsc::Sender<SwarmCmd>,
    pub peer_id: PeerId,
    pub root_dir_path: PathBuf,
    keypair: Keypair,
    /// Optinal Concurrent limiter to limit the number of concurrent requests
    /// Intended for client side use
    concurrency_limiter: Option<Arc<Semaphore>>,
}

impl Network {
    /// Signs the given data with the node's keypair.
    #[allow(clippy::result_large_err)]
    pub fn sign(&self, msg: &[u8]) -> Result<Vec<u8>> {
        self.keypair.sign(msg).map_err(Error::from)
    }

    /// Get the network's concurrency limiter
    pub fn concurrency_limiter(&self) -> Option<Arc<Semaphore>> {
        self.concurrency_limiter.clone()
    }

    /// Set a new concurrency semaphore to limit client network operations
    pub fn set_concurrency_limit(&mut self, limit: usize) {
        self.concurrency_limiter = Some(Arc::new(Semaphore::new(limit)));
    }

    /// Dial the given peer at the given address.
    pub async fn dial(&self, addr: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::Dial { addr, sender })?;
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

    /// Returns the closest peers to the given `NetworkAddress` that is fetched from the local
    /// Routing Table. It is ordered by increasing distance of the peers
    /// Note self peer_id is not included in the result.
    pub async fn get_closest_local_peers(&self, key: &NetworkAddress) -> Result<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetClosestLocalPeers {
            key: key.clone(),
            sender,
        })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
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
    ) -> Result<Vec<(PublicAddress, Token)>> {
        let (sender, receiver) = oneshot::channel();
        // get permit if semaphore supplied
        let mut _permit = None;
        if let Some(semaphore) = self.concurrency_limiter.clone() {
            let our_permit = semaphore.acquire_owned().await?;
            _permit = Some(our_permit);
        }

        debug!("Attempting to get store cost");
        // first we need to get CLOSE_GROUP of the dbc_id
        self.send_swarm_cmd(SwarmCmd::GetClosestPeers {
            key: record_address.clone(),
            sender,
        })?;

        let close_nodes = receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)?
            .into_iter()
            .collect_vec();

        let request = Request::Query(Query::GetStoreCost(record_address));
        let responses = self
            .send_and_get_responses(close_nodes, &request, true)
            .await;

        // loop over responses, generating an avergae fee and storing all responses along side
        let mut all_costs = vec![];
        for response in responses.into_iter().flatten() {
            if let Response::Query(QueryResponse::GetStoreCost {
                store_cost: Ok(cost),
                payment_address,
            }) = response
            {
                all_costs.push((payment_address, cost));
            } else {
                error!("Non store cost response received,  was {:?}", response);
            }
        }

        get_fee_from_store_cost_quotes(all_costs)
    }

    /// Get the Record from the network
    /// Carry out re-attempts if required
    /// In case a target_record is provided, only return when fetched target.
    /// Otherwise count it as a failure when all attempts completed.
    pub async fn get_record_from_network(
        &self,
        key: RecordKey,
        target_record: Option<Record>,
        re_attempt: bool,
    ) -> Result<Record> {
        let mut _permit = None;

        let total_attempts = if re_attempt { VERIFICATION_ATTEMPTS } else { 1 };

        let mut verification_attempts = 0;

        while verification_attempts < total_attempts {
            if let Some(semaphore) = self.concurrency_limiter.clone() {
                let our_permit = semaphore.acquire_owned().await?;
                _permit = Some(our_permit);
            }
            verification_attempts += 1;
            info!(
                "Getting record of {:?} attempts {verification_attempts:?}/{total_attempts:?}",
                PrettyPrintRecordKey::from(key.clone()),
            );

            let (sender, receiver) = oneshot::channel();
            self.send_swarm_cmd(SwarmCmd::GetNetworkRecord {
                key: key.clone(),
                sender,
            })?;

            match receiver
                .await
                .map_err(|_e| Error::InternalMsgChannelDropped)?
            {
                Ok(returned_record) => {
                    let header = RecordHeader::from_record(&returned_record)?;
                    let is_chunk = matches!(header.kind, RecordKind::Chunk);
                    info!(
                        "Record returned: {:?}",
                        PrettyPrintRecordKey::from(key.clone())
                    );

                    // Returning OK whenever fulfill one of the followings:
                    // 1, No targeting record
                    // 2, Fetched record matches the targeting record (when not chunk, as they are content addressed)
                    //
                    // Returning mismatched error when: completed all attempts
                    if target_record.is_none()
                        || (target_record.is_some()
                            // we dont need to match the whole record if chunks, 
                            // payment data could differ, but chunks themselves'
                            // keys are from the chunk address
                            && (target_record == Some(returned_record.clone()) || is_chunk))
                    {
                        return Ok(returned_record);
                    } else if verification_attempts >= total_attempts {
                        info!("Error: Returned record does not match target");
                        return Err(Error::ReturnedRecordDoesNotMatch(
                            returned_record.key.into(),
                        ));
                    }
                }
                Err(Error::RecordNotEnoughCopies(returned_record)) => {
                    // Only return when completed all attempts
                    if verification_attempts >= total_attempts {
                        if target_record.is_none()
                            || (target_record.is_some()
                                && target_record == Some(returned_record.clone()))
                        {
                            return Ok(returned_record);
                        } else {
                            return Err(Error::ReturnedRecordDoesNotMatch(
                                returned_record.key.into(),
                            ));
                        }
                    }
                }
                Err(error) => {
                    error!("{error:?}");
                    if verification_attempts >= total_attempts {
                        break;
                    }
                    warn!(
                        "Did not retrieve Record '{:?}' from network!. Retrying...",
                        PrettyPrintRecordKey::from(key.clone()),
                    );
                }
            }

            // drop any permit while we wait
            _permit = None;

            // wait for a bit before re-trying
            tokio::time::sleep(REVERIFICATION_WAIT_TIME_S).await;
        }

        Err(Error::RecordNotFound)
    }

    /// Get the cost of storing the next record from the network
    pub async fn get_local_storecost(&self) -> Result<Token> {
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
    /// optionally verify the record is stored after putting it to network
    pub async fn put_record(
        &self,
        record: Record,
        verify_store: bool,
        optional_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<()> {
        if verify_store {
            self.put_record_with_retries(record, verify_store, optional_permit)
                .await
        } else {
            self.put_record_once(record, false, optional_permit).await
        }
    }

    /// Put `Record` to network
    /// Verify the record is stored after putting it to network
    /// Retry up to `PUT_RECORD_RETRIES` times if we can't verify the record is stored
    async fn put_record_with_retries(
        &self,
        record: Record,
        verify_store: bool,
        mut optional_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<()> {
        let mut retries = 0;

        // let mut has_permit = optional_permit.is_some();
        // TODO: Move this put retry loop up above store cost checks so we can re-put if storecost failed.
        while retries < PUT_RECORD_RETRIES {
            trace!(
                "Attempting to PUT record of {:?} to network",
                PrettyPrintRecordKey::from(record.key.clone())
            );

            let res = self
                .put_record_once(record.clone(), verify_store, optional_permit)
                .await;
            if !matches!(res, Err(Error::FailedToVerifyRecordWasStored(_))) {
                return res;
            }

            // the permit will have been consumed above.
            optional_permit = None;

            retries += 1;
        }
        Err(Error::FailedToVerifyRecordWasStored(record.key.into()))
    }

    async fn put_record_once(
        &self,
        record: Record,
        verify_store: bool,
        starting_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<()> {
        let mut _permit = starting_permit;

        let record_key = record.key.clone();
        let pretty_key = PrettyPrintRecordKey::from(record_key.clone());
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

        drop(_permit);

        if verify_store {
            // small wait before we attempt to verify
            tokio::time::sleep(REVERIFICATION_WAIT_TIME_S).await;
            trace!("attempting to verify {pretty_key:?}");

            // Verify the record is stored, requiring re-attempts
            self.get_record_from_network(record_key, Some(record), true)
                .await
                .map_err(|e| {
                    trace!(
                        "Failing to verify the put record {:?} with error {e:?}",
                        pretty_key
                    );
                    Error::FailedToVerifyRecordWasStored(pretty_key)
                })?;
        }

        response
    }

    /// Put `Record` to the local RecordStore
    /// Must be called after the validations are performed on the Record
    #[allow(clippy::result_large_err)]
    pub fn put_local_record(&self, record: Record) -> Result<()> {
        debug!(
            "Writing Record locally, for {:?} - length {:?}",
            PrettyPrintRecordKey::from(record.key.clone()),
            record.value.len()
        );
        self.send_swarm_cmd(SwarmCmd::PutLocalRecord { record })
    }

    /// Returns true if a RecordKey is present locally in the RecordStore
    pub async fn is_key_present_locally(&self, key: &RecordKey) -> Result<bool> {
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
    pub async fn get_all_local_record_addresses(&self) -> Result<HashSet<NetworkAddress>> {
        let (sender, receiver) = oneshot::channel();
        self.send_swarm_cmd(SwarmCmd::GetAllLocalRecordAddresses { sender })?;

        receiver
            .await
            .map_err(|_e| Error::InternalMsgChannelDropped)
    }

    // Add a list of keys of a holder to Replication Fetcher.
    #[allow(clippy::result_large_err)]
    pub fn add_keys_to_replication_fetcher(&self, keys: Vec<NetworkAddress>) -> Result<()> {
        self.send_swarm_cmd(SwarmCmd::AddKeysToReplicationFetcher { keys })
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
    #[allow(clippy::result_large_err)]
    pub fn send_req_ignore_reply(&self, req: Request, peer: PeerId) -> Result<()> {
        let swarm_cmd = SwarmCmd::SendRequest {
            req,
            peer,
            sender: None,
        };
        self.send_swarm_cmd(swarm_cmd)
    }

    /// Send a `Response` through the channel opened by the requester.
    #[allow(clippy::result_large_err)]
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
    #[allow(clippy::result_large_err)]
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
        sort_peers_by_address(closest_peers, key, CLOSE_GROUP_SIZE)
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
        trace!("send_and_get_responses for {req:?}");
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
            trace!("Got response for the req: {req:?}, res: {res_string}");
            if !get_all_responses && res.is_ok() {
                return vec![res];
            }
            responses.push(res);
            list_of_futures = remaining_futures;
        }

        trace!("got all responses for {req:?}");
        responses
    }
}

/// Given `all_costs` it will return the CLOSE_GROUP majority cost.
#[allow(clippy::result_large_err)]
fn get_fee_from_store_cost_quotes(
    mut all_costs: Vec<(PublicAddress, Token)>,
) -> Result<Vec<(PublicAddress, Token)>> {
    // TODO: we should make this configurable based upon data type
    // or user requirements for resilience.
    let desired_quote_count = CLOSE_GROUP_SIZE;

    // sort all costs by fee, lowest to highest
    all_costs.sort_by(|(_, cost_a), (_, cost_b)| {
        cost_a
            .partial_cmp(cost_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // get the first desired_quote_count of all_costs
    all_costs.truncate(desired_quote_count);

    if all_costs.len() < desired_quote_count {
        return Err(Error::NotEnoughCostQuotes);
    }

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
    #[allow(clippy::result_large_err)]
    fn test_get_fee_from_store_cost_quotes() -> Result<()> {
        // for a vec of different costs of CLOUSE_GROUP size
        // ensure we return the CLOSE_GROUP / 2 indexed price
        let mut costs = vec![];
        for i in 0..CLOSE_GROUP_SIZE {
            let addr = PublicAddress::new(bls::SecretKey::random().public_key());
            costs.push((addr, Token::from_nano(i as u64)));
        }
        let prices = get_fee_from_store_cost_quotes(costs)?;
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
    fn test_get_any_fee_from_store_cost_quotes_errs_if_insufficient_quotes() -> eyre::Result<()> {
        // for a vec of different costs of CLOUSE_GROUP size
        // ensure we return the CLOSE_GROUP / 2 indexed price
        let mut costs = vec![];
        for i in 0..(CLOSE_GROUP_SIZE / 2) - 1 {
            let addr = PublicAddress::new(bls::SecretKey::random().public_key());
            costs.push((addr, Token::from_nano(i as u64)));
        }

        if get_fee_from_store_cost_quotes(costs).is_ok() {
            bail!("Should have errored as we have too few quotes")
        }

        Ok(())
    }
    #[test]
    #[ignore = "we want to pay the entire CLOSE_GROUP for now"]
    fn test_get_some_fee_from_store_cost_quotes_errs_if_suffcient() -> eyre::Result<()> {
        // for a vec of different costs of CLOUSE_GROUP size
        let quotes_count = CLOSE_GROUP_SIZE as u64 - 1;
        let mut costs = vec![];
        for i in 0..quotes_count {
            // push random PublicAddress and Token
            let addr = PublicAddress::new(bls::SecretKey::random().public_key());
            costs.push((addr, Token::from_nano(i)));
            println!("price added {}", i);
        }

        let prices = match get_fee_from_store_cost_quotes(costs) {
            Err(_) => bail!("Should not have errored as we have enough quotes"),
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
