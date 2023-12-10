// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    node::Node,
    node::ROYALTY_TRANSFER_NOTIF_TOPIC,
    spends::{aggregate_spends, check_parent_spends},
    Error, Marker, Result,
};
use bytes::{BufMut, BytesMut};
use libp2p::kad::{Record, RecordKey};
use serde::Serialize;
use sn_networking::{get_singed_spends_from_record, Error as NetworkError, GetRecordError};
use sn_protocol::{
    messages::CmdOk,
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, RecordHeader, RecordKind, RecordType,
        SpendAddress,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;
use sn_transfers::{
    calculate_royalties_fee, is_genesis_parent_tx, CashNote, CashNoteRedemption, LocalWallet,
    NanoTokens, Payment, SignedSpend, Transfer, UniquePubkey, WalletError, GENESIS_CASHNOTE,
    NETWORK_ROYALTIES_PK,
};
use std::collections::{BTreeSet, HashSet};
use xor_name::XorName;

impl Node {
    /// Validate a record and it's payment, and store the record to the RecordStore
    pub(crate) async fn validate_and_store_record(&self, record: Record) -> Result<CmdOk> {
        let record_header = RecordHeader::from_record(&record)?;

        match record_header.kind {
            RecordKind::ChunkWithPayment => {
                let record_key = record.key.clone();
                let (payment, chunk) = try_deserialize_record::<(Payment, Chunk)>(&record)?;
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

                // Writing chunk to disk takes time, hence try to execute it first.
                // So that when the replicate target asking for the copy,
                // the node can have a higher chance to respond.
                let store_chunk_result = self.store_chunk(&chunk);
                self.replicate_paid_record(record_key, RecordType::Chunk);

                store_chunk_result
            }
            RecordKind::Chunk => {
                error!("Chunk should not be validated at this point");
                Err(Error::InvalidPutWithoutPayment(
                    PrettyPrintRecordKey::from(&record.key).into_owned(),
                ))
            }
            RecordKind::Spend => self.validate_spend_record(record).await,
            RecordKind::Register => {
                let register = try_deserialize_record::<SignedRegister>(&record)?;

                // make sure we already have this register locally
                let net_addr = NetworkAddress::from_register_address(*register.address());
                let key = net_addr.to_record_key();
                let pretty_key = PrettyPrintRecordKey::from(&key);
                trace!("Got record to store without payment for register at {pretty_key:?}");
                if !self.validate_key_and_existence(&net_addr, &key).await? {
                    trace!("Ignore store without payment for register at {pretty_key:?}");
                    return Err(Error::InvalidPutWithoutPayment(
                        PrettyPrintRecordKey::from(&record.key).into_owned(),
                    ));
                }

                // store the update
                trace!("Store update without payment as we already had register at {pretty_key:?}");
                self.validate_and_store_register(register, true).await
            }
            RecordKind::RegisterWithPayment => {
                let (payment, register) =
                    try_deserialize_record::<(Payment, SignedRegister)>(&record)?;

                // check if the deserialized value's RegisterAddress matches the record's key
                let net_addr = NetworkAddress::from_register_address(*register.address());
                let key = net_addr.to_record_key();
                let pretty_key = PrettyPrintRecordKey::from(&key);
                if record.key != key {
                    warn!(
                        "Record's key {pretty_key:?} does not match with the value's RegisterAddress, ignoring PUT."
                    );
                    return Err(Error::RecordKeyMismatch);
                }

                let already_exists = self.validate_key_and_existence(&net_addr, &key).await?;

                // The register may already exist during the replication.
                // The payment shall get deposit to self even the register already presents.
                // However, if the register already presents, the incoming one maybe for edit only.
                // Hence the corresponding payment error shall not be thrown out.
                if let Err(err) = self
                    .payment_for_us_exists_and_is_still_valid(&net_addr, payment)
                    .await
                {
                    if already_exists {
                        trace!("Payment of the incoming exists register {pretty_key:?} having error {err:?}");
                    } else {
                        error!("Payment of the incoming non-exist register {pretty_key:?} having error {err:?}");
                        return Err(err);
                    }
                }

                self.validate_and_store_register(register, true).await
            }
        }
    }

    /// Perform all validations required on a SpendRequest entry.
    /// This applies for PUT and replication
    async fn validate_spend_record(&self, record: Record) -> Result<CmdOk> {
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

        self.validate_and_store_spends(spends, &record_key).await
    }

    /// Store a pre-validated, and already paid record to the RecordStore
    pub(crate) async fn store_prepaid_record(&self, record: Record) -> Result<CmdOk> {
        trace!("Storing prepaid record {record:?}");
        let record_header = RecordHeader::from_record(&record)?;
        match record_header.kind {
            // A separate flow handles payment for chunks and registers
            RecordKind::ChunkWithPayment | RecordKind::RegisterWithPayment => {
                warn!("Prepaid record came with Payment, which should be handled in another flow");
                Err(Error::UnexpectedRecordWithPayment(
                    PrettyPrintRecordKey::from(&record.key).into_owned(),
                ))
            }
            RecordKind::Chunk => {
                let chunk = try_deserialize_record::<Chunk>(&record)?;

                let record_key = record.key.clone();
                let already_exists = self
                    .validate_key_and_existence(&chunk.network_address(), &record_key)
                    .await?;
                trace!("Chunk with addr {chunk.network_address:?} already exists?: {already_exists}");
                if already_exists {
                    return Ok(CmdOk::DataAlreadyPresent);
                }

                self.store_chunk(&chunk)
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
                    return Err(Error::RecordKeyMismatch);
                }
                self.validate_and_store_register(register, false).await
            }
        }
    }

    /// Check key is valid compared to the network name, and if we already have this data or not.
    /// returns true if data already exists locally
    async fn validate_key_and_existence(
        &self,
        address: &NetworkAddress,
        expected_record_key: &RecordKey,
    ) -> Result<bool> {
        let data_key = address.to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(&data_key);

        if expected_record_key != &data_key {
            warn!(
                "record key: {:?}, key: {:?}",
                PrettyPrintRecordKey::from(expected_record_key),
                pretty_key
            );
            warn!("Record's key does not match with the value's address, ignoring PUT.");
            return Err(Error::RecordKeyMismatch);
        }

        let present_locally = self
            .network
            .is_record_key_present_locally(&data_key)
            .await?;

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

    /// Store a `Chunk` to the RecordStore
    pub(crate) fn store_chunk(&self, chunk: &Chunk) -> Result<CmdOk> {
        let chunk_name = *chunk.name();
        let chunk_addr = *chunk.address();

        let key = NetworkAddress::from_chunk_address(*chunk.address()).to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(&key).into_owned();

        let record = Record {
            key,
            value: try_serialize_record(&chunk, RecordKind::Chunk)?.to_vec(),
            publisher: None,
            expires: None,
        };

        // finally store the Record directly into the local storage
        debug!("Storing chunk {chunk_name:?} as Record locally");
        self.network.put_local_record(record)?;

        self.record_metrics(Marker::ValidChunkRecordPutFromNetwork(&pretty_key));

        self.events_channel
            .broadcast(crate::NodeEvent::ChunkStored(chunk_addr));

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store a `Register` to the RecordStore
    pub(crate) async fn validate_and_store_register(
        &self,
        register: SignedRegister,
        with_payment: bool,
    ) -> Result<CmdOk> {
        let reg_addr = register.address();
        debug!("Validating and storing register {reg_addr:?}");

        // check if the Register is present locally
        let key = NetworkAddress::from_register_address(*reg_addr).to_record_key();
        let present_locally = self.network.is_record_key_present_locally(&key).await?;
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
            value: try_serialize_record(&updated_register, RecordKind::Register)?.to_vec(),
            publisher: None,
            expires: None,
        };
        let content_hash = XorName::from_content(&record.value);

        debug!("Storing register {reg_addr:?} as Record locally");
        self.network.put_local_record(record)?;

        self.record_metrics(Marker::ValidRegisterRecordPutFromNetwork(&pretty_key));

        if with_payment {
            self.replicate_paid_record(key, RecordType::NonChunk(content_hash));
        }

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Validate and store `Vec<SignedSpend>` to the RecordStore
    pub(crate) async fn validate_and_store_spends(
        &self,
        signed_spends: Vec<SignedSpend>,
        key_for_debug: &RecordKey,
    ) -> Result<CmdOk> {
        let pretty_key = PrettyPrintRecordKey::from(key_for_debug);

        // make sure that the unique_pubkeys match
        let unique_pubkey = if let Some((first, elements)) = signed_spends.split_first() {
            let common_unique_pubkey = *first.unique_pubkey();
            if elements
                .iter()
                .all(|spend| spend.unique_pubkey() == &common_unique_pubkey)
            {
                common_unique_pubkey
            } else {
                warn!("Found SignedSpends with different UniquePubKeys for {pretty_key:?}");
                return Err(Error::MultipleUniquePubKey);
            }
        } else {
            warn!("Empty vec provided to validate and store spend for {pretty_key:?}");
            return Err(Error::EmptySignedSpends);
        };
        let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);

        debug!(
            "Validating and storing spends {:?} - {pretty_key:?}",
            cash_note_addr.xorname(),
        );

        let key = NetworkAddress::from_spend_address(cash_note_addr).to_record_key();
        let present_locally = self.network.is_record_key_present_locally(&key).await?;

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
            value: try_serialize_record(&validated_spends, RecordKind::Spend)?.to_vec(),
            publisher: None,
            expires: None,
        };
        self.network.put_local_record(record)?;

        // Notify the sender of any double spend
        if validated_spends.len() > 1 {
            warn!(
                "Got a double spend for the SpendCashNote PUT with unique_pubkey {unique_pubkey:?}",
            );
            let mut proof = validated_spends.iter();
            if let (Some(spend_one), Some(spend_two)) = (proof.next(), proof.next()) {
                return Err(NetworkError::DoubleSpendAttempt(
                    Box::new(spend_one.to_owned()),
                    Box::new(spend_two.to_owned()),
                ))?;
            }
        }

        self.record_metrics(Marker::ValidSpendRecordPutFromNetwork(&pretty_key));

        Ok(CmdOk::StoredSuccessfully)
    }

    /// Gets CashNotes out of Transfers, this includes network verifications of the Transfers
    /// Rewraps the royalties transfers into encrypted Transfers ready to be sent directly to the beneficiary
    async fn cash_notes_from_transfers(
        &self,
        transfers: Vec<Transfer>,
        wallet: &LocalWallet,
        pretty_key: PrettyPrintRecordKey<'static>,
    ) -> Result<(NanoTokens, Vec<CashNote>, Vec<CashNoteRedemption>)> {
        let royalties_pk = *NETWORK_ROYALTIES_PK;
        let mut cash_notes = vec![];
        let mut royalties_cash_notes_r = vec![];
        let mut received_fee = NanoTokens::zero();

        for transfer in transfers {
            match transfer {
                Transfer::Encrypted(_) => match self
                    .network
                    .verify_and_unpack_transfer(&transfer, wallet)
                    .await
                {
                    // transfer not for us
                    Err(NetworkError::Transfers(WalletError::FailedToDecypherTransfer)) => continue,
                    // transfer invalid
                    Err(e) => return Err(e.into()),
                    // transfer ok, add to cash_notes and continue as more transfers might be ours
                    Ok(cns) => cash_notes.extend(cns),
                },
                Transfer::NetworkRoyalties(cashnote_redemptions) => {
                    match self
                        .network
                        .verify_cash_notes_redemptions(royalties_pk, &cashnote_redemptions)
                        .await
                    {
                        Ok(cash_notes) => {
                            let received_royalties = total_cash_notes_amount(&cash_notes)?;
                            trace!(
                                "{} network royalties payment cash notes found for record {pretty_key} for a total value of {received_royalties:?}",
                                cash_notes.len()
                            );
                            royalties_cash_notes_r.extend(cashnote_redemptions);
                            received_fee = received_fee
                                .checked_add(received_royalties)
                                .ok_or_else(|| Error::NumericOverflow)?;
                        }
                        Err(e) => {
                            warn!(
                                "Invalid network royalties payment for record {pretty_key}: {e:?}"
                            );
                        }
                    }
                }
            }
        }

        if cash_notes.is_empty() {
            Err(Error::NoPaymentToOurNode(pretty_key))
        } else {
            let received_fee_to_our_node = total_cash_notes_amount(&cash_notes)?;
            info!(
                "{} cash note/s (for a total of {received_fee_to_our_node:?}) are for us for {pretty_key}",
                cash_notes.len()
            );
            received_fee = received_fee
                .checked_add(received_fee_to_our_node)
                .ok_or_else(|| Error::NumericOverflow)?;

            Ok((received_fee, cash_notes, royalties_cash_notes_r))
        }
    }

    /// Perform validations on the provided `Record`.
    async fn payment_for_us_exists_and_is_still_valid(
        &self,
        address: &NetworkAddress,
        payment: Payment,
    ) -> Result<()> {
        let key = address.to_record_key();
        let pretty_key = PrettyPrintRecordKey::from(&key).into_owned();
        trace!("Validating record payment for {pretty_key}");

        // load wallet
        let mut wallet = LocalWallet::load_from(&self.network.root_dir_path)?;

        // unpack transfer
        trace!("Unpacking incoming Transfers for record {pretty_key}");
        let (received_fee, cash_notes, royalties_cash_notes_r) = self
            .cash_notes_from_transfers(payment.transfers, &wallet, pretty_key.clone())
            .await?;

        trace!("Received payment of {received_fee:?} for {pretty_key}");

        // deposit the CashNotes in our wallet
        wallet.deposit_and_store_to_disk(&cash_notes)?;
        #[cfg(feature = "open-metrics")]
        let _ = self
            .node_metrics
            .reward_wallet_balance
            .set(wallet.balance().as_nano() as i64);

        if royalties_cash_notes_r.is_empty() {
            warn!("No network royalties payment found for record {pretty_key}");
            return Err(Error::NoNetworkRoyaltiesPayment(pretty_key.into_owned()));
        }

        // publish a notification over gossipsub topic ROYALTY_TRANSFER_NOTIF_TOPIC
        // for the network royalties payment.
        let royalties_pk = *NETWORK_ROYALTIES_PK;
        trace!("Publishing a royalties transfer notification over gossipsub for record {pretty_key} and beneficiary {royalties_pk:?}");
        let royalties_pk_bytes = royalties_pk.to_bytes();

        let mut msg = BytesMut::with_capacity(royalties_pk_bytes.len());
        msg.extend_from_slice(&royalties_pk_bytes);
        let mut msg = msg.writer();
        let mut serialiser = rmp_serde::Serializer::new(&mut msg);
        match royalties_cash_notes_r.serialize(&mut serialiser) {
            Ok(()) => {
                let msg = msg.into_inner().freeze();
                if let Err(err) = self.network.publish_on_topic(ROYALTY_TRANSFER_NOTIF_TOPIC.to_string(), msg) {
                    debug!("Failed to publish a network royalties payment notification over gossipsub for record {pretty_key} and beneficiary {royalties_pk:?}: {err:?}");
                }
            }
            Err(err) => warn!("Failed to serialise network royalties payment data to publish a notification over gossipsub for record {pretty_key}: {err:?}"),
        }

        // check if the quote is valid
        let storecost = payment.quote.cost;
        self.verify_quote_for_storecost(payment.quote, address)?;
        trace!("Payment quote valid for record {pretty_key}");

        // Let's check payment is sufficient both for our store cost and for network royalties
        // Since the storage payment is made to a single node, we can calculate the royalties fee based on that single payment.
        let expected_royalties_fee = calculate_royalties_fee(storecost);
        let expected_fee = storecost
            .checked_add(expected_royalties_fee)
            .ok_or(Error::NumericOverflow)?;

        // finally, (after we accept any payments to us as they are ours now anyway)
        // lets check they actually paid enough
        if received_fee < expected_fee {
            trace!("Payment insufficient for record {pretty_key}. {received_fee:?} is less than {expected_fee:?}");
            return Err(Error::PaymentProofInsufficientAmount {
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
    ) -> Result<Option<SignedRegister>> {
        // check if register is valid
        let reg_addr = register.address();
        register.verify()?;

        // if we don't have it locally return it
        if !present_locally {
            debug!("Register with addr {reg_addr:?} is valid and doesn't exist locally");
            return Ok(Some(register.to_owned()));
        }
        trace!("Register with addr {reg_addr:?} exists locally, comparing with local version");

        let key = NetworkAddress::from_register_address(*reg_addr).to_record_key();

        // get local register
        let maybe_record = self.network.get_local_record(&key).await?;
        let record = match maybe_record {
            Some(r) => r,
            None => {
                error!("Register with addr {reg_addr:?} already exists locally, but not found in local storage");
                return Err(Error::RegisterNotFoundLocally(Box::new(*reg_addr)));
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
    ) -> Result<Option<Vec<SignedSpend>>> {
        // get the UniquePubkey; used for validation
        let cash_note_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);
        let record_key = NetworkAddress::from_spend_address(cash_note_addr).to_record_key();
        debug!(
            "Validating and storing spend {cash_note_addr:?}, present_locally: {present_locally}"
        );

        if present_locally {
            debug!("Spend with UniquePubkey {unique_pubkey:?} already exists, checking if it's the same spend/double spend",);
            let local_record = self.network.get_local_record(&record_key).await?;
            let local_record = match local_record {
                Some(r) => r,
                None => {
                    error!("Could not retrieve Record with key{record_key:?}, the Record is supposed to be present.");
                    return Err(Error::SpendNotFoundLocally(cash_note_addr));
                }
            };

            let local_header = RecordHeader::from_record(&local_record)?;
            // Make sure the local copy is of the same kind
            if !matches!(local_header.kind, RecordKind::Spend) {
                error!(
                    "Expected CashNoteRecord kind, found {:?}",
                    local_header.kind
                );
                return Err(NetworkError::RecordKindMismatch(RecordKind::Spend).into());
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
                debug!("No valid spends found locally while validating Spend PUT.");
                return Err(Error::EmptySignedSpends);
            }
            1 => {
                debug!(
                "Received a single SignedSpend, verifying the parent and checking for double spend"
            );
                // using remove as we match against the len() above
                let signed_spend = signed_spends.remove(0);

                // check the spend
                signed_spend.verify(signed_spend.spent_tx_hash())?;

                // Get parents
                let mut parent_spends = BTreeSet::new();
                if is_genesis_parent_tx(&signed_spend.spend.parent_tx)
                    && signed_spend.unique_pubkey() == &GENESIS_CASHNOTE.id
                {
                    trace!("GENESIS_CASHNOTE {cash_note_addr:?} doesn't have a parent");
                } else {
                    trace!(
                        "Checking cash_note {cash_note_addr:?} parent transaction {:?}",
                        signed_spend.spend.parent_tx
                    );
                    for parent_input in &signed_spend.spend.parent_tx.inputs {
                        let parent_cash_note_address =
                            SpendAddress::from_unique_pubkey(parent_input.unique_pubkey());
                        trace!(
                            "Checking parent input at {:?} - {parent_cash_note_address:?}",
                            parent_input.unique_pubkey(),
                        );
                        let parent = match self
                            .network
                            .get_spend(parent_cash_note_address, false)
                            .await
                        {
                            Ok(parent) => parent,
                            Err(err) => {
                                error!("Error while getting parent spend {parent_cash_note_address:?} for cash_note addr {cash_note_addr:?}: {err:?}");
                                return Err(err.into());
                            }
                        };
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
                debug!("Check if any spend exist for the same unique_pubkey {cash_note_addr:?}");
                let mut spends = match self.network.get_spend(cash_note_addr, false).await {
                    Ok(spend) => {
                        debug!("Got spend from network for the same unique_pubkey");
                        vec![spend]
                    }
                    Err(NetworkError::DoubleSpendAttempt(spend1, spend2)) => {
                        warn!("Spends for {cash_note_addr:?} is a double-spend. Aggregating and storing them.");
                        vec![*spend1, *spend2]
                    }
                    Err(NetworkError::GetRecordError(GetRecordError::NotEnoughCopies(record))) => {
                        warn!("Spends for {cash_note_addr:?} resulted in a failed quorum. Trying to aggregate the spends in them.");
                        match get_singed_spends_from_record(&record) {
                            Ok(spends) => spends,
                            Err(err) => {
                                error!("Error while trying to get signed spends out of a record for {cash_note_addr:?}: {err:?}");
                                vec![]
                            }
                        }
                    }
                    Err(NetworkError::GetRecordError(GetRecordError::SplitRecord {
                        result_map,
                    })) => {
                        let mut all_spends = vec![];
                        warn!("Spends for {cash_note_addr:?} resulted in a split record. Trying to aggregate the spends in them.");
                        for (_, (record, _)) in result_map.into_iter() {
                            match get_singed_spends_from_record(&record) {
                                Ok(spends) => all_spends.extend(spends),
                                Err(err) => {
                                    error!("Error while trying to get signed spends out of a record for {cash_note_addr:?}: {err:?}");
                                }
                            };
                        }
                        all_spends
                    }
                    // get_spend does not set a target record, so this should not happen. But handling it if something
                    // does change there.
                    Err(NetworkError::GetRecordError(GetRecordError::RecordDoesNotMatch(
                        returned_record,
                    ))) => {
                        warn!("Spends for {cash_note_addr:?} resulted in a record does not match error . Trying to aggregate the spends in them.");
                        match get_singed_spends_from_record(&returned_record) {
                            Ok(spends) => spends,
                            Err(err) => {
                                error!("Error while trying to get signed spends out of a record for {cash_note_addr:?}: {err:?}");
                                vec![]
                            }
                        }
                    }
                    Err(err) => {
                        debug!("Fetching spend for the same unique_pubkey {cash_note_addr:?} returned: {err:?}");
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
fn total_cash_notes_amount<'a, I>(cash_notes: I) -> Result<NanoTokens>
where
    I: IntoIterator<Item = &'a CashNote>,
{
    let mut received_fee = NanoTokens::zero();
    for cash_note in cash_notes {
        let amount = cash_note.value()?;
        received_fee = received_fee
            .checked_add(amount)
            .ok_or(Error::NumericOverflow)?;
    }

    Ok(received_fee)
}
