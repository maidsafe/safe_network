// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    spends::{aggregate_spends, check_parent_spends},
    Marker, Node,
};
use libp2p::kad::{Record, RecordKey};
use sn_dbc::{Dbc, DbcId, DbcTransaction, SignedSpend, Token};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::CmdOk,
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, DbcAddress, RecordHeader, RecordKind,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;
use sn_transfers::{
    dbc_genesis::{is_genesis_parent_tx, GENESIS_DBC},
    wallet::LocalWallet,
};
use std::collections::{BTreeSet, HashSet};
use tokio::task::JoinSet;

impl Node {
    /// Validate a record and it's payment, and store the record to the RecordStore
    pub(crate) async fn validate_and_store_record(
        &self,
        record: Record,
    ) -> Result<CmdOk, ProtocolError> {
        let record_header = RecordHeader::from_record(&record)?;

        match record_header.kind {
            RecordKind::ChunkWithPayment => {
                let record_key = record.key.clone();
                let (payment, chunk) = try_deserialize_record::<(Vec<Dbc>, Chunk)>(&record)?;
                let already_exists = self
                    .validate_key_and_existence(&chunk.network_address(), &record_key)
                    .await?;

                if already_exists {
                    return Ok(CmdOk::DataAlreadyPresent);
                }

                // Validate the payment and that we received what we asked.
                self.payment_for_us_exists_and_is_still_valid(&chunk.network_address(), &payment)
                    .await?;

                self.store_chunk(chunk)
            }
            RecordKind::Chunk => {
                error!("Chunk should not be validated at this point");
                Err(ProtocolError::InvalidPutWithoutPayment(
                    PrettyPrintRecordKey::from(record.key),
                ))
            }
            RecordKind::DbcSpend => self.validate_spend_record(record).await,
            RecordKind::Register => {
                let register: SignedRegister = try_deserialize_record::<SignedRegister>(&record)?;

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
            RecordKind::RegisterWithPayment => {
                warn!("RegisterWith`Payment recieved but we cannot handle it yet!");
                Ok(CmdOk::StoredSuccessfully)
                // DO nothing yet
            }
        }
    }

    /// Perform all validations required on a SpendRequest entry.
    /// This applies for PUT and replication
    async fn validate_spend_record(&self, record: Record) -> Result<CmdOk, ProtocolError> {
        let record_key = record.key.clone();
        let spends = try_deserialize_record::<Vec<SignedSpend>>(&record)?;

        for spend in &spends {
            let dbc_addr = DbcAddress::from_dbc_id(spend.dbc_id());
            let address = NetworkAddress::DbcAddress(dbc_addr);

            // if it already exists, we still have to check if its a double spend or no, so we can ignore the result here
            let _exists = self
                .validate_key_and_existence(&address, &record_key)
                .await?;
        }

        self.validate_and_store_spends(spends).await
    }

    /// Store a prevalidated, and already paid record to the RecordStore
    pub(crate) async fn store_prepaid_record(
        &self,
        record: Record,
    ) -> Result<CmdOk, ProtocolError> {
        let record_header = RecordHeader::from_record(&record)?;

        match record_header.kind {
            RecordKind::ChunkWithPayment | RecordKind::RegisterWithPayment => Err(
                ProtocolError::UnexpectedRecordWithPayment(PrettyPrintRecordKey::from(record.key)),
            ),
            RecordKind::Chunk => {
                let chunk = try_deserialize_record::<Chunk>(&record)?;

                let record_key = record.key.clone();
                let already_exists = self
                    .validate_key_and_existence(&chunk.network_address(), &record_key)
                    .await?;

                if already_exists {
                    return Ok(CmdOk::DataAlreadyPresent);
                }

                self.store_chunk(chunk)
            }
            RecordKind::DbcSpend => self.validate_spend_record(record).await,
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

    /// Check key is valid compared to the network name, and if we already have this data or not.
    /// returns true if data already exists locally
    async fn validate_key_and_existence(
        &self,
        address: &NetworkAddress,
        expected_record_key: &RecordKey,
    ) -> Result<bool, ProtocolError> {
        let data_key = address.to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(data_key.clone());

        if expected_record_key != &data_key {
            warn!(
                "record key: {:?}, key: {:?}",
                PrettyPrintRecordKey::from(expected_record_key.clone()),
                pretty_key
            );
            warn!("Record's key does not match with the value's address, ignoring PUT.");
            return Err(ProtocolError::RecordKeyMismatch);
        }

        let present_locally = self
            .network
            .is_key_present_locally(&data_key)
            .await
            .map_err(|err| {
                warn!("Error while checking if Chunk's key is present locally {err}");
                ProtocolError::RecordNotStored(pretty_key)
            })?;

        if present_locally {
            // We may short circuit if the Record::key is present locally;
            debug!(
                "Record with addr {:?} already exists, not overwriting",
                address
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Store a `ChunkWithPayment` to the RecordStore
    pub(crate) fn store_chunk(&self, chunk: Chunk) -> Result<CmdOk, ProtocolError> {
        let chunk_name = *chunk.name();
        let chunk_addr = *chunk.address();
        debug!("storing chunk {chunk_name:?}");

        let key = NetworkAddress::from_chunk_address(*chunk.address()).to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(key.clone());

        let record = Record {
            key,
            value: try_serialize_record(&chunk, RecordKind::Chunk)?,
            publisher: None,
            expires: None,
        };

        // finally store the Record directly into the local storage
        debug!("Storing chunk {chunk_name:?} as Record locally");
        self.network.put_local_record(record).map_err(|err| {
            Marker::RecordRejected(&pretty_key).log();

            warn!("Error while locally storing Chunk as a Record{err}");
            ProtocolError::RecordNotStored(pretty_key.clone())
        })?;

        Marker::ValidChunkRecordPutFromNetwork(&pretty_key).log();

        self.events_channel
            .broadcast(crate::NodeEvent::ChunkStored(chunk_addr));

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

        let pretty_key = PrettyPrintRecordKey::from(key.clone());

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
            Marker::RecordRejected(&pretty_key).log();
            warn!("Error while locally storing register as a Record {err}");
            ProtocolError::RegisterNotStored(Box::new(*reg_addr))
        })?;

        Marker::ValidRegisterRecordPutFromNetwork(&pretty_key).log();

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
        let pretty_key = PrettyPrintRecordKey::from(key.clone());
        debug!(
            "validating and storing spends {:?} - {:?}",
            dbc_addr.xorname(),
            pretty_key
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
            .signed_spend_validation(signed_spends.clone(), dbc_id, present_locally)
            .await?
        {
            Some(spends) => spends,
            None => {
                // we trust the replicated data
                debug!(
                    "Trust replicated spend for {:?}",
                    PrettyPrintRecordKey::from(key.clone())
                );
                // TODO: may need to tweak the `signed_spend_validation` function,
                //       instead of trusting replicated spend directly
                signed_spends
            }
        };

        debug!(
            "Got {} validated spends for {:?}",
            validated_spends.len(),
            PrettyPrintRecordKey::from(key.clone())
        );

        // store the record into the local storage
        let record = Record {
            key,
            value: try_serialize_record(&validated_spends, RecordKind::DbcSpend)?,
            publisher: None,
            expires: None,
        };
        self.network.put_local_record(record).map_err(|_| {
            Marker::RecordRejected(&pretty_key).log();

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

        Marker::ValidSpendRecordPutFromNetwork(&pretty_key).log();

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Perform validations on the provided `Record`.
    async fn payment_for_us_exists_and_is_still_valid(
        &self,
        address: &NetworkAddress,
        created_dbcs: &[Dbc],
    ) -> Result<(), ProtocolError> {
        debug!("Validating record payment for {address:?}");

        let pretty_key = PrettyPrintRecordKey::from(address.to_record_key());

        // We need to fetch the inputs of the DBC tx in order to obtain the root-hash and
        // other info for verifications of valid payment.
        let mut tasks = JoinSet::new();

        let mut wallet = LocalWallet::load_from(&self.network.root_dir_path)
            .map_err(|err| ProtocolError::FailedToStorePaymentIntoNodeWallet(err.to_string()))?;

        info!("Checking if payment is sufficient for {pretty_key:?}");
        let current_store_cost = self
            .network
            .get_local_storecost()
            .await
            .map_err(|_| ProtocolError::RecordNotStored(pretty_key.clone()))?;

        for dbc in created_dbcs.iter() {
            let spent_dbc_ids = dbc
                .signed_spends
                .iter()
                .map(|s| *s.dbc_id())
                .collect::<Vec<_>>();

            // let dbc_target = dbc.ciphers.derivation_index_cipher;
            let dbc_target = dbc.ciphers.public_address;
            let dbc_is_for_us = dbc_target == self.reward_address;

            if dbc_is_for_us {
                trace!("Payment proof for record {pretty_key:?} is for us");

                // lets record how much we've apparently been paid
                let we_seem_to_have_been_paid = dbc
                    .token()
                    .map_err(|_| ProtocolError::RecordNotStored(pretty_key.clone()))?;

                // lets deposit the money first
                wallet.deposit(&vec![dbc.clone()]).map_err(|err| {
                    ProtocolError::FailedToStorePaymentIntoNodeWallet(err.to_string())
                })?;

                let tolerable_fee = tolerable_fee(current_store_cost);
                // we can bail early here and not bother checking payment validity.
                // we've depositied it, so if it's valid, that's fine.
                // if it's underpayment, we're bailing anyway, so we don't care.
                if we_seem_to_have_been_paid < tolerable_fee {
                    return Err(ProtocolError::PaymentProofInsufficientAmount {
                        paid: we_seem_to_have_been_paid,
                        expected: tolerable_fee,
                    });
                }

                // assuming we have been paid enough, lets get and validate the actualy txs
                trace!(
                    "Getting spends {:?} for payment of record {pretty_key:?} for validation",
                    spent_dbc_ids
                );

                for dbc_id in spent_dbc_ids {
                    let self_clone = self.clone();
                    let _ = tasks.spawn(async move {
                        let addr = DbcAddress::from_dbc_id(&dbc_id);
                        let signed_spend = self_clone.get_spend_from_network(addr, true).await?;
                        Ok::<DbcTransaction, ProtocolError>(signed_spend.spent_tx())
                    });
                }
            }
        }

        if tasks.is_empty() {
            warn!("No payment for us for record {pretty_key:?}");
        }

        wallet
            .store()
            .map_err(|err| ProtocolError::FailedToStorePaymentIntoNodeWallet(err.to_string()))?;

        // Then we verify the tx
        let mut payment_tx = None;

        // Check the spent transactions for our payment proof for double spend
        // error out if there's a mismatch over the tx
        while let Some(result) = tasks.join_next().await {
            // TODO: since we are not sending these errors as a response, return sn_node::Error instead.
            let spent_tx =
                result.map_err(|_| ProtocolError::RecordNotStored(pretty_key.clone()))??;
            match payment_tx {
                Some(tx) if spent_tx != tx => {
                    return Err(ProtocolError::PaymentProofTxMismatch(pretty_key));
                }
                Some(_) => {}
                None => payment_tx = Some(spent_tx),
            }
        }

        // There is a payment for us, lets validate it is actually enough
        if let Some(tx) = payment_tx {
            // Check if any of the dbcs sent are sufficient for this chunk.
            match verify_fee_is_sufficient(current_store_cost, &tx) {
                Ok(_) => {}
                Err(ProtocolError::PaymentProofInsufficientAmount { paid, expected }) => {
                    return Err(ProtocolError::PaymentProofInsufficientAmount { paid, expected });
                }
                Err(error) => {
                    return Err(error);
                }
            }
        } else {
            // There is no DBC for us, so we dont store it.
            return Err(ProtocolError::NoPaymentToOurNode(pretty_key));
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
        debug!("Validating and storing spend {dbc_addr:?}, present_locally: {present_locally}");

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
                    let err = Err(ProtocolError::SpendSignatureInvalid(format!(
                        "while verifying spend for {:?}: {e:?}",
                        signed_spend.dbc_id()
                    )));
                    error!("Error while verifying signed spend signature {err:?}");
                    return err;
                }

                // Get parents
                let mut parent_spends = BTreeSet::new();
                if is_genesis_parent_tx(&signed_spend.spend.dbc_creation_tx)
                    && signed_spend.dbc_id() == &GENESIS_DBC.id
                {
                    trace!("GENESIS_DBC {dbc_addr:?} doesn't have a parent");
                } else {
                    trace!(
                        "Checking dbc {dbc_addr:?} parent transaction {:?}",
                        signed_spend.spend.dbc_creation_tx
                    );
                    for parent_input in &signed_spend.spend.dbc_creation_tx.inputs {
                        let parent_dbc_address = DbcAddress::from_dbc_id(&parent_input.dbc_id());
                        trace!(
                            "Checking parent input at {:?} - {parent_dbc_address:?}",
                            parent_input.dbc_id(),
                        );
                        let parent = self
                            .get_spend_from_network(parent_dbc_address, false)
                            .await?;
                        trace!(
                            "Got parent input at {:?} - {parent_dbc_address:?}",
                            parent_input.dbc_id(),
                        );
                        let _ = parent_spends.insert(parent);
                    }
                }

                // Check parents
                if let Err(err) = check_parent_spends(&parent_spends, &signed_spend) {
                    error!("Error while checking parent spends {err:?}");
                    return Err(err);
                }

                // check the network if any spend has happened for the same dbc_id
                // Does not return an error, instead the Vec<SignedSpend> is returned.
                debug!("Check if any spend exist for the same dbc_id {dbc_addr:?}");
                let mut spends = match self.get_spend_from_network(dbc_addr, false).await {
                    Ok(spend) => {
                        debug!("Got spend from network for the same dbc_id");
                        vec![spend]
                    }
                    // Q: Should we not aggregate the double spends instead of using vec![]
                    Err(err) => {
                        debug!("Got error while fetching spend for the same dbc_id {err:?}");
                        vec![]
                    }
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

// Find a tolerable fee. This should help prevent us rejecting close, but currently
// insufficient payments
fn tolerable_fee(current_store_cost: Token) -> Token {
    Token::from_nano(current_store_cost.as_nano() / 2)
}

// Check if the fee output id and amount are correct, as well as verify the payment proof audit
// trail info corresponds to the fee output, i.e. the fee output's root-hash is derived from
// the proof's audit trail info.
fn verify_fee_is_sufficient(
    current_store_cost: Token,
    tx: &DbcTransaction,
) -> Result<(), ProtocolError> {
    // TODO: properly verify which one of these was for this node, and check _that_
    // against our store acceptable fee

    let mut highest_fee = Token::zero();
    // Check the expected amount of tokens was paid by the Tx, i.e. the amount of
    // the fee output the expected `acceptable_fee` nano per record.
    for output in tx.outputs.iter() {
        // TODO: Is an output
        if output.token > highest_fee {
            highest_fee = output.token;
        }
    }

    // We expect at least the current step or one down. This should smooth over any
    // issues that might arise with payment going up and down.
    let expected = tolerable_fee(current_store_cost);

    if highest_fee < current_store_cost {
        return Err(ProtocolError::PaymentProofInsufficientAmount {
            paid: highest_fee,
            expected,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use sn_dbc::{FeeOutput, Output, Token};
    use xor_name::XorName;

    proptest! {
        #[test]
        fn test_verify_record_payment(num_of_addrs in 1..1000, store_costs in 1..100_u64 ) {
            let mut rng = rand::thread_rng();
            let mut tx_payments = vec![];

            let mut lowest_fee = Token::zero();
            for cost in 0..store_costs {
                if cost < lowest_fee.as_nano() {
                    lowest_fee = Token::from_nano(cost);
                }

                let network_store_cost = Token::from_nano(cost);

                tx_payments.push(Output{
                    dbc_id: sn_dbc::DbcId::new(bls::SecretKey::random().public_key()),
                    token: network_store_cost,
                });
            }

            let random_names = (0..num_of_addrs).map(|_| XorName::random(&mut rng)).collect::<Vec<_>>();

            for _name in random_names.into_iter() {
                let tx = DbcTransaction {
                    inputs: vec![],
                    outputs: tx_payments.clone(),
                    // TODO: Clean this up when we remove FeeOutput
                    fee: FeeOutput {
                        id: sn_dbc::Hash::hash(b"id"),
                        token: Token::zero(),
                        root_hash: sn_dbc::Hash::hash(b"root"),
                    },
                };

                // verification should fail if the amount paid is not enough for the content
                // TODO: sort out what is acceptable based on the network store cost
                // more properly
                let _res = verify_fee_is_sufficient( lowest_fee, &tx);

            }
        }
    }
}
