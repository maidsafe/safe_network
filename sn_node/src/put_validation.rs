// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    spends::{aggregate_spends, check_parent_spends},
    Node,
};
use libp2p::kad::Record;
use sn_dbc::{DbcId, DbcTransaction, Hash, SignedSpend, Token};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::{CmdOk, MerkleTreeNodesType, PaymentProof},
    storage::{
        try_deserialize_record, try_serialize_record, ChunkWithPayment, DbcAddress, RecordHeader,
        RecordKind,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;
use sn_transfers::{
    dbc_genesis::{is_genesis_parent_tx, GENESIS_DBC},
    payment_proof::validate_payment_proof,
};
use std::collections::{BTreeSet, HashSet};
use tokio::task::JoinSet;
use xor_name::XorName;

impl Node {
    /// Validate and store a record to the RecordStore
    pub(crate) async fn validate_and_store_record(
        &self,
        record: Record,
        validate_payment: bool,
    ) -> Result<CmdOk, ProtocolError> {
        let record_header = RecordHeader::from_record(&record)?;

        match record_header.kind {
            RecordKind::Chunk => {
                let chunk_with_payment = try_deserialize_record::<ChunkWithPayment>(&record)?;

                // check if the deserialized value's ChunkAddress matches the record's key
                let key = NetworkAddress::from_chunk_address(*chunk_with_payment.chunk.address())
                    .to_record_key();
                if record.key != key {
                    warn!(
                        "Record's key does not match with the value's ChunkAddress, ignoring PUT."
                    );
                    return Err(ProtocolError::RecordKeyMismatch);
                }

                self.validate_and_store_chunk(chunk_with_payment, validate_payment)
                    .await
            }
            RecordKind::DbcSpend => {
                let signed_spends = try_deserialize_record::<Vec<SignedSpend>>(&record)?;

                // check if all the DbcAddresses matches with Record::key
                if !signed_spends.iter().all(|spend| {
                    let dbc_addr = DbcAddress::from_dbc_id(spend.dbc_id());
                    record.key == NetworkAddress::from_dbc_address(dbc_addr).to_record_key()
                }) {
                    warn!("Record's key does not match with the value's DbcAddress, ignoring PUT.");
                    return Err(ProtocolError::RecordKeyMismatch);
                }

                self.validate_and_store_spends(signed_spends).await
            }
            RecordKind::Register => {
                let register = try_deserialize_record::<SignedRegister>(&record)?;

                // check if the deserialized value's RegisterAddress matches the record's key
                let key =
                    NetworkAddress::from_register_address(*register.address()).to_record_key();
                if record.key != key {
                    warn!(
                        "Record's key does not match with the value's RegisterAddress, ignoring PUT."
                    );
                    return Err(ProtocolError::RecordKeyMismatch);
                }
                self.validate_and_store_register(register).await
            }
        }
    }

    /// Validate and store a `ChunkWithPayment` to the RecordStore
    pub(crate) async fn validate_and_store_chunk(
        &self,
        chunk_with_payment: ChunkWithPayment,
        validate_payment: bool,
    ) -> Result<CmdOk, ProtocolError> {
        let chunk_name = *chunk_with_payment.chunk.name();
        debug!("validating and storing chunk {chunk_name:?}");

        let key =
            NetworkAddress::from_chunk_address(*chunk_with_payment.chunk.address()).to_record_key();
        let present_locally = self
            .network
            .is_key_present_locally(&key)
            .await
            .map_err(|err| {
                warn!("Error while checking if Chunk's key is present locally {err}");
                ProtocolError::ChunkNotStored(chunk_name)
            })?;

        // If data is already present return early without validation
        if present_locally {
            // We outright short circuit if the Record::key is present locally;
            // Hence we don't have to verify if the local_header::kind == Chunk
            debug!(
                "Chunk with addr {:?} already exists, not overwriting",
                chunk_with_payment.chunk.address()
            );
            return Ok(CmdOk::DataAlreadyPresent);
        }

        if validate_payment {
            self.chunk_payment_validation(&chunk_with_payment).await?;
        }

        let record = Record {
            key,
            value: try_serialize_record(&chunk_with_payment, RecordKind::Chunk)?,
            publisher: None,
            expires: None,
        };

        // finally store the Record directly into the local storage
        debug!("Storing chunk {chunk_name:?} as Record locally");
        self.network.put_local_record(record).map_err(|err| {
            warn!("Error while locally storing Chunk as a Record{err}");
            ProtocolError::ChunkNotStored(chunk_name)
        })?;

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store a `Register` to the RecordStore
    pub(crate) async fn validate_and_store_register(
        &self,
        register: SignedRegister,
    ) -> Result<CmdOk, ProtocolError> {
        let reg_addr = register.address();
        debug!("Validating and storing register {reg_addr:?}");

        // check if the Register is present locally
        let key = NetworkAddress::from_register_address(*reg_addr).to_record_key();
        let present_locally = self
            .network
            .is_key_present_locally(&key)
            .await
            .map_err(|err| {
                warn!("Error while checking if register's key is present locally {err}");
                ProtocolError::RegisterNotStored(Box::new(*reg_addr))
            })?;

        // check register and merge if needed
        let updated_register = match self.register_validation(&register, present_locally).await? {
            Some(reg) => reg,
            None => {
                return Ok(CmdOk::DataAlreadyPresent);
            }
        };

        // store in kad
        let record = Record {
            key,
            value: try_serialize_record(&updated_register, RecordKind::Register)?,
            publisher: None,
            expires: None,
        };
        debug!("Storing register {reg_addr:?} as Record locally");
        self.network.put_local_record(record).map_err(|err| {
            warn!("Error while locally storing register as a Record {err}");
            ProtocolError::RegisterNotStored(Box::new(*reg_addr))
        })?;

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store `Vec<SignedSpend>` to the RecordStore
    pub(crate) async fn validate_and_store_spends(
        &self,
        signed_spends: Vec<SignedSpend>,
    ) -> Result<CmdOk, ProtocolError> {
        // make sure that the dbc_ids match
        let dbc_id = if let Some((first, elements)) = signed_spends.split_first() {
            let common_dbc_id = *first.dbc_id();
            if elements
                .iter()
                .all(|spend| spend.dbc_id() == &common_dbc_id)
            {
                common_dbc_id
            } else {
                let err = Err(ProtocolError::SpendNotStored(
                    "found SignedSpends with differing dbc_ids".to_string(),
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

        let key = NetworkAddress::from_dbc_address(dbc_addr).to_record_key();
        debug!(
            "validating and storing spends {:?} - {:?}",
            dbc_addr.xorname(),
            PrettyPrintRecordKey::from(key.clone())
        );

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
        let validated_spends = match self
            .signed_spend_validation(signed_spends, dbc_id, present_locally)
            .await?
        {
            Some(spends) => spends,
            None => {
                // data is already present
                return Ok(CmdOk::DataAlreadyPresent);
            }
        };

        // store the record into the local storage
        let record = Record {
            key,
            value: try_serialize_record(&validated_spends, RecordKind::DbcSpend)?,
            publisher: None,
            expires: None,
        };
        self.network.put_local_record(record).map_err(|_| {
            let err = ProtocolError::SpendNotStored(format!("Cannot PUT Spend with {dbc_addr:?}"));
            error!("Cannot put spend {err:?}");
            err
        })?;

        // Notify the sender of any double spend
        if validated_spends.len() > 1 {
            warn!("Got a double spend for the SpendDbc PUT with dbc_id {dbc_id:?}",);
            let mut proof = validated_spends.iter();
            if let (Some(spend_one), Some(spend_two)) = (proof.next(), proof.next()) {
                return Err(ProtocolError::DoubleSpendAttempt(
                    Box::new(spend_one.to_owned()),
                    Box::new(spend_two.to_owned()),
                ))?;
            }
        }

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Perform validations on the provided `ChunkWithPayment`.
    async fn chunk_payment_validation(
        &self,
        chunk_with_payment: &ChunkWithPayment,
    ) -> Result<(), ProtocolError> {
        let PaymentProof {
            spent_ids,
            audit_trail,
            path,
        } = &chunk_with_payment.payment;

        let addr_name = *chunk_with_payment.chunk.name();

        // We need to fetch the inputs of the DBC tx in order to obtain the root-hash and
        // other info for verifications of valid payment.
        let mut tasks = JoinSet::new();
        spent_ids.iter().cloned().for_each(|dbc_id| {
            trace!(
                "Getting spend {:?} for chunk {:?} to prove payment",
                dbc_id,
                addr_name
            );
            let self_clone = self.clone();
            let _ = tasks.spawn(async move {
                let addr = DbcAddress::from_dbc_id(&dbc_id);
                let signed_spend = self_clone.get_spend_from_network(addr, true).await?;
                Ok::<DbcTransaction, ProtocolError>(signed_spend.spent_tx())
            });
        });

        let mut payment_tx = None;
        while let Some(result) = tasks.join_next().await {
            // TODO: since we are not sending these errors as a response, return sn_node::Error instead.
            let spent_tx = result.map_err(|_| ProtocolError::ChunkNotStored(addr_name))??;
            match payment_tx {
                Some(tx) if spent_tx != tx => {
                    return Err(ProtocolError::PaymentProofTxMismatch(addr_name));
                }
                Some(_) => {}
                None => payment_tx = Some(spent_tx),
            }
        }

        if let Some(tx) = payment_tx {
            let acceptable_fee = self
                .network
                .get_local_storecost()
                .await
                .map_err(|_| ProtocolError::ChunkNotStored(addr_name))?;
            // Check if the fee output id and amount are correct, as well as verify
            // the payment proof corresponds to the fee output and that
            // the fee is sufficient for this chunk.
            verify_fee_output_and_proof(addr_name, acceptable_fee, &tx, audit_trail, path)?;
        } else {
            return Err(ProtocolError::PaymentProofWithoutInputs(addr_name));
        }

        Ok(())
    }

    async fn register_validation(
        &self,
        register: &SignedRegister,
        present_locally: bool,
    ) -> Result<Option<SignedRegister>, ProtocolError> {
        // check if register is valid
        let reg_addr = register.address();
        if let Err(e) = register.verify() {
            error!("Register with addr {reg_addr:?} is invalid: {e:?}");
            return Err(ProtocolError::RegisterInvalid(Box::new(*reg_addr)));
        }

        // if we don't have it locally return it
        if !present_locally {
            debug!("Register with addr {reg_addr:?} is valid and doesn't exist locally");
            return Ok(Some(register.to_owned()));
        }
        debug!("Register with addr {reg_addr:?} exists locally, comparing with local version");

        let key = NetworkAddress::from_register_address(*reg_addr).to_record_key();

        // get local register
        let maybe_record = self.network.get_local_record(&key).await.map_err(|err| {
            warn!("Error while fetching local record {err}");
            ProtocolError::RegisterNotStored(Box::new(*reg_addr))
        })?;
        let record = match maybe_record {
            Some(r) => r,
            None => {
                error!("Register with addr {reg_addr:?} already exists locally, but not found in local storage");
                return Err(ProtocolError::RegisterNotStored(Box::new(*reg_addr)));
            }
        };
        let local_register: SignedRegister = try_deserialize_record(&record)?;

        // merge the two registers
        let mut merged_register = local_register.clone();
        merged_register.verified_merge(register.to_owned())?;
        if merged_register == local_register {
            debug!("Register with addr {reg_addr:?} is the same as the local version");
            Ok(None)
        } else {
            debug!("Register with addr {reg_addr:?} is different from the local version");
            Ok(Some(merged_register))
        }
    }

    /// Perform validations on the provided `Vec<SignedSpend>`. Returns `Some<Vec<SignedSpend>>` if
    /// the spends has to be stored to the `RecordStore`. The resultant spends are aggregated and can
    /// have a max of only 2 elements. Any double spend error has to be thrown by the caller.
    ///
    /// The Vec<SignedSpend> must all have the same dbc_id.
    ///
    /// - If the SignedSpend for the provided DbcId is present locally, check for new spends by
    /// comparing it with the local copy.
    /// - If incoming signed_spends.len() > 1, aggregate store them directly as they are a double spent.
    /// - If incoming signed_spends.len() == 1, then check for parent_inputs and the closest(dbc_id)
    /// for any double spend, which are then aggregated and returned.
    async fn signed_spend_validation(
        &self,
        mut signed_spends: Vec<SignedSpend>,
        dbc_id: DbcId,
        present_locally: bool,
    ) -> Result<Option<Vec<SignedSpend>>, ProtocolError> {
        // get the DbcId; used for validation
        let dbc_addr = DbcAddress::from_dbc_id(&dbc_id);
        let record_key = NetworkAddress::from_dbc_address(dbc_addr).to_record_key();

        if present_locally {
            debug!("DbcSpend with DbcId {dbc_id:?} already exists, checking if it's the same spend/double spend",);
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

            let local_signed_spends: Vec<SignedSpend> = try_deserialize_record(&local_record)?;

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
                debug!(
                    "Seen new spends that are not part of the local copy. Mostly a double spend, checking for it"
                );
                // continue with local_spends + new_ones
                signed_spends = local_signed_spends
                    .into_iter()
                    .chain(newly_seen_spends)
                    .collect();
            }
        }

        // Check the parent spends and check the closest(dbc_id) for any double spend
        // if so aggregate the spends and return just 2 spends.
        let signed_spends = match signed_spends.len() {
            0 => {
                let err = ProtocolError::SpendNotStored("No valid Spend found locally".to_string());
                debug!("No valid spends found locally while validating Spend PUT {err}");

                return Err(err);
            }
            1 => {
                debug!(
                "Received a single SignedSpend, verifying the parent and checking for double spend"
            );
                let signed_spend = match signed_spends.pop() {
                    Some(signed_spends) => signed_spends,
                    None => {
                        return Err(ProtocolError::SpendNotStored(
                            "No valid Spend found".to_string(),
                        ));
                    }
                };

                // check the spend
                if let Err(e) = signed_spend.verify(signed_spend.spent_tx_hash()) {
                    return Err(ProtocolError::SpendSignatureInvalid(format!(
                        "while verifying spend for {:?}: {e:?}",
                        signed_spend.dbc_id()
                    )));
                }

                // If this is a storage payment, then verify FeeOutput's id is the expected.
                verify_fee_output_id(&signed_spend.spent_tx(), false)?;

                // Get parents
                let mut parent_spends = BTreeSet::new();
                if is_genesis_parent_tx(&signed_spend.spend.dbc_creation_tx)
                    && signed_spend.dbc_id() == &GENESIS_DBC.id
                {
                    trace!("GENESIS_DBC {dbc_addr:?} doesn't have a parent");
                } else {
                    for parent_input in &signed_spend.spend.dbc_creation_tx.inputs {
                        let _ = parent_spends.insert(
                            self.get_spend_from_network(
                                DbcAddress::from_dbc_id(&parent_input.dbc_id()),
                                true,
                            )
                            .await?,
                        );
                    }
                }

                // Check parents
                check_parent_spends(&parent_spends, &signed_spend)?;

                // check the network if any spend has happened for the same dbc_id
                // Does not return an error, instead the Vec<SignedSpend> is returned.
                let mut spends =
                    if let Ok(spend) = self.get_spend_from_network(dbc_addr, false).await {
                        vec![spend]
                    } else {
                        vec![]
                    };
                // aggregate the spends from the network with our own
                spends.push(signed_spend);
                aggregate_spends(spends, dbc_id)
            }
            _ => {
                debug!("Received >1 spends with parent. Aggregating the spends to check for double spend. Not performing parent check or querying the network for double spend");
                // if we got 2 or more, then it is a double spend for sure.
                // We don't have to check parent/ ask network for extra spend.
                // Validate and store just 2 of them.
                // The nodes will be synced up during replication.
                aggregate_spends(signed_spends, dbc_id)
            }
        };

        Ok(Some(signed_spends))
    }
}

// If the given TX is a storage payment, i.e. contains a fee output, then verify FeeOutput's id is
// the expected. The fee output id is expected to be built from hashing: root_hash + input DBCs ids.
// This requirement makes it possible for this output to be used as an input in a network
// rewards/farming reclaiming TX, by making its spend location deterministic, analogous to
// how an output DBC Id works for regular outputs.
// If 'required' was set to 'true' then the fee output must be non-zero and valid.
fn verify_fee_output_id(spent_tx: &DbcTransaction, required: bool) -> Result<(), ProtocolError> {
    let fee = &spent_tx.fee;
    debug!("Verification fee: {fee:?}");
    if fee.is_free() {
        if required {
            return Err(ProtocolError::PaymentProofInvalidFeeOutput(fee.id));
        }
    } else {
        let mut fee_id_bytes = fee.root_hash.slice().to_vec();
        spent_tx
            .inputs
            .iter()
            .for_each(|input| fee_id_bytes.extend(&input.dbc_id().to_bytes()));

        if fee.id != Hash::hash(&fee_id_bytes) {
            return Err(ProtocolError::PaymentProofInvalidFeeOutput(fee.id));
        }
    }

    Ok(())
}

// Check if the fee output id and amount are correct, as well as verify the payment proof audit
// trail info corresponds to the fee output, i.e. the fee output's root-hash is derived from
// the proof's audit trail info.
fn verify_fee_output_and_proof(
    addr_name: XorName,
    acceptable_fee: Token,
    tx: &DbcTransaction,
    audit_trail: &[MerkleTreeNodesType],
    path: &[usize],
) -> Result<(), ProtocolError> {
    // Check if the fee output id is correct
    verify_fee_output_id(tx, true)?;

    // TODO: was there a reason we used leaf index here for payment calc?
    // Check the root hash verifies the merkle-tree audit trail and path against the content address name
    let _leaf_index = validate_payment_proof(addr_name, &tx.fee.root_hash, audit_trail, path)
        .map_err(|err| ProtocolError::InvalidPaymentProof {
            addr_name,
            reason: err.to_string(),
        })?;

    // Check the expected amount of tokens was paid by the Tx, i.e. the amount of
    // the fee output the expected `acceptable_fee` nano per Chunk/address.
    let paid = tx.fee.token;
    if paid <= acceptable_fee {
        // the payment amount is not enough, we expect `acceptable_fee` nanos per adddress
        return Err(ProtocolError::PaymentProofInsufficientAmount {
            paid,
            expected: acceptable_fee,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use sn_dbc::{FeeOutput, Token};
    use sn_transfers::payment_proof::build_payment_proofs;

    proptest! {
        #[test]
        fn test_verify_payment_proof(num_of_addrs in 1..1000) {
            let mut rng = rand::thread_rng();
            let random_names = (0..num_of_addrs).map(|_| XorName::random(&mut rng)).collect::<Vec<_>>();
            let (root_hash, proofs) = build_payment_proofs(random_names.iter())?;
            let total_cost = num_of_addrs as u64;

            for (leaf_index, name) in random_names.into_iter().enumerate() {
                // TODO: populate with random inputs to make the test more complete.
                let mut tx = DbcTransaction {
                    inputs: vec![],
                    outputs: vec![],
                    fee: FeeOutput {
                        id: Hash::hash(root_hash.slice()),
                        token: Token::from_nano(total_cost),
                        root_hash,
                    },
                };

                let (audit_trail, path) = proofs.get(&name).expect(
                    "Failed to obtain payment proof for test content address at index #{leaf_index}"
                );

                // verification should pass since we provide the correct audit trail info
                assert!(matches!(
                    verify_fee_output_and_proof(name, &tx, audit_trail, path),
                    Ok(())
                ));

                // verification should fail if we pass invalid payment proof audit trail or path
                assert!(matches!(
                    verify_fee_output_and_proof(name, &tx, &[], path),
                    Err(ProtocolError::InvalidPaymentProof {addr_name, ..}) if addr_name == name
                ));
                assert!(matches!(
                    verify_fee_output_and_proof(name, &tx, audit_trail, &[]),
                    Err(ProtocolError::InvalidPaymentProof {addr_name, ..}) if addr_name == name
                ));

                // verification should fail if the amount paid is not enough for the content
                tx.fee.token = Token::from_nano(leaf_index as u64); // it should fail with an amount less or equal to this value
                assert!(matches!(
                    verify_fee_output_and_proof(name, &tx, audit_trail, path),
                    Err(ProtocolError::PaymentProofInsufficientAmount { paid, expected })
                        if paid == leaf_index && expected == leaf_index + 1
                ));

                // verification should pass if the amount is more than enough for the content
                tx.fee.token = Token::from_nano(total_cost + 1);
                assert!(matches!(
                    verify_fee_output_and_proof(name, &tx, audit_trail, path),
                    Ok(())
                ));

                // test that verification fails when the fee output id is incorrect
                let invalid_fee_id = [123; 32].into();
                tx.fee.id = invalid_fee_id;
                assert!(matches!(
                    verify_fee_output_and_proof(name, &tx, audit_trail, &[]),
                    Err(err) if err == ProtocolError::PaymentProofInvalidFeeOutput(invalid_fee_id)));

            }
        }
    }
}
