// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    node::Node,
    node::TRANSFER_NOTIF_TOPIC,
    spends::{aggregate_spends, check_parent_spends},
    Marker,
};
use bytes::BytesMut;
use libp2p::kad::{Record, RecordKey};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::CmdOk,
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, RecordHeader, RecordKind, SpendAddress,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;
use sn_transfers::{
    is_genesis_parent_tx, LocalWallet, Transfer, GENESIS_CASHNOTE, NETWORK_ROYALTIES_PK,
};
use sn_transfers::{
    CashNote, NanoTokens, SignedSpend, UniquePubkey, NETWORK_ROYALTIES_AMOUNT_PER_ADDR,
};
use std::collections::{BTreeSet, HashSet};

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
                let (payment, chunk) = try_deserialize_record::<(Vec<Transfer>, Chunk)>(&record)?;
                let already_exists = self
                    .validate_key_and_existence(&chunk.network_address(), &record_key)
                    .await?;

                // Validate the payment and that we received what we asked.
                // This stores any payments to disk
                let payment_res = self
                    .payment_for_us_exists_and_is_still_valid(&chunk.network_address(), payment)
                    .await;

                // Now that we've taken any money passed to us, regardless of the payment's validity,
                // if we already have the data we can return early
                if already_exists {
                    return Ok(CmdOk::DataAlreadyPresent);
                }

                // Finally before we store, lets bail for any payment issues
                payment_res?;

                self.store_chunk(chunk)
            }
            RecordKind::Chunk => {
                error!("Chunk should not be validated at this point");
                Err(ProtocolError::InvalidPutWithoutPayment(
                    PrettyPrintRecordKey::from(&record.key).into_owned(),
                ))
            }
            RecordKind::Spend => self.validate_spend_record(record).await,
            RecordKind::Register => {
                error!("Register should not be validated at this point");
                Err(ProtocolError::InvalidPutWithoutPayment(
                    PrettyPrintRecordKey::from(&record.key).into_owned(),
                ))
            }
            RecordKind::RegisterWithPayment => {
                let (payment, register) =
                    try_deserialize_record::<(Vec<Transfer>, SignedRegister)>(&record)?;

                // check if the deserialized value's RegisterAddress matches the record's key
                let net_addr = NetworkAddress::from_register_address(*register.address());
                let key = net_addr.to_record_key();
                if record.key != key {
                    warn!(
                        "Record's key does not match with the value's RegisterAddress, ignoring PUT."
                    );
                    return Err(ProtocolError::RecordKeyMismatch);
                }

                let already_exists = self.validate_key_and_existence(&net_addr, &key).await?;

                if !already_exists {
                    // Validate the payment and that we received what we asked.
                    self.payment_for_us_exists_and_is_still_valid(&net_addr, payment)
                        .await?;
                }

                self.validate_and_store_register(register).await
            }
        }
    }

    /// Perform all validations required on a SpendRequest entry.
    /// This applies for PUT and replication
    async fn validate_spend_record(&self, record: Record) -> Result<CmdOk, ProtocolError> {
        let record_key = record.key.clone();
        let spends = try_deserialize_record::<Vec<SignedSpend>>(&record)?;

        for spend in &spends {
            let cash_note_addr = SpendAddress::from_unique_pubkey(spend.unique_pubkey());
            let address = NetworkAddress::SpendAddress(cash_note_addr);

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
            RecordKind::ChunkWithPayment | RecordKind::RegisterWithPayment => {
                Err(ProtocolError::UnexpectedRecordWithPayment(
                    PrettyPrintRecordKey::from(&record.key).into_owned(),
                ))
            }
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
            RecordKind::Spend => self.validate_spend_record(record).await,
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
        let pretty_key = PrettyPrintRecordKey::from(&data_key);

        if expected_record_key != &data_key {
            warn!(
                "record key: {:?}, key: {:?}",
                PrettyPrintRecordKey::from(expected_record_key),
                pretty_key
            );
            warn!("Record's key does not match with the value's address, ignoring PUT.");
            return Err(ProtocolError::RecordKeyMismatch);
        }

        let present_locally = self
            .network
            .is_record_key_present_locally(&data_key)
            .await
            .map_err(|err| {
                let msg = format!("Error while checking if Chunk's key is present locally {err}");
                warn!("{msg}");
                ProtocolError::RecordNotStored(pretty_key.into_owned(), msg)
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

        let key = NetworkAddress::from_chunk_address(*chunk.address()).to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(&key).into_owned();

        let record = Record {
            key,
            value: try_serialize_record(&chunk, RecordKind::Chunk)?,
            publisher: None,
            expires: None,
        };

        // finally store the Record directly into the local storage
        debug!("Storing chunk {chunk_name:?} as Record locally");
        self.network.put_local_record(record).map_err(|err| {
            let msg = format!("Error while locally storing Chunk as a Record: {err}");
            warn!("{msg}");
            ProtocolError::RecordNotStored(pretty_key.clone(), msg)
        })?;

        self.record_metrics(Marker::ValidChunkRecordPutFromNetwork(&pretty_key));

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
            .is_record_key_present_locally(&key)
            .await
            .map_err(|err| {
                warn!("Error while checking if register's key is present locally {err}");
                ProtocolError::RegisterNotStored(Box::new(*reg_addr))
            })?;

        let pretty_key = PrettyPrintRecordKey::from(&key);

        // check register and merge if needed
        let updated_register = match self.register_validation(&register, present_locally).await? {
            Some(reg) => reg,
            None => {
                return Ok(CmdOk::DataAlreadyPresent);
            }
        };

        // store in kad
        let record = Record {
            key: key.clone(),
            value: try_serialize_record(&updated_register, RecordKind::Register)?,
            publisher: None,
            expires: None,
        };
        debug!("Storing register {reg_addr:?} as Record locally");
        self.network.put_local_record(record).map_err(|err| {
            warn!("Error while locally storing register as a Record {err}");
            ProtocolError::RegisterNotStored(Box::new(*reg_addr))
        })?;

        self.record_metrics(Marker::ValidRegisterRecordPutFromNetwork(&pretty_key));

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store `Vec<SignedSpend>` to the RecordStore
    pub(crate) async fn validate_and_store_spends(
        &self,
        signed_spends: Vec<SignedSpend>,
    ) -> Result<CmdOk, ProtocolError> {
        // make sure that the unique_pubkeys match
        let unique_pubkey = if let Some((first, elements)) = signed_spends.split_first() {
            let common_unique_pubkey = *first.unique_pubkey();
            if elements
                .iter()
                .all(|spend| spend.unique_pubkey() == &common_unique_pubkey)
            {
                common_unique_pubkey
            } else {
                let err = Err(ProtocolError::SpendNotStored(
                    "found SignedSpends with differing unique_pubkeys".to_string(),
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
        let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);

        let key = NetworkAddress::from_cash_note_address(cash_note_addr).to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(&key);
        debug!(
            "validating and storing spends {:?} - {pretty_key:?}",
            cash_note_addr.xorname(),
        );

        let present_locally = self
            .network
            .is_record_key_present_locally(&key)
            .await
            .map_err(|_err| {
                let err = ProtocolError::SpendNotStored(format!(
                    "Error while checking if Spend's key was present locally, {cash_note_addr:?}"
                ));
                warn!("{err:?}");
                err
            })?;

        // validate the signed spends against the network and the local copy
        let validated_spends = match self
            .signed_spend_validation(signed_spends.clone(), unique_pubkey, present_locally)
            .await?
        {
            Some(spends) => spends,
            None => {
                // we trust the replicated data
                debug!("Trust replicated spend for {pretty_key:?}",);
                // TODO: may need to tweak the `signed_spend_validation` function,
                //       instead of trusting replicated spend directly
                signed_spends
            }
        };

        debug!(
            "Got {} validated spends for {pretty_key:?}",
            validated_spends.len(),
        );

        // store the record into the local storage
        let record = Record {
            key: key.clone(),
            value: try_serialize_record(&validated_spends, RecordKind::Spend)?,
            publisher: None,
            expires: None,
        };
        self.network.put_local_record(record).map_err(|_| {
            let err =
                ProtocolError::SpendNotStored(format!("Cannot PUT Spend with {cash_note_addr:?}"));
            error!("Cannot put spend {err:?}");
            err
        })?;

        // Notify the sender of any double spend
        if validated_spends.len() > 1 {
            warn!(
                "Got a double spend for the SpendCashNote PUT with unique_pubkey {unique_pubkey:?}",
            );
            let mut proof = validated_spends.iter();
            if let (Some(spend_one), Some(spend_two)) = (proof.next(), proof.next()) {
                return Err(ProtocolError::DoubleSpendAttempt(
                    Box::new(spend_one.to_owned()),
                    Box::new(spend_two.to_owned()),
                ))?;
            }
        }

        self.record_metrics(Marker::ValidSpendRecordPutFromNetwork(&pretty_key));

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Gets CashNotes out of a Payment, this includes network verifications of the Transfer.
    /// Return the CashNotes corresponding to our wallet and the trasfers corresponding to network royalties payment.
    async fn cash_notes_from_payment(
        &self,
        payment: &Vec<Transfer>,
        wallet: &LocalWallet,
        pretty_key: PrettyPrintRecordKey<'static>,
    ) -> Result<(NanoTokens, Vec<CashNote>, Vec<Transfer>), ProtocolError> {
        let royalties_pk = *NETWORK_ROYALTIES_PK;
        let mut cash_notes = vec![];
        let mut royalties_transfers = vec![];
        let mut received_fee = NanoTokens::zero();

        for transfer in payment {
            match transfer {
                Transfer::Encrypted(_) => match self
                    .network
                    .verify_and_unpack_transfer(transfer, wallet)
                    .await
                {
                    // transfer not for us
                    Err(ProtocolError::FailedToDecypherTransfer) => continue,
                    // transfer invalid
                    Err(e) => return Err(e),
                    // transfer ok
                    Ok(cns) => cash_notes = cns,
                },
                Transfer::NetworkRoyalties(cashnote_redemptions) => {
                    if let Ok(cash_notes) = self
                        .network
                        .verify_cash_notes_redemptions(royalties_pk, cashnote_redemptions)
                        .await
                    {
                        let received_royalties =
                            total_cash_notes_amount(&cash_notes, pretty_key.clone())?;
                        trace!(
                            "{} network royalties payment cash notes found for record {pretty_key} for a total value of {received_royalties:?}",
                            cash_notes.len()
                        );
                        let encrypted_cashnote_redemptions = cash_notes
                            .into_iter()
                            .map(Transfer::transfers_from_cash_note)
                            .collect::<Result<Vec<Transfer>, sn_transfers::Error>>()
                            .map_err(|err| {
                                error!("Error generating royalty transfer: {err:?}");
                                ProtocolError::FailedToEncryptTransfer
                            })?;

                        royalties_transfers.extend(encrypted_cashnote_redemptions);
                        received_fee = received_fee
                            .checked_add(received_royalties)
                            .ok_or_else(|| ProtocolError::PaymentExceedsTotalTokens)?;
                    }
                }
            }
        }

        if cash_notes.is_empty() {
            Err(ProtocolError::NoPaymentToOurNode(pretty_key))
        } else {
            let received_fee_to_our_node =
                total_cash_notes_amount(&cash_notes, pretty_key.clone())?;
            info!(
                "{} cash note/s (for a total of {received_fee_to_our_node:?}) are for us for {pretty_key}",
                cash_notes.len()
            );
            received_fee = received_fee
                .checked_add(received_fee_to_our_node)
                .ok_or_else(|| ProtocolError::PaymentExceedsTotalTokens)?;

            Ok((received_fee, cash_notes, royalties_transfers))
        }
    }

    /// Perform validations on the provided `Record`.
    async fn payment_for_us_exists_and_is_still_valid(
        &self,
        address: &NetworkAddress,
        payment: Vec<Transfer>,
    ) -> Result<(), ProtocolError> {
        let key = address.to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(&key).into_owned();
        trace!("Validating record payment for {pretty_key}");

        // load wallet
        let mut wallet = LocalWallet::load_from(&self.network.root_dir_path)
            .map_err(|err| ProtocolError::FailedToStorePaymentIntoNodeWallet(err.to_string()))?;

        // unpack transfer
        trace!("Unpacking incoming Transfers for record {pretty_key}");
        let (received_fee, cash_notes, royalties_transfers) = self
            .cash_notes_from_payment(&payment, &wallet, pretty_key.clone())
            .await?;

        trace!("Received payment of {received_fee:?} for {pretty_key}");

        // deposit the CashNotes in our wallet
        wallet
            .deposit_and_store_to_disk(&cash_notes)
            .map_err(|err| ProtocolError::FailedToStorePaymentIntoNodeWallet(err.to_string()))?;
        #[cfg(feature = "open-metrics")]
        let _ = self
            .node_metrics
            .reward_wallet_balance
            .set(wallet.balance().as_nano() as i64);

        if royalties_transfers.is_empty() {
            return Err(ProtocolError::NoNetworkRoyaltiesPayment(
                pretty_key.into_owned(),
            ));
        }

        // publish a notification over gossipsub topic TRANSFER_NOTIF_TOPIC for the network royalties payment.
        match bincode::serialize(&royalties_transfers) {
            Ok(serialised) => {
                let royalties_pk = *NETWORK_ROYALTIES_PK;
                trace!("Publishing a royalties transfer notification over gossipsub for record {pretty_key} and beneficiary {royalties_pk:?}");
                let topic = TRANSFER_NOTIF_TOPIC.to_string();
                let royalties_pk_bytes = royalties_pk.to_bytes();
                let mut msg = BytesMut::with_capacity(royalties_pk_bytes.len() + serialised.len());
                msg.extend_from_slice(&royalties_pk_bytes);
                msg.extend(serialised);

                let msg = msg.freeze();
                if let Err(err) = self.network.publish_on_topic(topic.clone(), msg) {
                    debug!("Failed to publish a network royalties payment notification over gossipsub for record {pretty_key} and beneficiary {royalties_pk:?}: {err:?}");
                }
            }
            Err(err) => debug!("Failed to serialise network royalties payment data to publish a notification over gossipsub for record {pretty_key}: {err:?}"),
        }

        // check payment is sufficient both for our store cost and for network royalties
        let expected_fee = self
            .network
            .get_local_storecost()
            .await
            .map_err(|e| ProtocolError::RecordNotStored(pretty_key.clone(), format!("{e:?}")))?
            .checked_add(NETWORK_ROYALTIES_AMOUNT_PER_ADDR)
            .ok_or(ProtocolError::RecordNotStored(
                pretty_key.clone(),
                "CashNote value overflow".to_string(),
            ))?;
        // finally, (after we accept any payments to us as they are ours now anyway)
        // lets check they actually paid enough
        if received_fee < expected_fee {
            trace!("Payment insufficient for record {pretty_key}. {received_fee:?} is less than {expected_fee:?}");
            return Err(ProtocolError::PaymentProofInsufficientAmount {
                paid: received_fee,
                expected: expected_fee,
            });
        }
        info!("Total payment of {received_fee:?} nanos accepted for record {pretty_key}");

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
        trace!("Register with addr {reg_addr:?} exists locally, comparing with local version");

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
            trace!("Register with addr {reg_addr:?} is the same as the local version");
            Ok(None)
        } else {
            trace!("Register with addr {reg_addr:?} is different from the local version");
            Ok(Some(merged_register))
        }
    }

    /// Perform validations on the provided `Vec<SignedSpend>`. Returns `Some<Vec<SignedSpend>>` if
    /// the spends has to be stored to the `RecordStore`. The resultant spends are aggregated and can
    /// have a max of only 2 elements. Any double spend error has to be thrown by the caller.
    ///
    /// The Vec<SignedSpend> must all have the same unique_pubkey.
    ///
    /// - If the SignedSpend for the provided UniquePubkey is present locally, check for new spends by
    /// comparing it with the local copy.
    /// - If incoming signed_spends.len() > 1, aggregate store them directly as they are a double spent.
    /// - If incoming signed_spends.len() == 1, then check for parent_inputs and the closest(unique_pubkey)
    /// for any double spend, which are then aggregated and returned.
    async fn signed_spend_validation(
        &self,
        mut signed_spends: Vec<SignedSpend>,
        unique_pubkey: UniquePubkey,
        present_locally: bool,
    ) -> Result<Option<Vec<SignedSpend>>, ProtocolError> {
        // get the UniquePubkey; used for validation
        let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);
        let record_key = NetworkAddress::from_cash_note_address(cash_note_addr).to_record_key();
        debug!(
            "Validating and storing spend {cash_note_addr:?}, present_locally: {present_locally}"
        );

        if present_locally {
            debug!("Spend with UniquePubkey {unique_pubkey:?} already exists, checking if it's the same spend/double spend",);
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
                    return Err(ProtocolError::SpendNotFound(cash_note_addr));
                }
            };

            let local_header = RecordHeader::from_record(&local_record)?;
            // Make sure the local copy is of the same kind
            if !matches!(local_header.kind, RecordKind::Spend) {
                error!(
                    "Expected CashNoteRecord kind, found {:?}",
                    local_header.kind
                );
                return Err(ProtocolError::RecordKindMismatch(RecordKind::Spend));
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
                debug!("Vec<SignedSpend> with addr {cash_note_addr:?} already exists, not overwriting!",);
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

        // Check the parent spends and check the closest(unique_pubkey) for any double spend
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
                        signed_spend.unique_pubkey()
                    )));
                    error!("Error while verifying signed spend signature {err:?}");
                    return err;
                }

                // Get parents
                let mut parent_spends = BTreeSet::new();
                if is_genesis_parent_tx(&signed_spend.spend.cashnote_creation_tx)
                    && signed_spend.unique_pubkey() == &GENESIS_CASHNOTE.id
                {
                    trace!("GENESIS_CASHNOTE {cash_note_addr:?} doesn't have a parent");
                } else {
                    trace!(
                        "Checking cash_note {cash_note_addr:?} parent transaction {:?}",
                        signed_spend.spend.cashnote_creation_tx
                    );
                    for parent_input in &signed_spend.spend.cashnote_creation_tx.inputs {
                        let parent_cash_note_address =
                            SpendAddress::from_unique_pubkey(&parent_input.unique_pubkey());
                        trace!(
                            "Checking parent input at {:?} - {parent_cash_note_address:?}",
                            parent_input.unique_pubkey(),
                        );
                        let parent = self
                            .network
                            .get_spend(parent_cash_note_address, false)
                            .await?;
                        trace!(
                            "Got parent input at {:?} - {parent_cash_note_address:?}",
                            parent_input.unique_pubkey(),
                        );
                        let _ = parent_spends.insert(parent);
                    }
                }

                // Check parents
                if let Err(err) = check_parent_spends(&parent_spends, &signed_spend) {
                    error!("Error while checking parent spends {err:?}");
                    return Err(err);
                }

                // check the network if any spend has happened for the same unique_pubkey
                // Does not return an error, instead the Vec<SignedSpend> is returned.
                debug!("Check if any spend exist for the same unique_pubkey {cash_note_addr:?}");
                let mut spends = match self.network.get_spend(cash_note_addr, false).await {
                    Ok(spend) => {
                        debug!("Got spend from network for the same unique_pubkey");
                        vec![spend]
                    }
                    // Q: Should we not aggregate the double spends instead of using vec![]
                    Err(err) => {
                        debug!("fetching spend for the same unique_pubkey returned: {err:?}");
                        vec![]
                    }
                };
                // aggregate the spends from the network with our own
                spends.push(signed_spend);
                aggregate_spends(spends, unique_pubkey)
            }
            _ => {
                warn!("Received >1 spends with parent. Aggregating the spends to check for double spend. Not performing parent check or querying the network for double spend");
                // if we got 2 or more, then it is a double spend for sure.
                // We don't have to check parent/ ask network for extra spend.
                // Validate and store just 2 of them.
                // The nodes will be synced up during replication.
                aggregate_spends(signed_spends, unique_pubkey)
            }
        };

        Ok(Some(signed_spends))
    }
}

// Helper to calculate total amout of tokens received in a given set of CashNotes
fn total_cash_notes_amount<'a, I>(
    cash_notes: I,
    pretty_key: PrettyPrintRecordKey,
) -> Result<NanoTokens, ProtocolError>
where
    I: IntoIterator<Item = &'a CashNote>,
{
    let mut received_fee = NanoTokens::zero();
    for cash_note in cash_notes {
        let amount = cash_note.value().map_err(|_| {
            ProtocolError::RecordNotStored(
                pretty_key.clone().into_owned(),
                "Failed to get CashNote value".to_string(),
            )
        })?;
        received_fee = received_fee
            .checked_add(amount)
            .ok_or(ProtocolError::RecordNotStored(
                pretty_key.clone().into_owned(),
                "CashNote value overflow".to_string(),
            ))?;
    }

    Ok(received_fee)
}
