// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    close_group_majority, Error, GetRecordCfg, GetRecordError, Result, SwarmDriver,
    CLOSE_GROUP_SIZE,
};
use libp2p::{
    kad::{self, PeerRecord, ProgressStep, QueryId, QueryResult, QueryStats, Quorum, Record},
    PeerId,
};
use sn_protocol::PrettyPrintRecordKey;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use tokio::sync::oneshot;
use xor_name::XorName;

/// Using XorName to differentiate different record content under the same key.
type GetRecordResultMap = HashMap<XorName, (Record, HashSet<PeerId>)>;
pub(crate) type PendingGetRecord = HashMap<
    QueryId,
    (
        oneshot::Sender<std::result::Result<Record, GetRecordError>>,
        GetRecordResultMap,
        GetRecordCfg,
    ),
>;

// For `get_record` returning behaviour:
//   1, targeting a non-existing entry
//     there will only be one event of `kad::Event::OutboundQueryProgressed`
//     with `ProgressStep::last` to be `true`
//          `QueryStats::requests` to be 20 (K-Value)
//          `QueryStats::success` to be over majority of the requests
//          `err::NotFound::closest_peers` contains a list of CLOSE_GROUP_SIZE peers
//   2, targeting an existing entry
//     there will a sequence of (at least CLOSE_GROUP_SIZE) events of
//     `kad::Event::OutboundQueryProgressed` to be received
//     with `QueryStats::end` always being `None`
//          `ProgressStep::last` all to be `false`
//          `ProgressStep::count` to be increased with step of 1
//             capped and stopped at CLOSE_GROUP_SIZE, may have duplicated counts
//          `PeerRecord::peer` could be None to indicate from self
//             in which case it always use a duplicated `ProgressStep::count`
//     the sequence will be completed with `FinishedWithNoAdditionalRecord`
//     where: `cache_candidates`: being the peers supposed to hold the record but not
//            `ProgressStep::count`: to be `number of received copies plus one`
//            `ProgressStep::last` to be `true`
impl SwarmDriver {
    // Accumulates the GetRecord query results
    // If we get enough responses (quorum) for a record with the same content hash:
    // - we return the Record after comparing with the target record. This might return RecordDoesNotMatch if the
    // check fails.
    // - if multiple content hashes are found, we return a SplitRecord Error
    // And then we stop the kad query as we are done here.
    pub(crate) fn accumulate_get_record_found(
        &mut self,
        query_id: QueryId,
        peer_record: PeerRecord,
        stats: QueryStats,
        step: ProgressStep,
    ) -> Result<()> {
        let peer_id = if let Some(peer_id) = peer_record.peer {
            peer_id
        } else {
            self.self_peer_id
        };

        if let Entry::Occupied(mut entry) = self.pending_get_record.entry(query_id) {
            let (_sender, result_map, cfg) = entry.get_mut();

            let pretty_key = PrettyPrintRecordKey::from(&peer_record.record.key).into_owned();

            if !cfg.expected_holders.is_empty() {
                if cfg.expected_holders.remove(&peer_id) {
                    debug!("For record {pretty_key:?} task {query_id:?}, received a copy from an expected holder {peer_id:?}");
                } else {
                    debug!("For record {pretty_key:?} task {query_id:?}, received a copy from an unexpected holder {peer_id:?}");
                }
            }

            // Insert the record and the peer into the result_map.
            let record_content_hash = XorName::from_content(&peer_record.record.value);
            let responded_peers =
                if let Entry::Occupied(mut entry) = result_map.entry(record_content_hash) {
                    let (_, peer_list) = entry.get_mut();
                    let _ = peer_list.insert(peer_id);
                    peer_list.len()
                } else {
                    let mut peer_list = HashSet::new();
                    let _ = peer_list.insert(peer_id);
                    result_map.insert(record_content_hash, (peer_record.record.clone(), peer_list));
                    1
                };

            let expected_answers = match cfg.get_quorum {
                Quorum::Majority => close_group_majority(),
                Quorum::All => CLOSE_GROUP_SIZE,
                Quorum::N(v) => v.get(),
                Quorum::One => 1,
            };

            trace!("Expecting {expected_answers:?} answers for record {pretty_key:?} task {query_id:?}, received {responded_peers} so far");

            if responded_peers >= expected_answers {
                if !cfg.expected_holders.is_empty() {
                    debug!("For record {pretty_key:?} task {query_id:?}, fetch completed with non-responded expected holders {:?}", cfg.expected_holders);
                }
                let cfg = cfg.clone();

                // Remove the query task and consume the variables.
                let (sender, result_map, _) = entry.remove();

                if result_map.len() == 1 {
                    Self::send_record_after_checking_target(sender, peer_record.record, &cfg)?;
                } else {
                    debug!("For record {pretty_key:?} task {query_id:?}, fetch completed with split record");
                    sender
                        .send(Err(GetRecordError::SplitRecord { result_map }))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;
                }

                // Stop the query; possibly stops more nodes from being queried.
                if let Some(mut query) = self.swarm.behaviour_mut().kademlia.query_mut(&query_id) {
                    query.finish();
                }
            } else if usize::from(step.count) >= CLOSE_GROUP_SIZE {
                debug!("For record {pretty_key:?} task {query_id:?}, got {:?} with {} versions so far.",
                   step.count, result_map.len());
            }
        } else {
            // return error if the entry cannot be found
            return Err(Error::ReceivedKademliaEventDropped(
                kad::Event::OutboundQueryProgressed {
                    id: query_id,
                    result: QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
                    stats,
                    step,
                },
            ));
        }
        Ok(())
    }

    // Handles the possible cases when a GetRecord Query completes.
    // The accumulate_get_record_found returns the record if the quorum is satisfied, but, if we have reached this point
    // then we did not get enough records or we got split records (which prevented the quorum to pass).
    // Returns the following errors:
    // RecordNotFound if the result_map is empty.
    // NotEnoughCopies if there is only a single content hash version.
    // SplitRecord if there are multiple content hash versions.
    pub(crate) fn handle_get_record_finished(
        &mut self,
        query_id: QueryId,
        step: ProgressStep,
    ) -> Result<()> {
        // return error if the entry cannot be found
        if let Some((sender, result_map, cfg)) = self.pending_get_record.remove(&query_id) {
            let num_of_versions = result_map.len();
            let (result, log_string) = if let Some((record, _)) = result_map.values().next() {
                let result = if num_of_versions == 1 {
                    Err(GetRecordError::NotEnoughCopies(record.clone()))
                } else {
                    Err(GetRecordError::SplitRecord {
                        result_map: result_map.clone(),
                    })
                };

                (
                result,
                format!("Getting record {:?} completed with only {:?} copies received, and {num_of_versions} versions.",
                    PrettyPrintRecordKey::from(&record.key), usize::from(step.count) - 1)
                )
            } else {
                (
                Err(GetRecordError::RecordNotFound),
                format!("Getting record task {query_id:?} completed with step count {:?}, but no copy found.", step.count),
                )
            };

            if cfg.expected_holders.is_empty() {
                debug!("{log_string}");
            } else {
                debug!(
                    "{log_string}, and {:?} expected holders not responded",
                    cfg.expected_holders
                );
            }

            sender
                .send(result)
                .map_err(|_| Error::InternalMsgChannelDropped)?;
        } else {
            // We manually perform `query.finish()` if we return early from accumulate fn.
            // Thus we will still get FinishedWithNoAdditionalRecord.
            trace!("Can't locate query task {query_id:?} during GetRecord finished. We might have already returned the result to the sender.");
        }
        Ok(())
    }

    /// Handles the possible cases when a kad GetRecord returns an error.
    /// If we get NotFound/QuorumFailed, we return a RecordNotFound error. Kad currently does not enforce any quorum.
    /// If we get a Timeout:
    /// - return a QueryTimeout if we get a split record (?) if we have multiple content hashes.
    /// - if the quorum is satisfied, we return the record after comparing it with the target record. This might return
    /// RecordDoesNotMatch if the check fails.
    /// - else we return q QueryTimeout error.
    pub(crate) fn handle_get_record_error(
        &mut self,
        query_id: QueryId,
        get_record_err: kad::GetRecordError,
        stats: QueryStats,
        step: ProgressStep,
    ) -> Result<()> {
        match &get_record_err {
            kad::GetRecordError::NotFound { .. } | kad::GetRecordError::QuorumFailed { .. } => {
                // return error if the entry cannot be found
                let (sender, _, cfg) =
                self.pending_get_record.remove(&query_id).ok_or_else(|| {
                    trace!("Can't locate query task {query_id:?}, it has likely been completed already.");
                    Error::ReceivedKademliaEventDropped( kad::Event::OutboundQueryProgressed {
                        id: query_id,
                        result: QueryResult::GetRecord(Err(get_record_err.clone())),
                        stats,
                        step,
                    })
                })?;

                if cfg.expected_holders.is_empty() {
                    info!("Get record task {query_id:?} failed with error {get_record_err:?}");
                } else {
                    debug!("Get record task {query_id:?} failed with {:?} expected holders not responded, error {get_record_err:?}", cfg.expected_holders);
                }
                sender
                    .send(Err(GetRecordError::RecordNotFound))
                    .map_err(|_| Error::InternalMsgChannelDropped)?;
            }
            kad::GetRecordError::Timeout { key } => {
                // return error if the entry cannot be found
                let pretty_key = PrettyPrintRecordKey::from(key);
                let (sender, result_map, cfg) =
                    self.pending_get_record.remove(&query_id).ok_or_else(|| {
                        trace!(
                            "Can't locate query task {query_id:?} for {pretty_key:?}, it has likely been completed already."
                        );
                        Error::ReceivedKademliaEventDropped( kad::Event::OutboundQueryProgressed {
                            id: query_id,
                            result: QueryResult::GetRecord(Err(get_record_err.clone())),
                            stats,
                            step,
                        })
                    })?;

                let required_response_count = match cfg.get_quorum {
                    Quorum::Majority => close_group_majority(),
                    Quorum::All => CLOSE_GROUP_SIZE,
                    Quorum::N(v) => v.into(),
                    Quorum::One => 1,
                };

                // if we've a split over the result xorname, then we don't attempt to resolve this here.
                // Retry and resolve through normal flows without a timeout.
                // todo: is the above still the case? Why don't we return a split record error.
                if result_map.len() > 1 {
                    warn!(
                        "Get record task {query_id:?} for {pretty_key:?} timed out with split result map"
                    );
                    sender
                        .send(Err(GetRecordError::QueryTimeout))
                        .map_err(|_| Error::InternalMsgChannelDropped)?;

                    return Ok(());
                }

                // if we have enough responses here, we can return the record
                if let Some((record, peers)) = result_map.values().next() {
                    if peers.len() >= required_response_count {
                        Self::send_record_after_checking_target(sender, record.clone(), &cfg)?;
                        return Ok(());
                    }
                }

                warn!("Get record task {query_id:?} for {pretty_key:?} returned insufficient responses. {:?} did not return record", cfg.expected_holders);
                // Otherwise report the timeout
                sender
                    .send(Err(GetRecordError::QueryTimeout))
                    .map_err(|_| Error::InternalMsgChannelDropped)?;
            }
        }

        Ok(())
    }

    fn send_record_after_checking_target(
        sender: oneshot::Sender<std::result::Result<Record, GetRecordError>>,
        record: Record,
        cfg: &GetRecordCfg,
    ) -> Result<()> {
        if cfg.target_record.is_none() || cfg.does_target_match(&record) {
            sender
                .send(Ok(record))
                .map_err(|_| Error::InternalMsgChannelDropped)
        } else {
            sender
                .send(Err(GetRecordError::RecordDoesNotMatch(record)))
                .map_err(|_| Error::InternalMsgChannelDropped)
        }
    }
}
