// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    error::Error,
    spends::{aggregate_spends, check_parent_spends, get_aggregated_spends_from_peers},
    Node,
};
use libp2p::kad::{Record, RecordKey};
use sn_dbc::{DbcId, Hash};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{CmdOk, PaymentProof},
    storage::{
        try_deserialize_record, try_serialize_record, ChunkWithPayment, DbcAddress, RecordHeader,
        RecordKind, SpendWithParent,
    },
};
use sn_transfers::payment_proof::validate_payment_proof;
use std::collections::HashSet;

impl Node {
    /// Validate and store a `ChunkWithPayment` to the RecordStore
    pub(crate) async fn validate_and_store_chunk(
        &self,
        chunk_with_payment: ChunkWithPayment,
    ) -> Result<CmdOk, ProtocolError> {
        let chunk_name = *chunk_with_payment.chunk.name();
        debug!("validating and storing chunk {chunk_name:?}");

        let key = RecordKey::new(&chunk_name);
        let present_locally = self
            .network
            .is_key_present_locally(&key)
            .await
            .map_err(|err| {
                warn!("Error while checking if Chunk's key is present locally {err}");
                ProtocolError::ChunkNotStored(chunk_name)
            })?;

        if self
            .chunk_validation(&chunk_with_payment, present_locally)
            .await?
            .is_none()
        {
            // data is already present
            return Ok(CmdOk::DataAlreadyPresent);
        }

        let record = Record {
            key: RecordKey::new(chunk_with_payment.chunk.name()),
            value: try_serialize_record(&chunk_with_payment, RecordKind::Chunk)?,
            publisher: None,
            expires: None,
        };

        // finally store the Record directly into the local storage
        debug!("Storing chunk as Record locally");
        self.network.put_local_record(record).await.map_err(|err| {
            warn!("Error while locally storing Chunk as a Record{err}");
            ProtocolError::ChunkNotStored(chunk_name)
        })?;

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store `Vec<SpendWithParent>` to the RecordStore
    pub(crate) async fn validate_and_store_spends(
        &mut self,
        spends_with_parent: Vec<SpendWithParent>,
    ) -> Result<CmdOk, ProtocolError> {
        // make sure that the dbc_ids match
        let dbc_id = if let Some((first, elements)) = spends_with_parent.split_first() {
            let common_dbc_id = *first.signed_spend.dbc_id();
            if elements
                .iter()
                .all(|spend_with_parent| spend_with_parent.signed_spend.dbc_id() == &common_dbc_id)
            {
                common_dbc_id
            } else {
                let err = Err(ProtocolError::SpendNotStored(
                    "The dbc_id of the provided Vec<SpendWithParent> does not match".to_string(),
                ));
                error!("{err:?}");
                return err;
            }
        } else {
            let err = Err(ProtocolError::SpendNotStored(
                "Spend was not provided".to_string(),
            ));
            warn!("Empty vec provided to validate and store spend, {err:?}");
            return err;
        };
        let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);

        debug!("validating and storing spends {:?}", dbc_addr.name());
        let key = RecordKey::new(dbc_addr.name());

        let present_locally = self
            .network
            .is_key_present_locally(&key)
            .await
            .map_err(|_err| {
                let err = ProtocolError::SpendNotStored(format!(
                    "Error while checking if Spend's key was present locally, {dbc_addr:?}"
                ));
                warn!("{err:?}");
                err
            })?;

        // validate the signed spends against the network and the local copy
        let spends_with_parent = match self
            .signed_spend_validation(spends_with_parent, dbc_id, present_locally)
            .await?
        {
            Some(spends_with_parent) => spends_with_parent,
            None => {
                // data is already present
                return Ok(CmdOk::DataAlreadyPresent);
            }
        };

        // store the record into the local storage
        let record = Record {
            key,
            value: try_serialize_record(&spends_with_parent, RecordKind::DbcSpend)?,
            publisher: None,
            expires: None,
        };
        self.network.put_local_record(record).await.map_err(|_| {
            let err = ProtocolError::SpendNotStored(format!("Cannot PUT Spend with {dbc_addr:?}"));
            error!("Cannot put spend {err:?}");
            err
        })?;

        // Notify the sender of any double spend
        if spends_with_parent.len() > 1 {
            warn!("Got a double spend for the SpendDbc PUT with dbc_id {dbc_id:?}",);
            let mut proof = spends_with_parent.iter();
            if let (Some(spend_one), Some(spend_two)) = (proof.next(), proof.next()) {
                return Err(ProtocolError::DoubleSpendAttempt(
                    Box::new(spend_one.to_owned()),
                    Box::new(spend_two.to_owned()),
                ))?;
            }
        }

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store a `Record` directly. This is used to fall back to KAD's PUT flow instead
    /// of using our custom Cmd flow.
    /// Also prevents Record overwrite through `malicious_node.kademlia.put_record()` which would trigger
    /// trigger a SwarmEvent that is propagated to here to be handled.
    /// Note: KAD's PUT does not support error propagation. Only the error variants inside
    /// `RecordStore` are propagated.
    pub(crate) async fn validate_and_store_record(&self, mut record: Record) -> Result<(), Error> {
        let header = RecordHeader::from_record(&record)?;
        let present_locally = self.network.is_key_present_locally(&record.key).await?;

        match header.kind {
            RecordKind::Chunk => {
                // return early without deserializing
                if present_locally {
                    // We outright short circuit if the Record::key is present locally;
                    // Hence we don't have to verify if the local_header::kind == Chunk
                    debug!(
                        "Chunk with key {:?} already exists, not overwriting",
                        record.key
                    );
                    return Ok(());
                }

                let chunk_with_payment: ChunkWithPayment = try_deserialize_record(&record)?;
                let addr = chunk_with_payment.chunk.name();
                // check if the deserialized value's ChunkAddress matches the record's key
                if record.key != RecordKey::new(&addr) {
                    error!(
                        "Record's key does not match with the value's ChunkAddress, ignoring PUT."
                    );
                    return Err(ProtocolError::RecordKeyMismatch.into());
                }

                // Common validation logic. Can ignore the new_data_present as we already have a
                // check for it earlier in the code that allows us to bail out early without
                // deserializing the `Record`
                let _new_data_present = self
                    .chunk_validation(&chunk_with_payment, present_locally)
                    .await?;
            }
            RecordKind::DbcSpend => {
                let spends_with_parent: Vec<SpendWithParent> = try_deserialize_record(&record)?;
                // Prevents someone from crafting large Vec and slowing down nodes
                if spends_with_parent.len() > 2 {
                    warn!(
                        "Discarding incoming DbcSpend PUT as it contains more than 2 SignedSpends"
                    );
                    return Err(ProtocolError::MaxNumberOfSpendsExceeded.into());
                }

                // filter out the spends whose DbcAddress does not match with Record::key
                let spends_with_parent = spends_with_parent.into_iter().filter(|s| {
                    let dbc_addr = DbcAddress::from_dbc_id(s.signed_spend.dbc_id());
                    if record.key != RecordKey::new(dbc_addr.name()) {
                        warn!(
                        "Record's key {:?} does not match with the value's DbcAddress {dbc_addr:?}. Filtering it out.",
                            record.key,
                    );
                        false
                    } else {
                        true
                    }

                }).collect::<Vec<_>>();

                if spends_with_parent.is_empty() {
                    warn!("No spend with valid Record key found. Ignoring DbcSpend PUT request");
                    return Err(ProtocolError::RecordKeyMismatch.into());
                }
                // Since spends_with_parent is not empty and they contain the same DbcId (RecordKeys are
                // the same), get the DbcId
                let dbc_id = *spends_with_parent[0].signed_spend.dbc_id();
                match self
                    .signed_spend_validation(spends_with_parent, dbc_id, present_locally)
                    .await?
                {
                    Some(spends_with_parent) => {
                        // Just log the double spent attempt as it cannot be propagated back
                        if spends_with_parent.len() > 1 {
                            warn!(
                                "Got a double spend for the SpendWithParent PUT with dbc_id {dbc_id:?}",
                            );
                        }

                        // replace the Record's value with the new one
                        let spends_with_parent =
                            try_serialize_record(&spends_with_parent, RecordKind::DbcSpend)?;
                        record.value = spends_with_parent;
                    }
                    None => {
                        // data already present
                        return Ok(());
                    }
                };
            }
            RecordKind::Register => {
                if present_locally {
                    warn!("Overwrite attempt handling for Registers has not been implemented yet. key {:?}", record.key);
                    return Ok(());
                }
            }
        }

        // finally store the Record directly into the local storage
        self.network.put_local_record(record).await?;

        Ok(())
    }

    /// Perform validations on the provided `ChunkWithPayment`. Returns `Some(())>` if the Chunk has to be
    /// stored stored to the RecordStore
    /// - Return early if overwrite attempt
    /// - Validate the provided payment proof
    async fn chunk_validation(
        &self,
        chunk_with_payment: &ChunkWithPayment,
        present_locally: bool,
    ) -> Result<Option<()>, ProtocolError> {
        let addr = *chunk_with_payment.chunk.address();
        let addr_name = *addr.name();

        // return early without validation
        if present_locally {
            // We outright short circuit if the Record::key is present locally;
            // Hence we don't have to verify if the local_header::kind == Chunk
            debug!("Chunk with addr {addr:?} already exists, not overwriting",);
            return Ok(None);
        }

        // TODO: temporarily payment proof is optional
        if let Some(PaymentProof {
            tx,
            audit_trail,
            path,
        }) = &chunk_with_payment.payment
        {
            // TODO: check the expected amount of tokens was paid by the Tx, i.e. the amount one of the
            // outputs sent (unblinded) is the expected, and the paid address is the predefined "burning address"

            // We need to fetch the inputs of the DBC tx in order to obtain the reason-hash and
            // other info for verifications of valid payment.
            // TODO: perform verifications in multiple concurrent tasks
            let mut reasons = Vec::<Hash>::new();
            for input in &tx.inputs {
                let dbc_id = input.dbc_id();
                let addr = DbcAddress::from_dbc_id(&dbc_id);
                match get_aggregated_spends_from_peers(&self.network, dbc_id).await {
                    Ok(mut signed_spends) => {
                        match signed_spends.len() {
                            0 => {
                                error!("Could not get spends from the network");
                                return Err(ProtocolError::SpendNotFound(addr));
                            }
                            1 => {
                                match signed_spends.pop() {
                                    Some(signed_spend) => {
                                        // TODO: self-verification of the signed spend?

                                        if &signed_spend.signed_spend.spent_tx() != tx {
                                            return Err(ProtocolError::PaymentProofTxMismatch(
                                                addr_name,
                                            ));
                                        }

                                        reasons.push(signed_spend.signed_spend.reason());
                                    }
                                    None => return Err(ProtocolError::SpendNotFound(addr)),
                                }
                            }
                            _ => {
                                warn!("Got a double spend for during chunk payment validation {dbc_id:?}",);
                                let mut proof = signed_spends.iter();
                                if let (Some(spend_one), Some(spend_two)) =
                                    (proof.next(), proof.next())
                                {
                                    return Err(ProtocolError::DoubleSpendAttempt(
                                        Box::new(spend_one.to_owned()),
                                        Box::new(spend_two.to_owned()),
                                    ))?;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        error!("Error getting payment's input DBC {dbc_id:?} from network: {err}");
                        return Err(ProtocolError::SpendNotFound(addr));
                    }
                }
            }

            match reasons.first() {
                None => return Err(ProtocolError::PaymentProofWithoutInputs(addr_name)),
                Some(reason_hash) => {
                    // check all reasons are the same
                    if !reasons.iter().all(|r| r == reason_hash) {
                        return Err(ProtocolError::PaymentProofInconsistentReason(addr_name));
                    }

                    // check the reason hash verifies the merkle-tree audit trail and path against the content address name
                    validate_payment_proof(addr_name, reason_hash, audit_trail, path).map_err(
                        |err| ProtocolError::InvalidPaymentProof {
                            addr_name,
                            reason: err.to_string(),
                        },
                    )?
                }
            }
        }

        Ok(Some(()))
    }

    /// Perform validations on the provided `Vec<SpendWithParent>`. Returns `Some<Vec<SpendWithParent>>` if
    /// the spends has to be stored to the `RecordStore` where the spends are aggregated and can
    /// have a max of only 2 elements. Any double spend error has to be thrown by the caller.
    ///
    /// The Vec<SpendWithParent> must all have the same dbc_id.
    ///
    /// - If the SpendWithParent for the provided DbcId is present locally, check for new spends by
    /// comparing it with the local copy.
    /// - If incoming spends_with_parent.len() > 1, aggregate store them directly as they are a double spent.
    /// - If incoming spends_with_parent.len() == 1, then check for parent_inputs and the closet(dbc_id)
    /// for any double spend, which are then aggregated and returned.
    async fn signed_spend_validation(
        &self,
        mut spends_with_parent: Vec<SpendWithParent>,
        dbc_id: DbcId,
        present_locally: bool,
    ) -> Result<Option<Vec<SpendWithParent>>, ProtocolError> {
        // get the DbcId; used for validation
        let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);
        let record_key = RecordKey::new(dbc_addr.name());

        if present_locally {
            debug!("DbcSpend with DbcId {dbc_id:?} already exists, checking if it's the same spend/double spend",);
            // fetch the locally stored record; should be present
            //
            let local_record = self
                .network
                .get_local_record(&record_key)
                .await
                .map_err(|err| {
                    let err = ProtocolError::SpendNotStored(format!(
                        "Error while fetching local record {err}"
                    ));
                    warn!("{err:?}");
                    err
                })?;
            let local_record = match local_record {
                Some(r) => r,
                None => {
                    error!("Could not retrieve Record with key{record_key:?}, the Record is supposed to be present.");
                    return Err(ProtocolError::SpendNotFound(dbc_addr));
                }
            };

            let local_header = RecordHeader::from_record(&local_record)?;
            // Make sure the local copy is of the same kind
            if !matches!(local_header.kind, RecordKind::DbcSpend) {
                error!("Expected DbcRecord kind, found {:?}", local_header.kind);
                return Err(ProtocolError::RecordKindMismatch(RecordKind::DbcSpend));
            }

            let local_spends_with_parent: Vec<SpendWithParent> =
                try_deserialize_record(&local_record)?;

            // spends that are not present locally
            let newly_seen_spends = spends_with_parent
                .iter()
                .filter(|s| !local_spends_with_parent.contains(s))
                .cloned()
                .collect::<HashSet<_>>();

            // return early if the PUT is for the same local copy
            if newly_seen_spends.is_empty() {
                debug!(
                    "Vec<SpendWithParent> with addr {dbc_addr:?} already exists, not overwriting!",
                );
                return Ok(None);
            } else {
                debug!(
                    "Seen new spends that are not part of the local copy. Mostly a double spend, checking for it"
                );
                // continue with local_spends + new_ones
                spends_with_parent = local_spends_with_parent
                    .into_iter()
                    .chain(newly_seen_spends)
                    .collect();
            }
        }

        // Check the parent spends and check the closest(dbc_id) for any double spend
        // if so aggregate the spends and return just 2 spends.
        let spends_with_parent = if spends_with_parent.len() == 1 {
            debug!(
                "Received a single SpendWithParent, verifying the parent and checking for double spend"
            );
            let spends_with_parent = spends_with_parent.remove(0);

            // check the spend
            if let Err(e) = spends_with_parent
                .signed_spend
                .verify(spends_with_parent.signed_spend.spent_tx_hash())
            {
                return Err(ProtocolError::InvalidSpendSignature(format!(
                    "while verifying spend for {:?}: {e:?}",
                    spends_with_parent.signed_spend.dbc_id()
                )));
            }

            // check parent tx hash
            if spends_with_parent.parent_tx.hash()
                != spends_with_parent.signed_spend.dbc_creation_tx_hash()
            {
                return Err(ProtocolError::InvalidSpendParents(format!(
                    "parent tx hash does not match the parent tx in the DBC we're trying to spend: {:?} != {:?}",
                    spends_with_parent.signed_spend.dbc_creation_tx_hash(),
                    spends_with_parent.parent_tx.hash(),
                )));
            }

            // Check parents
            if let Err(e) = check_parent_spends(&self.network, &spends_with_parent).await {
                return Err(ProtocolError::InvalidSpendParents(format!("{e:?}")));
            }

            // check the network if any spend has happened for the same dbc_id
            // Does not return an error, instead the Vec<SpendWithParent> is returned.
            let mut spends = get_aggregated_spends_from_peers(&self.network, dbc_id).await?;
            // aggregate the spends from the network with our own
            spends.push(spends_with_parent);
            aggregate_spends(spends, dbc_id)
        } else {
            debug!("Received >1 spends with parent. Aggregating the spends to check for double spend. Not performing parent check or querying the network for double spend");
            // if we got 2 or more, then it is a double spend for sure.
            // We don't have to check parent/ ask network for extra spend.
            // Validate and store just 2 of them.
            // The nodes will be synced up during replication.
            aggregate_spends(spends_with_parent, dbc_id)
        };

        Ok(Some(spends_with_parent))
    }
}
