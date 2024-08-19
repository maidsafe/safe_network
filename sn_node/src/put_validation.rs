// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{node::Node, quote::verify_quote_for_storecost, Error, Marker, Result};
use libp2p::kad::{Record, RecordKey};
use sn_networking::{get_raw_signed_spends_from_record, GetRecordError, NetworkError};
use sn_protocol::{
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, RecordHeader, RecordKind, RecordType,
        SpendAddress,
    },
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;
use sn_transfers::{
    calculate_royalties_fee, CashNote, CashNoteRedemption, HotWallet, NanoTokens, Payment,
    SignedSpend, Transfer, TransferError, UniquePubkey, WalletError, NETWORK_ROYALTIES_PK,
};
use std::collections::BTreeSet;
use tokio::task::JoinSet;
use xor_name::XorName;

impl Node {
    /// Validate a record and it's payment, and store the record to the RecordStore
    pub(crate) async fn validate_and_store_record(&self, record: Record) -> Result<()> {
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
                    // if we're receiving this chunk PUT again, and we have been paid,
                    // we eagery retry replicaiton as it seems like other nodes are having trouble
                    // did not manage to get this chunk as yet
                    self.replicate_valid_fresh_record(record_key, RecordType::Chunk);

                    // Notify replication_fetcher to mark the attempt as completed.
                    // Send the notification earlier to avoid it got skipped due to:
                    // the record becomes stored during the fetch because of other interleaved process.
                    self.network()
                        .notify_fetch_completed(record.key.clone(), RecordType::Chunk);

                    debug!(
                        "Chunk with addr {:?} already exists: {already_exists}, payment extracted.",
                        chunk.network_address()
                    );
                    return Ok(());
                }

                // Finally before we store, lets bail for any payment issues
                payment_res?;

                // Writing chunk to disk takes time, hence try to execute it first.
                // So that when the replicate target asking for the copy,
                // the node can have a higher chance to respond.
                let store_chunk_result = self.store_chunk(&chunk);

                if store_chunk_result.is_ok() {
                    Marker::ValidPaidChunkPutFromClient(&PrettyPrintRecordKey::from(&record.key))
                        .log();
                    self.replicate_valid_fresh_record(record_key, RecordType::Chunk);

                    // Notify replication_fetcher to mark the attempt as completed.
                    // Send the notification earlier to avoid it got skipped due to:
                    // the record becomes stored during the fetch because of other interleaved process.
                    self.network()
                        .notify_fetch_completed(record.key.clone(), RecordType::Chunk);
                }

                store_chunk_result
            }
            RecordKind::Chunk => {
                error!("Chunk should not be validated at this point");
                Err(Error::InvalidPutWithoutPayment(
                    PrettyPrintRecordKey::from(&record.key).into_owned(),
                ))
            }
            RecordKind::Spend => {
                let record_key = record.key.clone();
                let value_to_hash = record.value.clone();
                let spends = try_deserialize_record::<Vec<SignedSpend>>(&record)?;
                let result = self
                    .validate_merge_and_store_spends(spends, &record_key)
                    .await;
                if result.is_ok() {
                    Marker::ValidSpendPutFromClient(&PrettyPrintRecordKey::from(&record_key)).log();
                    let content_hash = XorName::from_content(&value_to_hash);
                    self.replicate_valid_fresh_record(
                        record_key,
                        RecordType::NonChunk(content_hash),
                    );

                    // Notify replication_fetcher to mark the attempt as completed.
                    // Send the notification earlier to avoid it got skipped due to:
                    // the record becomes stored during the fetch because of other interleaved process.
                    self.network().notify_fetch_completed(
                        record.key.clone(),
                        RecordType::NonChunk(content_hash),
                    );
                }
                result
            }
            RecordKind::Register => {
                let register = try_deserialize_record::<SignedRegister>(&record)?;

                // make sure we already have this register locally
                let net_addr = NetworkAddress::from_register_address(*register.address());
                let key = net_addr.to_record_key();
                let pretty_key = PrettyPrintRecordKey::from(&key);
                debug!("Got record to store without payment for register at {pretty_key:?}");
                if !self.validate_key_and_existence(&net_addr, &key).await? {
                    debug!("Ignore store without payment for register at {pretty_key:?}");
                    return Err(Error::InvalidPutWithoutPayment(
                        PrettyPrintRecordKey::from(&record.key).into_owned(),
                    ));
                }

                // store the update
                debug!("Store update without payment as we already had register at {pretty_key:?}");
                let result = self.validate_and_store_register(register, true).await;

                if result.is_ok() {
                    debug!("Successfully stored register update at {pretty_key:?}");
                    Marker::ValidPaidRegisterPutFromClient(&pretty_key).log();
                    // we dont try and force replicaiton here as there's state to be kept in sync
                    // which we leave up to the client to enforce

                    let content_hash = XorName::from_content(&record.value);

                    // Notify replication_fetcher to mark the attempt as completed.
                    // Send the notification earlier to avoid it got skipped due to:
                    // the record becomes stored during the fetch because of other interleaved process.
                    self.network().notify_fetch_completed(
                        record.key.clone(),
                        RecordType::NonChunk(content_hash),
                    );
                } else {
                    warn!("Failed to store register update at {pretty_key:?}");
                }
                result
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
                        debug!("Payment of the incoming exists register {pretty_key:?} having error {err:?}");
                    } else {
                        error!("Payment of the incoming non-exist register {pretty_key:?} having error {err:?}");
                        return Err(err);
                    }
                }

                let res = self.validate_and_store_register(register, true).await;
                if res.is_ok() {
                    let content_hash = XorName::from_content(&record.value);

                    // Notify replication_fetcher to mark the attempt as completed.
                    // Send the notification earlier to avoid it got skipped due to:
                    // the record becomes stored during the fetch because of other interleaved process.
                    self.network().notify_fetch_completed(
                        record.key.clone(),
                        RecordType::NonChunk(content_hash),
                    );
                }
                res
            }
        }
    }

    /// Store a pre-validated, and already paid record to the RecordStore
    pub(crate) async fn store_replicated_in_record(&self, record: Record) -> Result<()> {
        debug!("Storing record which was replicated to us {:?}", record.key);
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
                if already_exists {
                    debug!(
                        "Chunk with addr {:?} already exists?: {already_exists}, do nothing",
                        chunk.network_address()
                    );
                    return Ok(());
                }

                self.store_chunk(&chunk)
            }
            RecordKind::Spend => {
                let record_key = record.key.clone();
                let spends = try_deserialize_record::<Vec<SignedSpend>>(&record)?;
                self.validate_merge_and_store_spends(spends, &record_key)
                    .await
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
            .network()
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
    pub(crate) fn store_chunk(&self, chunk: &Chunk) -> Result<()> {
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
        self.network().put_local_record(record);

        self.record_metrics(Marker::ValidChunkRecordPutFromNetwork(&pretty_key));

        self.events_channel()
            .broadcast(crate::NodeEvent::ChunkStored(chunk_addr));

        Ok(())
    }

    /// Validate and store a `Register` to the RecordStore
    pub(crate) async fn validate_and_store_register(
        &self,
        register: SignedRegister,
        with_payment: bool,
    ) -> Result<()> {
        let reg_addr = register.address();
        debug!("Validating and storing register {reg_addr:?}");

        // check if the Register is present locally
        let key = NetworkAddress::from_register_address(*reg_addr).to_record_key();
        let present_locally = self.network().is_record_key_present_locally(&key).await?;
        let pretty_key = PrettyPrintRecordKey::from(&key);

        // check register and merge if needed
        let updated_register = match self.register_validation(&register, present_locally).await? {
            Some(reg) => {
                debug!("Register {pretty_key:?} needed to be updated");
                reg
            }
            None => {
                debug!("No update needed for register");
                return Ok(());
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

        info!("Storing register {reg_addr:?} with content of {content_hash:?} as Record locally");
        self.network().put_local_record(record);

        self.record_metrics(Marker::ValidRegisterRecordPutFromNetwork(&pretty_key));

        if with_payment {
            self.replicate_valid_fresh_record(key, RecordType::NonChunk(content_hash));
        }

        Ok(())
    }

    /// Validate and store `Vec<SignedSpend>` to the RecordStore
    /// If we already have a spend at this address, the Vec is extended and stored.
    pub(crate) async fn validate_merge_and_store_spends(
        &self,
        signed_spends: Vec<SignedSpend>,
        record_key: &RecordKey,
    ) -> Result<()> {
        let pretty_key = PrettyPrintRecordKey::from(record_key);
        debug!("Validating spends before storage at {pretty_key:?}");

        // only keep spends that match the record key
        let spends_for_key: Vec<SignedSpend> = signed_spends
            .into_iter()
            .filter(|s| {
                // get the record key for the spend
                let spend_address = SpendAddress::from_unique_pubkey(s.unique_pubkey());
                let network_address = NetworkAddress::from_spend_address(spend_address);
                let spend_record_key = network_address.to_record_key();
                let spend_pretty = PrettyPrintRecordKey::from(&spend_record_key);
                if &spend_record_key != record_key {
                    warn!("Ignoring spend for another record key {spend_pretty:?} when verifying: {pretty_key:?}");
                    return false;
                }
                true
            })
            .collect();

        // if we have no spends to verify, return early
        let unique_pubkey = match spends_for_key.as_slice() {
            [] => {
                warn!("Found no valid spends to verify upon validation for {pretty_key:?}");
                return Err(Error::InvalidRequest(format!(
                    "No spends to verify when validating {pretty_key:?}"
                )));
            }
            [a, ..] => {
                // they should all have the same unique_pubkey so we take the 1st one
                a.unique_pubkey()
            }
        };

        // validate the signed spends against the network and the local knowledge
        debug!("Validating spends for {pretty_key:?} with unique key: {unique_pubkey:?}");
        let validated_spends = match self
            .signed_spends_to_keep(spends_for_key.clone(), *unique_pubkey)
            .await
        {
            Ok((one, None)) => vec![one],
            Ok((one, Some(two))) => vec![one, two],
            Err(e) => {
                warn!("Failed to validate spends at {pretty_key:?} with unique key {unique_pubkey:?}: {e}");
                return Err(e);
            }
        };

        debug!(
            "Got {} validated spends with key: {unique_pubkey:?} at {pretty_key:?}",
            validated_spends.len()
        );

        // store the record into the local storage
        let record = Record {
            key: record_key.clone(),
            value: try_serialize_record(&validated_spends, RecordKind::Spend)?.to_vec(),
            publisher: None,
            expires: None,
        };
        self.network().put_local_record(record);
        debug!(
            "Successfully stored validated spends with key: {unique_pubkey:?} at {pretty_key:?}"
        );

        // Just log the double spend attempt. DoubleSpend error during PUT is not used and would just lead to
        // RecordRejected marker (which is incorrect, since we store double spends).
        if validated_spends.len() > 1 {
            warn!("Got double spend(s) of len {} for the Spend PUT with unique_pubkey {unique_pubkey}", validated_spends.len());
        }

        self.record_metrics(Marker::ValidSpendRecordPutFromNetwork(&pretty_key));
        Ok(())
    }

    /// Gets CashNotes out of Transfers, this includes network verifications of the Transfers
    /// Rewraps the royalties transfers into encrypted Transfers ready to be sent directly to the beneficiary
    async fn cash_notes_from_transfers(
        &self,
        transfers: Vec<Transfer>,
        wallet: &HotWallet,
        pretty_key: PrettyPrintRecordKey<'static>,
    ) -> Result<(NanoTokens, Vec<CashNote>, Vec<CashNoteRedemption>)> {
        let royalties_pk = *NETWORK_ROYALTIES_PK;
        let mut cash_notes = vec![];
        let mut royalties_cash_notes_r = vec![];
        let mut received_fee = NanoTokens::zero();

        for transfer in transfers {
            match transfer {
                Transfer::Encrypted(_) => match self
                    .network()
                    .verify_and_unpack_transfer(&transfer, wallet)
                    .await
                {
                    // transfer not for us
                    Err(NetworkError::Wallet(WalletError::FailedToDecypherTransfer)) => continue,
                    // transfer invalid
                    Err(e) => return Err(e.into()),
                    // transfer ok, add to cash_notes and continue as more transfers might be ours
                    Ok(cns) => cash_notes.extend(cns),
                },
                Transfer::NetworkRoyalties(cashnote_redemptions) => {
                    match self
                        .network()
                        .verify_cash_notes_redemptions(royalties_pk, &cashnote_redemptions)
                        .await
                    {
                        Ok(cash_notes) => {
                            let received_royalties = total_cash_notes_amount(&cash_notes)?;
                            debug!(
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
        debug!("Validating record payment for {pretty_key}");

        // load wallet
        let mut wallet = HotWallet::load_from(self.network().root_dir_path())?;
        let old_balance = wallet.balance().as_nano();

        // unpack transfer
        debug!("Unpacking incoming Transfers for record {pretty_key}");
        let (received_fee, mut cash_notes, royalties_cash_notes_r) = self
            .cash_notes_from_transfers(payment.transfers, &wallet, pretty_key.clone())
            .await?;

        // check for cash notes that we have already spent
        // this can happen in cases where the client retries a failed PUT after we have already used the cash note
        cash_notes.retain(|cash_note| {
            let already_present = wallet.cash_note_presents(&cash_note.unique_pubkey());
            if already_present {
                return !already_present;
            }

            let spend_addr = SpendAddress::from_unique_pubkey(&cash_note.unique_pubkey());
            !wallet.has_confirmed_spend(spend_addr)
        });
        if cash_notes.is_empty() {
            info!("All incoming cash notes were already received, no need to further process");
            return Err(Error::ReusedPayment);
        }

        debug!("Received payment of {received_fee:?} for {pretty_key}");

        // Notify `record_store` that the node received a payment.
        self.network().notify_payment_received();

        // deposit the CashNotes in our wallet
        wallet.deposit_and_store_to_disk(&cash_notes)?;
        let new_balance = wallet.balance().as_nano();
        info!(
            "The new wallet balance is {new_balance}, after earning {}",
            new_balance - old_balance
        );

        #[cfg(feature = "open-metrics")]
        if let Some(node_metrics) = self.node_metrics() {
            let _ = node_metrics
                .current_reward_wallet_balance
                .set(new_balance as i64);
        }

        if royalties_cash_notes_r.is_empty() {
            warn!("No network royalties payment found for record {pretty_key}");
            return Err(Error::NoNetworkRoyaltiesPayment(pretty_key.into_owned()));
        }

        // check if the quote is valid
        let storecost = payment.quote.cost;
        verify_quote_for_storecost(self.network(), payment.quote, address)?;
        debug!("Payment quote valid for record {pretty_key}");

        // Let's check payment is sufficient both for our store cost and for network royalties
        // Since the storage payment is made to a single node, we can calculate the royalties fee based on that single payment.
        let expected_royalties_fee = calculate_royalties_fee(storecost);
        let expected_fee = storecost
            .checked_add(expected_royalties_fee)
            .ok_or(Error::NumericOverflow)?;

        // finally, (after we accept any payments to us as they are ours now anyway)
        // lets check they actually paid enough
        if received_fee < expected_fee {
            debug!("Payment insufficient for record {pretty_key}. {received_fee:?} is less than {expected_fee:?}");
            return Err(Error::PaymentProofInsufficientAmount {
                paid: received_fee,
                expected: expected_fee,
            });
        }
        // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
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
        debug!("Register with addr {reg_addr:?} exists locally, comparing with local version");

        let key = NetworkAddress::from_register_address(*reg_addr).to_record_key();

        // get local register
        let maybe_record = self.network().get_local_record(&key).await?;
        let record = match maybe_record {
            Some(r) => r,
            None => {
                error!("Register with addr {reg_addr:?} already exists locally, but not found in local storage");
                return Err(Error::InvalidRequest(format!(
                    "Register with addr {reg_addr:?} claimed to be existing locally was not found"
                )));
            }
        };
        let local_register: SignedRegister = try_deserialize_record(&record)?;

        // merge the two registers
        let mut merged_register = local_register.clone();
        merged_register.verified_merge(register)?;
        if merged_register == local_register {
            debug!("Register with addr {reg_addr:?} is the same as the local version");
            Ok(None)
        } else {
            debug!("Register with addr {reg_addr:?} is different from the local version");
            Ok(Some(merged_register))
        }
    }

    /// Get the local spends for the provided `SpendAddress`
    /// This only fetches the spends from the local store and does not perform any network operations.
    async fn get_local_spends(&self, addr: SpendAddress) -> Result<Vec<SignedSpend>> {
        // get the local spends
        let record_key = NetworkAddress::from_spend_address(addr).to_record_key();
        debug!("Checking for local spends with key: {record_key:?}");
        let local_record = match self.network().get_local_record(&record_key).await? {
            Some(r) => r,
            None => {
                debug!("Spend is not present locally: {record_key:?}");
                return Ok(vec![]);
            }
        };

        // deserialize the record and get the spends
        let local_header = RecordHeader::from_record(&local_record)?;
        let record_kind = local_header.kind;
        if !matches!(record_kind, RecordKind::Spend) {
            error!("Found a {record_kind} when expecting to find Spend at {addr:?}");
            return Err(NetworkError::RecordKindMismatch(RecordKind::Spend).into());
        }
        let local_signed_spends: Vec<SignedSpend> = try_deserialize_record(&local_record)?;
        Ok(local_signed_spends)
    }

    /// Determine which spends our node should keep and store
    /// - get local spends and trust them
    /// - get spends from the network
    /// - verify incoming spend + network spends and ignore the invalid ones
    /// - orders all the verified spends by:
    ///     - if they have spent descendants (meaning live branch)
    ///     - deterministicaly by their order in the BTreeSet
    /// - returns the spend to keep along with another spend if it was a double spend
    /// - when we get more than two spends, only keeps 2 that are chosen deterministically so
    ///     all nodes running this code are eventually consistent
    async fn signed_spends_to_keep(
        &self,
        signed_spends: Vec<SignedSpend>,
        unique_pubkey: UniquePubkey,
    ) -> Result<(SignedSpend, Option<SignedSpend>)> {
        let spend_addr = SpendAddress::from_unique_pubkey(&unique_pubkey);
        debug!(
            "Validating before storing spend at {spend_addr:?} with unique key: {unique_pubkey}"
        );

        // trust local spends as we've verified them before
        let local_spends = self.get_local_spends(spend_addr).await?;

        // get spends from the network at the address for that unique pubkey
        let network_spends = match self.network().get_raw_spends(spend_addr).await {
            Ok(spends) => spends,
            Err(NetworkError::GetRecordError(GetRecordError::SplitRecord { result_map })) => {
                warn!("Got a split record (double spend) for {unique_pubkey:?} from the network");
                let mut spends = vec![];
                for (record, _) in result_map.values() {
                    match get_raw_signed_spends_from_record(record) {
                        Ok(s) => spends.extend(s),
                        Err(e) => warn!("Ignoring invalid record received from the network for spend: {unique_pubkey:?}: {e}"),
                    }
                }
                spends
            }
            Err(NetworkError::GetRecordError(GetRecordError::NotEnoughCopies {
                record,
                got,
                ..
            })) => {
                info!(
                    "Retrieved {got} copies of the record for {unique_pubkey:?} from the network"
                );
                match get_raw_signed_spends_from_record(&record) {
                    Ok(spends) => spends,
                    Err(err) => {
                        warn!("Ignoring invalid record received from the network for spend: {unique_pubkey:?}: {err}");
                        vec![]
                    }
                }
            }

            Err(e) => {
                warn!("Continuing without network spends as failed to get spends from the network for {unique_pubkey:?}: {e}");
                vec![]
            }
        };
        debug!(
            "For {unique_pubkey:?} got {} local spends, {} from network and {} provided",
            local_spends.len(),
            network_spends.len(),
            signed_spends.len()
        );
        debug!("Local spends {local_spends:?}; from network {network_spends:?}; provided {signed_spends:?}");

        // only verify spends we don't know of
        let mut all_verified_spends = BTreeSet::from_iter(local_spends.into_iter());
        let unverified_spends =
            BTreeSet::from_iter(network_spends.into_iter().chain(signed_spends.into_iter()));
        let known_spends = all_verified_spends.clone();
        let new_unverified_spends: BTreeSet<_> =
            unverified_spends.difference(&known_spends).collect();

        let mut tasks = JoinSet::new();
        for s in new_unverified_spends.into_iter() {
            let self_clone = self.clone();
            let spend_clone = s.clone();
            let _ = tasks.spawn(async move {
                let res = self_clone.network().verify_spend(&spend_clone).await;
                (spend_clone, res)
            });
        }

        // gather verified spends
        let mut double_spent_parent = BTreeSet::new();
        while let Some(res) = tasks.join_next().await {
            match res {
                Ok((spend, Ok(()))) => {
                    info!("Successfully verified {spend:?}");
                    let _inserted = all_verified_spends.insert(spend.to_owned().clone());
                }
                Ok((spend, Err(NetworkError::Transfer(TransferError::DoubleSpentParent)))) => {
                    warn!("Parent of {spend:?} was double spent, keeping aside in case we're a double spend as well");
                    let _ = double_spent_parent.insert(spend.clone());
                }
                Ok((spend, Err(e))) => {
                    // an error here most probably means the received spend is invalid
                    warn!("Skipping spend {spend:?} as an error occurred during validation: {e:?}");
                }
                Err(e) => {
                    let s =
                        format!("Async thread error while verifying spend {unique_pubkey}: {e:?}");
                    error!("{}", s);
                    return Err(Error::JoinErrorInAsyncThread(s))?;
                }
            }
        }

        // keep track of double spend with double spent parent
        if !all_verified_spends.is_empty() && !double_spent_parent.is_empty() {
            warn!("Parent of {unique_pubkey:?} was double spent, but it's also a double spend. So keeping track of this double spend attempt.");
            all_verified_spends.extend(double_spent_parent.into_iter())
        }

        // return 2 spends max
        let all_verified_spends: Vec<_> = all_verified_spends.into_iter().collect();
        match all_verified_spends.as_slice() {
            [one_spend] => Ok((one_spend.clone(), None)),
            [one, two] => Ok((one.clone(), Some(two.clone()))),
            [] => {
                warn!("Invalid request: none of the spends were valid for {unique_pubkey:?}");
                Err(Error::InvalidRequest(format!(
                    "Found no valid spends while validating Spends for {unique_pubkey:?}"
                )))
            }
            more => {
                warn!("Got more than 2 verified spends, this might be a double spend spam attack, making sure to favour live branches (branches with spent descendants)");
                let (one, two) = self.verified_spends_select_2_live(more).await?;
                Ok((one, Some(two)))
            }
        }
    }

    async fn verified_spends_select_2_live(
        &self,
        many_spends: &[SignedSpend],
    ) -> Result<(SignedSpend, SignedSpend)> {
        // get all spends descendants
        let mut tasks = JoinSet::new();
        for spend in many_spends {
            let descendants: BTreeSet<_> = spend
                .spend
                .spent_tx
                .outputs
                .iter()
                .map(|o| o.unique_pubkey())
                .map(SpendAddress::from_unique_pubkey)
                .collect();
            for d in descendants {
                let self_clone = self.clone();
                let spend_clone = spend.to_owned();
                let _ = tasks.spawn(async move {
                    let res = self_clone.network().get_raw_spends(d).await;
                    (spend_clone, res)
                });
            }
        }

        // identify up to two live spends (aka spends with spent descendants)
        let mut live_spends = BTreeSet::new();
        while let Some(res) = tasks.join_next().await {
            match res {
                Ok((spend, Ok(_descendant))) => {
                    debug!("Spend {spend:?} has a live descendant");
                    let _inserted = live_spends.insert(spend);
                }
                Ok((spend, Err(NetworkError::GetRecordError(GetRecordError::RecordNotFound)))) => {
                    debug!("Spend {spend:?} descendant was not found, continuing...");
                }
                Ok((spend, Err(e))) => {
                    warn!(
                        "Error fetching spend descendant while checking if {spend:?} is live: {e}"
                    );
                }
                Err(e) => {
                    let s = format!("Async thread error while selecting live spends: {e}");
                    error!("{}", s);
                    return Err(Error::JoinErrorInAsyncThread(s))?;
                }
            }
        }

        // order by live or not live, then order in the BTreeSet and take first 2
        let not_live_spends: BTreeSet<_> = many_spends
            .iter()
            .filter(|s| !live_spends.contains(s))
            .collect();
        debug!(
            "Got {} live spends and {} not live ones, keeping only the favoured 2",
            live_spends.len(),
            not_live_spends.len()
        );
        let ordered_spends: Vec<_> = live_spends
            .iter()
            .chain(not_live_spends.into_iter())
            .collect();
        match ordered_spends.as_slice() {
            [one, two, ..] => Ok((one.to_owned().clone(), two.to_owned().clone())),
            _ => Err(Error::InvalidRequest(format!(
                "Expected many spends but got {}",
                many_spends.len()
            ))),
        }
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
