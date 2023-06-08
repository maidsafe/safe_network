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
use sn_dbc::Hash;
use sn_dbc::{DbcId, SignedSpend};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{CmdOk, PaymentProof},
    storage::{ChunkWithPayment, DbcAddress, RecordHeader, RecordKind},
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
            value: Self::try_serialize_record(&chunk_with_payment, RecordKind::Chunk)?,
            publisher: None,
            expires: None,
        };

        // finally store the Record directly into the local storage
        self.network.put_local_record(record).await.map_err(|err| {
            warn!("Error while locally storing Chunk as a Record{err}");
            ProtocolError::ChunkNotStored(chunk_name)
        })?;

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store a `SignedSpend` to the RecordStore
    pub(crate) async fn validate_and_store_spend(
        &mut self,
        spends: Vec<SignedSpend>,
    ) -> Result<CmdOk, ProtocolError> {
        // make sure that the dbc_ids match
        let dbc_id = if let Some((first, elements)) = spends.split_first() {
            let common_dbc_id = *first.dbc_id();
            if elements
                .iter()
                .all(|spend| spend.dbc_id() == &common_dbc_id)
            {
                common_dbc_id
            } else {
                println!("The dbc_id of the provided signed_spends do not match");
                return Err(ProtocolError::SpendNotStored(None));
            }
        } else {
            warn!("Empty vec provided to validate and store spend");
            return Err(ProtocolError::SpendNotStored(None));
        };
        let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);
        let key = RecordKey::new(dbc_addr.name());

        let present_locally = self
            .network
            .is_key_present_locally(&key)
            .await
            .map_err(|err| {
                warn!("Error while checking if Spend's key is present locally {err}");
                ProtocolError::SpendNotStored(Some(dbc_addr))
            })?;

        // validate the signed spends against the network and the local copy
        let signed_spends = match self
            .signed_spend_validation(spends, dbc_id, present_locally)
            .await?
        {
            Some(signed_spends) => signed_spends,
            None => {
                // data is already present
                return Ok(CmdOk::DataAlreadyPresent);
            }
        };

        // store the record into the local storage
        let record = Record {
            key,
            value: Self::try_serialize_record(&signed_spends, RecordKind::DbcSpend)?,
            publisher: None,
            expires: None,
        };
        self.network
            .put_local_record(record)
            .await
            .map_err(|_| ProtocolError::SpendNotStored(Some(dbc_addr)))?;

        // Notify the sender of any double spend
        if signed_spends.len() > 1 {
            warn!("Got a double spend for the SignedSpend PUT with dbc_id {dbc_id:?}",);
            let mut proof = signed_spends.iter();
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
    /// Note: Kad's PUT does not support error propagation. Only the error variants inside
    /// `RecordStore` are propagated.
    pub(crate) async fn validate_and_store_record(&self, mut record: Record) -> Result<(), Error> {
        let header = RecordHeader::from_record(&record)?;
        let present_locally = self.network.is_key_present_locally(&record.key).await?;

        match header.kind {
            RecordKind::Chunk => {
                // return early without deserializing
                if present_locally {
                    // We outright short circuit if the Record::key is present locally;
                    // Hence we dont have to verify if the local_header::kind == Chunk
                    debug!(
                        "Chunk with key {:?} already exists, not overwriting",
                        record.key
                    );
                    return Ok(());
                }

                let chunk_with_payment: ChunkWithPayment = Self::try_deserialize_record(&record)?;
                let addr = chunk_with_payment.chunk.name();
                // check if the deserialized value's ChunkAddress matches the record's key
                if record.key != RecordKey::new(&addr) {
                    error!(
                        "Record's key does not match with the value's ChunkAddress, ignoring PUT."
                    );
                    return Err(ProtocolError::RecordKeyMismatch.into());
                }

                // Common validation logic. Can ignore the new_data_present as we already have a
                // check for it earlier in the code that allows us to bail out early wihtout
                // deserializing the `Record`
                let _new_data_present = self
                    .chunk_validation(&chunk_with_payment, present_locally)
                    .await?;
            }
            RecordKind::DbcSpend => {
                let signed_spends: Vec<SignedSpend> = Self::try_deserialize_record(&record)?;
                // Prevents someone from crafting large Vec and slowing down nodes
                if signed_spends.len() > 2 {
                    warn!(
                        "Discarding incoming DbcSpend PUT as it contains more than 2 SignedSpends"
                    );
                    return Err(ProtocolError::IncorrectSignedSpendLength.into());
                }

                // filter out the spends whose DbcAddress does not match with Record::key
                let signed_spends = signed_spends.into_iter().filter(|s| {
                    let dbc_addr = DbcAddress::from_dbc_id(s.dbc_id());
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

                if signed_spends.is_empty() {
                    warn!("No spend with valid Record key found. Ignoring DbcSpend PUT request");
                    return Err(ProtocolError::RecordKeyMismatch.into());
                }
                // Since signed_spends is not empty and they contain the same DbcId (RecordKeys are
                // the same), get the DbcId
                let dbc_id = *signed_spends[0].dbc_id();
                match self
                    .signed_spend_validation(signed_spends, dbc_id, present_locally)
                    .await?
                {
                    Some(signed_spends) => {
                        // Just log the double spent attempt as it cannot be propagated back
                        if signed_spends.len() > 1 {
                            warn!(
                                "Got a double spend for the SignedSpend PUT with dbc_id {dbc_id:?}",
                            );
                        }

                        // replace the Record's value with the new one
                        let signed_spends =
                            Self::try_serialize_record(&signed_spends, RecordKind::DbcSpend)?;
                        record.value = signed_spends;
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

    /// Perfrom validations on the provided `ChunkWithPayment`. Retruns `Some(())>` if the Chunk has to be
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
                    Ok(signed_spend) => {
                        // TODO: self-verification of the signed spend?

                        // if &signed_spend.spent_tx() != tx {
                        //     return Err(ProtocolError::PaymentProofTxMismatch(addr_name));
                        // }

                        // reasons.push(signed_spend.reason());
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

    /// Perfrom validations on the provided `Vec<SignedSpend>`. Returns `Some<Vec<SignedSpend>>` if
    /// the spends has to be stored to the `RecordStore` where the spends are aggregated and can
    /// have a max of only 2 elements. Any double spend error has to be thrown by the caller.
    ///
    /// The Vec<SignedSpend> must all have the same dbc_id.
    ///
    /// - If the SignedSpend for the provided DbcId is present locally, check for new spends by
    /// comparing it with the local copy.
    /// - If incoming signed_spends.len() > 1, aggregate store them directly as they are a double spent.
    /// - If incoming signed_spends.len() == 1, then check for parent_inputs and the closet(dbc_id)
    /// for any double spend, which are then aggregated and returned.
    async fn signed_spend_validation(
        &self,
        mut signed_spends: Vec<SignedSpend>,
        dbc_id: DbcId,
        present_locally: bool,
    ) -> Result<Option<Vec<SignedSpend>>, ProtocolError> {
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
                    warn!("Error while fetching local record {err}");
                    ProtocolError::SpendNotStored(Some(dbc_addr))
                })?;
            let local_record = match local_record {
                Some(r) => r,
                None => {
                    error!("Could not retreive Record with key{record_key:?}, the Record is supposed to be present.");
                    return Err(ProtocolError::SpendNotFound(dbc_addr));
                }
            };

            let local_header = RecordHeader::from_record(&local_record)?;
            // Make sure the local copy is of the same kind
            if !matches!(local_header.kind, RecordKind::DbcSpend) {
                error!("Expected DbcRecord kind, found {:?}", local_header.kind);
                return Err(ProtocolError::SpendNotStored(Some(dbc_addr)));
            }

            let local_signed_spends: Vec<SignedSpend> =
                Self::try_deserialize_record(&local_record)?;

            // spends that are not present locally
            let newly_seen_spends = signed_spends
                .iter()
                .filter(|s| !local_signed_spends.contains(s))
                .cloned()
                .collect::<HashSet<_>>();

            // return early if the PUT is for the same local copy
            if newly_seen_spends.is_empty() {
                debug!("Vec<SignedSpend> with addr {dbc_addr:?} already exists, not overwriting!",);
                return Ok(None);
            } else {
                // continue with local_spends + new_ones
                signed_spends = local_signed_spends
                    .into_iter()
                    .chain(newly_seen_spends)
                    .collect();
            }
        }

        // Check the parent spends and check the closest(dbc_id) for any double spend
        // if so aggregate the spends and return just 2 spends.
        let signed_spends = if signed_spends.len() == 1 {
            let signed_spend = signed_spends.remove(0);
            // Returns an error if any of the parent_input has a DoubleSpend
            check_parent_spends(&self.network, &signed_spend).await?;

            // check the network if any spend has happned for the same dbc_id
            // Does not return an error, instead the Vec<SignedSpend> is returned.
            let mut spends = get_aggregated_spends_from_peers(&self.network, dbc_id).await?;
            // aggregate the spends from the network with our own
            spends.push(signed_spend);
            aggregate_spends(spends, dbc_id)
        } else {
            // if we got 2 or more, then it is a double spend for sure.
            // We don't have to check parent/ ask network for extra spend.
            // Validate and store just 2 of them.
            aggregate_spends(signed_spends, dbc_id)
        };

        Ok(Some(signed_spends))
    }

    fn try_deserialize_record<T: serde::de::DeserializeOwned>(
        record: &Record,
    ) -> Result<T, ProtocolError> {
        let bytes = &record.value[RecordHeader::SIZE..];
        let value = bincode::deserialize(bytes).map_err(|_| ProtocolError::RecordParsingFailed);
        if let Err(err) = &value {
            warn!("Error while deserializing Record to a value {err}");
        }
        value
    }

    fn try_serialize_record<T: serde::Serialize>(
        data: &T,
        record_kind: RecordKind,
    ) -> Result<Vec<u8>, ProtocolError> {
        let payload = match bincode::serialize(data).map_err(|_| ProtocolError::RecordParsingFailed)
        {
            Ok(p) => p,
            Err(err) => {
                error!("Error while serializing data to Record");
                return Err(err);
            }
        };

        let record_header = RecordHeader { kind: record_kind };

        let mut record_value = match bincode::serialize(&record_header)
            .map_err(|_| ProtocolError::RecordParsingFailed)
        {
            Ok(r) => r,
            Err(err) => {
                error!("Error while serializing RecordHeader");
                return Err(err);
            }
        };

        record_value.extend(payload);
        Ok(record_value)
    }
}
