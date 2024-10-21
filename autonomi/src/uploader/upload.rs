// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    GetStoreCostStrategy, TaskResult, UploadCfg, UploadEvent, UploadItem, UploadSummary, Uploader,
    UploaderInterface, PAYMENT_BATCH_SIZE,
};
#[cfg(feature = "registers")]
use crate::client::registers::Register;
use crate::{uploader::UploadError, utils::payment_proof_from_quotes_and_payments, Client};
use bytes::Bytes;
use itertools::Either;
use libp2p::{kad::Quorum, PeerId};
use rand::{thread_rng, Rng};
use sn_evm::{Amount, EvmWallet, ProofOfPayment};
use sn_networking::target_arch::{mpsc, mpsc_channel, mpsc_recv, spawn};
use sn_networking::{GetRecordCfg, PayeeQuote, PutRecordCfg, VerificationKind};
#[cfg(feature = "data")]
use sn_protocol::{messages::ChunkProof, storage::Chunk};
use sn_protocol::{storage::RetryStrategy, NetworkAddress};
#[cfg(feature = "registers")]
use sn_registers::RegisterAddress;
use std::{
    collections::{HashMap, HashSet},
    num::NonZero,
};
use xor_name::XorName;

/// The maximum number of sequential payment failures before aborting the upload process.
#[cfg(not(test))]
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 3;
#[cfg(test)]
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 1;

/// The maximum number of sequential network failures before aborting the upload process.
// todo: use uploader.retry_strategy.get_count() instead.
#[cfg(not(test))]
const MAX_SEQUENTIAL_NETWORK_ERRORS: usize = 32;
#[cfg(test)]
const MAX_SEQUENTIAL_NETWORK_ERRORS: usize = 1;

/// The number of upload failures for a single data item before
#[cfg(not(test))]
const UPLOAD_FAILURES_BEFORE_SELECTING_DIFFERENT_PAYEE: usize = 3;
#[cfg(test)]
const UPLOAD_FAILURES_BEFORE_SELECTING_DIFFERENT_PAYEE: usize = 1;

type Result<T> = std::result::Result<T, UploadError>;

// TODO:
// 1. track each batch with an id
// 2. create a irrecoverable error type, so we can bail on io/serialization etc.
// 3. separate cfgs/retries for register/chunk etc
// 4. log whenever we insert/remove items. i.e., don't ignore values with `let _`

/// The main loop that performs the upload process.
/// An interface is passed here for easy testing.
pub(super) async fn start_upload(
    mut interface: Box<dyn UploaderInterface>,
) -> Result<UploadSummary> {
    let mut uploader = interface.take_inner_uploader();

    uploader.validate_upload_cfg()?;

    // Take out the testing task senders if any. This is only set for tests.
    let (task_result_sender, mut task_result_receiver) =
        if let Some(channels) = uploader.testing_task_channels.take() {
            channels
        } else {
            // 6 because of the 6 pipelines, 1 for redundancy.
            mpsc_channel(uploader.cfg.batch_size * 6 + 1)
        };
    let (make_payment_sender, make_payment_receiver) = mpsc_channel(uploader.cfg.batch_size);

    uploader.start_payment_processing_thread(
        make_payment_receiver,
        task_result_sender.clone(),
        uploader.cfg.payment_batch_size,
    )?;

    // chunks can be pushed to pending_get_store_cost directly
    #[cfg(feature = "data")]
    {
        uploader.pending_to_get_store_cost = uploader
            .all_upload_items
            .iter()
            .filter_map(|(xorname, item)| {
                if let UploadItem::Chunk { .. } = item {
                    Some((*xorname, GetStoreCostStrategy::Cheapest))
                } else {
                    None
                }
            })
            .collect();
    }

    // registers have to be verified + merged with remote replica, so we have to fetch it first.
    #[cfg(feature = "registers")]
    {
        uploader.pending_to_get_register = uploader
            .all_upload_items
            .iter()
            .filter_map(|(_xorname, item)| {
                if let UploadItem::Register { address, .. } = item {
                    Some(*address)
                } else {
                    None
                }
            })
            .collect();
    }

    loop {
        // Break if we have uploaded all the items.
        // The loop also breaks if we fail to get_store_cost / make payment / upload for n consecutive times.
        if uploader.all_upload_items.is_empty() {
            debug!("Upload items are empty, exiting main upload loop.");

            // To avoid empty final_balance when all items are skipped. Skip for tests.
            #[cfg(not(test))]
            {
                uploader.upload_final_balance = uploader
                    .wallet
                    .balance_of_tokens()
                    .await
                    .inspect_err(|err| {
                        error!("Failed to get wallet balance: {err:?}");
                    })?;
            }

            #[cfg(test)]
            trace!("UPLOADER STATE: finished uploading all items {uploader:?}");
            let summary = UploadSummary {
                storage_cost: uploader.tokens_spent,
                final_balance: uploader.upload_final_balance,
                uploaded_addresses: uploader.uploaded_addresses,
                uploaded_count: uploader.uploaded_count,
                skipped_count: uploader.skipped_count,
                uploaded_registers: uploader.uploaded_registers,
            };

            if !uploader.max_repayments_reached.is_empty() {
                error!(
                    "The maximum repayments were reached for these addresses: {:?}",
                    uploader.max_repayments_reached
                );
                return Err(UploadError::MaximumRepaymentsReached {
                    items: uploader.max_repayments_reached.into_iter().collect(),
                });
            }

            return Ok(summary);
        }

        // try to GET register if we have enough buffer.
        // The results of the get & push register steps are used to fill up `pending_to_get_store` cost
        // Since the get store cost list is the init state, we don't have to check if it is not full.
        #[cfg(feature = "registers")]
        {
            while !uploader.pending_to_get_register.is_empty()
                && uploader.on_going_get_register.len() < uploader.cfg.batch_size
            {
                if let Some(reg_addr) = uploader.pending_to_get_register.pop() {
                    trace!("Conditions met for GET registers {:?}", reg_addr.xorname());
                    let _ = uploader.on_going_get_register.insert(reg_addr.xorname());
                    interface.submit_get_register_task(
                        uploader.client.clone(),
                        reg_addr,
                        task_result_sender.clone(),
                    );
                }
            }
        }

        // try to push register if we have enough buffer.
        // No other checks for the same reason as the above step.
        #[cfg(feature = "registers")]
        {
            while !uploader.pending_to_push_register.is_empty()
                && uploader.on_going_get_register.len() < uploader.cfg.batch_size
            {
                let upload_item = uploader.pop_item_for_push_register()?;
                trace!(
                    "Conditions met for push registers {:?}",
                    upload_item.xorname()
                );
                let _ = uploader
                    .on_going_push_register
                    .insert(upload_item.xorname());
                interface.submit_push_register_task(
                    uploader.client.clone(),
                    upload_item,
                    uploader.cfg.verify_store,
                    task_result_sender.clone(),
                );
            }
        }

        // try to get store cost for an item if pending_to_pay needs items & if we have enough buffer.
        while !uploader.pending_to_get_store_cost.is_empty()
            && uploader.on_going_get_cost.len() < uploader.cfg.batch_size
            && uploader.pending_to_pay.len() < uploader.cfg.payment_batch_size
        {
            let (xorname, address, get_store_cost_strategy) =
                uploader.pop_item_for_get_store_cost()?;
            trace!("Conditions met for get store cost. {xorname:?} {get_store_cost_strategy:?}",);

            let _ = uploader.on_going_get_cost.insert(xorname);
            interface.submit_get_store_cost_task(
                uploader.client.clone(),
                xorname,
                address,
                uploader.payment_proofs.get(&xorname),
                get_store_cost_strategy,
                uploader.cfg.max_repayments_for_failed_data,
                task_result_sender.clone(),
            );
        }

        // try to make payment for an item if pending_to_upload needs items & if we have enough buffer.
        while !uploader.pending_to_pay.is_empty()
            && uploader.on_going_payments.len() < uploader.cfg.payment_batch_size
            && uploader.pending_to_upload.len() < uploader.cfg.batch_size
        {
            let (upload_item, quote) = uploader.pop_item_for_make_payment()?;
            trace!(
                "Conditions met for making payments. {:?} {quote:?}",
                upload_item.xorname()
            );
            let _ = uploader.on_going_payments.insert(upload_item.xorname());

            interface
                .submit_make_payment_task(Some((upload_item, quote)), make_payment_sender.clone());
        }

        // try to upload if we have enough buffer to upload.
        while !uploader.pending_to_upload.is_empty()
            && uploader.on_going_uploads.len() < uploader.cfg.batch_size
        {
            #[cfg(test)]
            trace!("UPLOADER STATE: upload_item : {uploader:?}");
            let upload_item = uploader.pop_item_for_upload_item()?;
            let xorname = upload_item.xorname();

            trace!("Conditions met for uploading. {xorname:?}");
            let _ = uploader.on_going_uploads.insert(xorname);
            interface.submit_upload_item_task(
                upload_item,
                uploader.client.clone(),
                uploader.payment_proofs.get(&xorname),
                uploader.cfg.verify_store,
                uploader.cfg.retry_strategy,
                task_result_sender.clone(),
            );
        }

        // Fire None to trigger a forced round of making leftover payments, if there are not enough store cost tasks
        // to fill up the buffer.
        if uploader.pending_to_get_store_cost.is_empty()
            && uploader.on_going_get_cost.is_empty()
            && !uploader.on_going_payments.is_empty()
            && uploader.on_going_payments.len() < uploader.cfg.payment_batch_size
        {
            #[cfg(test)]
            trace!("UPLOADER STATE: make_payment (forced): {uploader:?}");

            debug!("There are not enough on going payments to trigger a batch Payment and no get_store_costs to fill the batch. Triggering forced round of payment");
            interface.submit_make_payment_task(None, make_payment_sender.clone());
        }

        #[cfg(test)]
        trace!("UPLOADER STATE: before await task result: {uploader:?}");

        trace!("Fetching task result");
        let task_result = mpsc_recv(&mut task_result_receiver)
            .await
            .ok_or(UploadError::InternalError)?;
        trace!("Received task result: {task_result:?}");
        match task_result {
            #[cfg(feature = "registers")]
            TaskResult::GetRegisterFromNetworkOk { remote_register } => {
                // if we got back the register, then merge & PUT it.
                let xorname = remote_register.address().xorname();
                trace!("TaskResult::GetRegisterFromNetworkOk for remote register: {xorname:?} \n{remote_register:?}");
                let _ = uploader.on_going_get_register.remove(&xorname);

                let reg = uploader.all_upload_items.get_mut(&xorname).ok_or_else(|| {
                    error!("Register {xorname:?} not found in all_upload_items.");
                    UploadError::InternalError
                })?;
                if let UploadItem::Register { reg, .. } = reg {
                    reg.merge(&remote_register).inspect_err(|err| {
                        error!("Uploader failed to merge remote register: {err:?}");
                    })?;
                    uploader.pending_to_push_register.push(xorname);
                }
            }
            #[cfg(feature = "registers")]
            TaskResult::GetRegisterFromNetworkErr(xorname) => {
                // then the register is a new one. It can follow the same flow as chunks now.
                let _ = uploader.on_going_get_register.remove(&xorname);

                uploader
                    .pending_to_get_store_cost
                    .push((xorname, GetStoreCostStrategy::Cheapest));
            }
            #[cfg(feature = "registers")]
            TaskResult::PushRegisterOk { updated_register } => {
                // push modifies the register, so we return this instead of the one from all_upload_items
                let xorname = updated_register.address().xorname();
                let _ = uploader.on_going_push_register.remove(&xorname);
                uploader.skipped_count += 1;
                let _ = uploader
                    .uploaded_addresses
                    .insert(NetworkAddress::from_register_address(
                        *updated_register.address(),
                    ));

                let _old_register =
                    uploader.all_upload_items.remove(&xorname).ok_or_else(|| {
                        error!("Register {xorname:?} not found in all_upload_items");
                        UploadError::InternalError
                    })?;

                if uploader.cfg.collect_registers {
                    let _ = uploader
                        .uploaded_registers
                        .insert(*updated_register.address(), updated_register.clone());
                }
                uploader.emit_upload_event(UploadEvent::RegisterUpdated(updated_register));
            }
            #[cfg(feature = "registers")]
            TaskResult::PushRegisterErr(xorname) => {
                // the register failed to be Pushed. Retry until failure.
                let _ = uploader.on_going_push_register.remove(&xorname);
                uploader.pending_to_push_register.push(xorname);

                uploader.push_register_errors += 1;
                if uploader.push_register_errors > MAX_SEQUENTIAL_NETWORK_ERRORS {
                    error!("Max sequential network failures reached during PushRegisterErr.");
                    return Err(UploadError::SequentialNetworkErrors);
                }
            }
            TaskResult::GetStoreCostOk { xorname, quote } => {
                let _ = uploader.on_going_get_cost.remove(&xorname);
                uploader.get_store_cost_errors = 0; // reset error if Ok. We only throw error after 'n' sequential errors

                trace!("GetStoreCostOk for {xorname:?}'s store_cost {:?}", quote.2);

                if !quote.2.cost.is_zero() {
                    uploader.pending_to_pay.push((xorname, quote));
                }
                // if cost is 0, then it already in the network.
                else {
                    // remove the item since we have uploaded it.
                    let removed_item =
                        uploader.all_upload_items.remove(&xorname).ok_or_else(|| {
                            error!("Uploadable item not found in all_upload_items: {xorname:?}");
                            UploadError::InternalError
                        })?;
                    let _ = uploader.uploaded_addresses.insert(removed_item.address());
                    trace!("{xorname:?} has store cost of 0 and it already exists on the network");
                    uploader.skipped_count += 1;

                    // if during the first try we skip the item, then it is already present in the network.
                    match removed_item {
                        #[cfg(feature = "data")]
                        UploadItem::Chunk { address, .. } => {
                            uploader.emit_upload_event(UploadEvent::ChunkAlreadyExistsInNetwork(
                                address,
                            ));
                        }
                        #[cfg(feature = "registers")]
                        UploadItem::Register { reg, .. } => {
                            if uploader.cfg.collect_registers {
                                let _ = uploader
                                    .uploaded_registers
                                    .insert(*reg.address(), reg.clone());
                            }
                            uploader.emit_upload_event(UploadEvent::RegisterUpdated(reg));
                        }
                    }
                }
            }
            TaskResult::GetStoreCostErr {
                xorname,
                get_store_cost_strategy,
                max_repayments_reached,
            } => {
                let _ = uploader.on_going_get_cost.remove(&xorname);
                trace!("GetStoreCostErr for {xorname:?} , get_store_cost_strategy: {get_store_cost_strategy:?}, max_repayments_reached: {max_repayments_reached:?}");

                // If max repayments reached, track it separately. Else retry get_store_cost.
                if max_repayments_reached {
                    error!("Max repayments reached for {xorname:?}. Skipping upload for it");
                    uploader.max_repayments_reached.insert(xorname);
                    uploader.all_upload_items.remove(&xorname);
                } else {
                    // use the same strategy. The repay different payee is set only if upload fails.
                    uploader
                        .pending_to_get_store_cost
                        .push((xorname, get_store_cost_strategy.clone()));
                }
                uploader.get_store_cost_errors += 1;
                if uploader.get_store_cost_errors > MAX_SEQUENTIAL_NETWORK_ERRORS {
                    error!("Max sequential network failures reached during GetStoreCostErr.");
                    return Err(UploadError::SequentialNetworkErrors);
                }
            }
            TaskResult::MakePaymentsOk { payment_proofs } => {
                let tokens_spent = payment_proofs
                    .values()
                    .map(|proof| proof.quote.cost.as_atto())
                    .try_fold(Amount::from(0), |acc, cost| acc.checked_add(cost))
                    .ok_or_else(|| {
                        error!("Overflow when summing up tokens spent");
                        UploadError::InternalError
                    })?;
                trace!(
                    "MakePaymentsOk for {} items, with {tokens_spent:?} tokens.",
                    payment_proofs.len(),
                );
                for xorname in payment_proofs.keys() {
                    let _ = uploader.on_going_payments.remove(xorname);
                }
                uploader
                    .pending_to_upload
                    .extend(payment_proofs.keys().cloned());
                for (xorname, proof) in payment_proofs {
                    if let Some(payments) = uploader.payment_proofs.get_mut(&xorname) {
                        payments.push(proof)
                    } else {
                        uploader.payment_proofs.insert(xorname, vec![proof]);
                    }
                }
                // reset sequential payment fail error if ok. We throw error if payment fails continuously more than
                // MAX_SEQUENTIAL_PAYMENT_FAILS errors.
                uploader.make_payments_errors = 0;
                uploader.tokens_spent = uploader
                    .tokens_spent
                    .checked_add(tokens_spent)
                    .ok_or_else(|| {
                        error!("Overflow when summing up tokens spent for summary.");
                        UploadError::InternalError
                    })?;

                uploader.emit_upload_event(UploadEvent::PaymentMade { tokens_spent });
            }
            TaskResult::MakePaymentsErr { failed_xornames } => {
                trace!("MakePaymentsErr for {:?} items", failed_xornames.len());
                // TODO: handle insufficient balance error

                for (xorname, quote) in failed_xornames {
                    let _ = uploader.on_going_payments.remove(&xorname);
                    uploader.pending_to_pay.push((xorname, quote));
                }
                uploader.make_payments_errors += 1;

                if uploader.make_payments_errors >= MAX_SEQUENTIAL_PAYMENT_FAILS {
                    error!("Max sequential upload failures reached during MakePaymentsErr.");
                    // Too many sequential overall payment failure indicating
                    // unrecoverable failure of spend tx continuously rejected by network.
                    // The entire upload process shall be terminated.
                    return Err(UploadError::SequentialUploadPaymentError);
                }
            }
            TaskResult::UploadOk(xorname) => {
                let _ = uploader.on_going_uploads.remove(&xorname);
                uploader.uploaded_count += 1;
                trace!("UploadOk for {xorname:?}");
                // remove the previous payments
                uploader.payment_proofs.remove(&xorname);
                // remove the item since we have uploaded it.
                let removed_item = uploader.all_upload_items.remove(&xorname).ok_or_else(|| {
                    error!("Uploadable item not found in all_upload_items: {xorname:?}");
                    UploadError::InternalError
                })?;
                let _ = uploader.uploaded_addresses.insert(removed_item.address());

                match removed_item {
                    #[cfg(feature = "data")]
                    UploadItem::Chunk { address, .. } => {
                        uploader.emit_upload_event(UploadEvent::ChunkUploaded(address));
                    }
                    #[cfg(feature = "registers")]
                    UploadItem::Register { reg, .. } => {
                        if uploader.cfg.collect_registers {
                            let _ = uploader
                                .uploaded_registers
                                .insert(*reg.address(), reg.clone());
                        }
                        uploader.emit_upload_event(UploadEvent::RegisterUploaded(reg));
                    }
                }
            }
            TaskResult::UploadErr { xorname, io_error } => {
                if let Some(io_error) = io_error {
                    error!(
                        "Upload failed for {xorname:?} with error: {io_error:?}. Stopping upload."
                    );
                    return Err(UploadError::Io(*io_error));
                }

                let _ = uploader.on_going_uploads.remove(&xorname);
                debug!("UploadErr for {xorname:?}. Keeping track of failure and trying again.");

                // keep track of the failure
                let n_errors = uploader.n_errors_during_uploads.entry(xorname).or_insert(0);
                *n_errors += 1;

                // if quote has expired, don't retry the upload again. Instead get the cheapest quote again.
                if *n_errors > UPLOAD_FAILURES_BEFORE_SELECTING_DIFFERENT_PAYEE {
                    // if error > threshold, then select different payee. else retry again
                    // Also reset n_errors as we want to enable retries for the new payee.
                    *n_errors = 0;
                    debug!("Max error during upload reached for {xorname:?}. Selecting a different payee.");

                    uploader
                        .pending_to_get_store_cost
                        .push((xorname, GetStoreCostStrategy::SelectDifferentPayee));
                } else {
                    uploader.pending_to_upload.push(xorname);
                }
            }
        }
    }
}

impl UploaderInterface for Uploader {
    fn take_inner_uploader(&mut self) -> InnerUploader {
        self.inner
            .take()
            .expect("Uploader::new makes sure inner is present")
    }

    fn submit_get_store_cost_task(
        &mut self,
        client: Client,
        xorname: XorName,
        address: NetworkAddress,
        previous_payments: Option<&Vec<ProofOfPayment>>,
        get_store_cost_strategy: GetStoreCostStrategy,
        max_repayments_for_failed_data: usize,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        trace!("Spawning get_store_cost for {xorname:?}");
        let previous_payments_to = if let Some(previous_payments) = previous_payments {
            let peer_ids = previous_payments
                .iter()
                .map(|payment_proof| {
                    payment_proof
                        .to_peer_id_payee()
                        .ok_or_else(|| {
                            error!("Invalid payment proof found, could not obtain peer_id {payment_proof:?}");
                            UploadError::InternalError
                        })
                })
                .collect::<Result<Vec<_>>>();
            peer_ids
        } else {
            Ok(vec![])
        };

        let _handle = spawn(async move {
            let task_result = match InnerUploader::get_store_cost(
                client,
                xorname,
                address,
                get_store_cost_strategy.clone(),
                previous_payments_to,
                max_repayments_for_failed_data,
            )
            .await
            {
                Ok(quote) => {
                    debug!("StoreCosts retrieved for {xorname:?} quote: {quote:?}");
                    TaskResult::GetStoreCostOk {
                        xorname,
                        quote: Box::new(quote),
                    }
                }
                Err(err) => {
                    error!("Encountered error {err:?} when getting store_cost for {xorname:?}",);

                    let max_repayments_reached =
                        matches!(&err, UploadError::MaximumRepaymentsReached { .. });

                    TaskResult::GetStoreCostErr {
                        xorname,
                        get_store_cost_strategy,
                        max_repayments_reached,
                    }
                }
            };

            let _ = task_result_sender.send(task_result).await;
        });
    }

    #[cfg(feature = "registers")]
    fn submit_get_register_task(
        &mut self,
        client: Client,
        reg_addr: RegisterAddress,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = reg_addr.xorname();
        trace!("Spawning get_register for {xorname:?}");
        let _handle = spawn(async move {
            let task_result = match InnerUploader::get_register(client, reg_addr).await {
                Ok(register) => {
                    debug!("Register retrieved for {xorname:?}");
                    TaskResult::GetRegisterFromNetworkOk {
                        remote_register: register,
                    }
                }
                Err(err) => {
                    // todo match on error to only skip if GetRecordError
                    warn!("Encountered error {err:?} during get_register. The register has to be PUT as it is a new one.");
                    TaskResult::GetRegisterFromNetworkErr(xorname)
                }
            };
            let _ = task_result_sender.send(task_result).await;
        });
    }

    #[cfg(feature = "registers")]
    fn submit_push_register_task(
        &mut self,
        client: Client,
        upload_item: UploadItem,
        verify_store: bool,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = upload_item.xorname();
        trace!("Spawning push_register for {xorname:?}");
        let _handle = spawn(async move {
            let task_result = match InnerUploader::push_register(client, upload_item, verify_store)
                .await
            {
                Ok(reg) => {
                    debug!("Register pushed: {xorname:?}");
                    TaskResult::PushRegisterOk {
                        updated_register: reg,
                    }
                }
                Err(err) => {
                    // todo match on error to only skip if GetRecordError
                    error!("Encountered error {err:?} during push_register. The register might not be present in the network");
                    TaskResult::PushRegisterErr(xorname)
                }
            };
            let _ = task_result_sender.send(task_result).await;
        });
    }

    fn submit_make_payment_task(
        &mut self,
        to_send: Option<(UploadItem, Box<PayeeQuote>)>,
        make_payment_sender: mpsc::Sender<Option<(UploadItem, Box<PayeeQuote>)>>,
    ) {
        let _handle = spawn(async move {
            let _ = make_payment_sender.send(to_send).await;
        });
    }

    fn submit_upload_item_task(
        &mut self,
        upload_item: UploadItem,
        client: Client,
        previous_payments: Option<&Vec<ProofOfPayment>>,
        verify_store: bool,
        retry_strategy: RetryStrategy,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        trace!("Spawning upload item task for {:?}", upload_item.xorname());

        let last_payment = previous_payments.and_then(|payments| payments.last().cloned());

        let _handle = spawn(async move {
            let xorname = upload_item.xorname();
            let result = InnerUploader::upload_item(
                client,
                upload_item,
                last_payment,
                verify_store,
                retry_strategy,
            )
            .await;

            trace!("Upload item {xorname:?} uploaded with result {result:?}");
            match result {
                Ok(_) => {
                    let _ = task_result_sender.send(TaskResult::UploadOk(xorname)).await;
                }
                Err(UploadError::Io(io_error)) => {
                    let _ = task_result_sender
                        .send(TaskResult::UploadErr {
                            xorname,
                            io_error: Some(Box::new(io_error)),
                        })
                        .await;
                }
                Err(_) => {
                    let _ = task_result_sender
                        .send(TaskResult::UploadErr {
                            xorname,
                            io_error: None,
                        })
                        .await;
                }
            };
        });
    }
}

/// `Uploader` provides functionality for uploading both Chunks and Registers with support for retries and queuing.
/// This struct is not cloneable. To create a new instance with default configuration, use the `new` function.
/// To modify the configuration, use the provided setter methods (`set_...` functions).
#[derive(custom_debug::Debug)]
pub(super) struct InnerUploader {
    pub(super) cfg: UploadCfg,
    #[debug(skip)]
    pub(super) client: Client,
    #[debug(skip)]
    pub(super) wallet: EvmWallet,

    // states
    pub(super) all_upload_items: HashMap<XorName, UploadItem>,
    #[cfg(feature = "registers")]
    pub(super) pending_to_get_register: Vec<RegisterAddress>,
    #[cfg(feature = "registers")]
    pub(super) pending_to_push_register: Vec<XorName>,
    pub(super) pending_to_get_store_cost: Vec<(XorName, GetStoreCostStrategy)>,
    pub(super) pending_to_pay: Vec<(XorName, Box<PayeeQuote>)>,
    pub(super) pending_to_upload: Vec<XorName>,
    pub(super) payment_proofs: HashMap<XorName, Vec<ProofOfPayment>>,

    // trackers
    #[cfg(feature = "registers")]
    pub(super) on_going_get_register: HashSet<XorName>,
    #[cfg(feature = "registers")]
    pub(super) on_going_push_register: HashSet<XorName>,
    pub(super) on_going_get_cost: HashSet<XorName>,
    pub(super) on_going_payments: HashSet<XorName>,
    pub(super) on_going_uploads: HashSet<XorName>,

    // error trackers
    pub(super) n_errors_during_uploads: HashMap<XorName, usize>,
    #[cfg(feature = "registers")]
    pub(super) push_register_errors: usize,
    pub(super) get_store_cost_errors: usize,
    pub(super) make_payments_errors: usize,

    // Upload summary
    pub(super) tokens_spent: Amount,
    pub(super) upload_final_balance: Amount,
    pub(super) max_repayments_reached: HashSet<XorName>,
    pub(super) uploaded_addresses: HashSet<NetworkAddress>,
    #[cfg(feature = "registers")]
    pub(super) uploaded_registers: HashMap<RegisterAddress, Register>,
    pub(super) uploaded_count: usize,
    pub(super) skipped_count: usize,

    // Task channels for testing. Not used in actual code.
    pub(super) testing_task_channels:
        Option<(mpsc::Sender<TaskResult>, mpsc::Receiver<TaskResult>)>,

    // Public events events
    #[debug(skip)]
    pub(super) logged_event_sender_absence: bool,
    #[debug(skip)]
    pub(super) event_sender: Option<mpsc::Sender<UploadEvent>>,
}

impl InnerUploader {
    pub(super) fn new(client: Client, wallet: EvmWallet) -> Self {
        Self {
            cfg: Default::default(),
            client,
            wallet,

            all_upload_items: Default::default(),
            #[cfg(feature = "registers")]
            pending_to_get_register: Default::default(),
            #[cfg(feature = "registers")]
            pending_to_push_register: Default::default(),
            pending_to_get_store_cost: Default::default(),
            pending_to_pay: Default::default(),
            pending_to_upload: Default::default(),
            payment_proofs: Default::default(),

            #[cfg(feature = "registers")]
            on_going_get_register: Default::default(),
            #[cfg(feature = "registers")]
            on_going_push_register: Default::default(),
            on_going_get_cost: Default::default(),
            on_going_payments: Default::default(),
            on_going_uploads: Default::default(),

            n_errors_during_uploads: Default::default(),
            #[cfg(feature = "registers")]
            push_register_errors: Default::default(),
            get_store_cost_errors: Default::default(),
            max_repayments_reached: Default::default(),
            make_payments_errors: Default::default(),

            tokens_spent: Amount::from(0),
            upload_final_balance: Amount::from(0),
            uploaded_addresses: Default::default(),
            #[cfg(feature = "registers")]
            uploaded_registers: Default::default(),
            uploaded_count: Default::default(),
            skipped_count: Default::default(),

            testing_task_channels: None,
            logged_event_sender_absence: Default::default(),
            event_sender: Default::default(),
        }
    }

    // ====== Pop items ======

    #[cfg(feature = "registers")]
    fn pop_item_for_push_register(&mut self) -> Result<UploadItem> {
        if let Some(name) = self.pending_to_push_register.pop() {
            let upload_item = self.all_upload_items.get(&name).cloned().ok_or_else(|| {
                error!("Uploadable item not found in all_upload_items: {name:?}");
                UploadError::InternalError
            })?;
            Ok(upload_item)
        } else {
            // the caller will be making sure this does not happen.
            error!("No item found for push register");
            Err(UploadError::InternalError)
        }
    }

    fn pop_item_for_get_store_cost(
        &mut self,
    ) -> Result<(XorName, NetworkAddress, GetStoreCostStrategy)> {
        let (xorname, strategy) = self.pending_to_get_store_cost.pop().ok_or_else(|| {
            error!("No item found for get store cost");
            UploadError::InternalError
        })?;
        let address = self
            .all_upload_items
            .get(&xorname)
            .map(|item| item.address())
            .ok_or_else(|| {
                error!("Uploadable item not found in all_upload_items: {xorname:?}");
                UploadError::InternalError
            })?;
        Ok((xorname, address, strategy))
    }

    fn pop_item_for_make_payment(&mut self) -> Result<(UploadItem, Box<PayeeQuote>)> {
        if let Some((name, quote)) = self.pending_to_pay.pop() {
            let upload_item = self.all_upload_items.get(&name).cloned().ok_or_else(|| {
                error!("Uploadable item not found in all_upload_items: {name:?}");
                UploadError::InternalError
            })?;
            Ok((upload_item, quote))
        } else {
            // the caller will be making sure this does not happen.
            error!("No item found for make payment");
            Err(UploadError::InternalError)
        }
    }

    fn pop_item_for_upload_item(&mut self) -> Result<UploadItem> {
        if let Some(name) = self.pending_to_upload.pop() {
            let upload_item = self.all_upload_items.get(&name).cloned().ok_or_else(|| {
                error!("Uploadable item not found in all_upload_items: {name:?}");
                UploadError::InternalError
            })?;
            Ok(upload_item)
        } else {
            // the caller will be making sure this does not happen.
            error!("No item found for upload item");
            Err(UploadError::InternalError)
        }
    }

    // ====== Processing Loop ======

    // This is spawned as a long running task to prevent us from reading the wallet files
    // each time we have to make a payment.
    fn start_payment_processing_thread(
        &self,
        mut make_payment_receiver: mpsc::Receiver<Option<(UploadItem, Box<PayeeQuote>)>>,
        task_result_sender: mpsc::Sender<TaskResult>,
        payment_batch_size: usize,
    ) -> Result<()> {
        let wallet = self.wallet.clone();

        let _handle = spawn(async move {
            debug!("Spawning the long running make payment processing loop.");

            let mut to_be_paid_list = Vec::new();
            let mut cost_map = HashMap::new();

            let mut got_a_previous_force_payment = false;
            while let Some(payment) = mpsc_recv(&mut make_payment_receiver).await {
                let make_payments = if let Some((item, quote)) = payment {
                    to_be_paid_list.push((
                        quote.2.hash(),
                        quote.2.rewards_address,
                        quote.2.cost.as_atto(),
                    ));
                    let xorname = item.xorname();
                    debug!("Inserted {xorname:?} into to_be_paid_list");

                    let _ = cost_map.insert(xorname, (quote.0, quote.1, quote.2));
                    cost_map.len() >= payment_batch_size || got_a_previous_force_payment
                } else {
                    // using None to indicate as all paid.
                    let make_payments = !cost_map.is_empty();
                    debug!("Got a forced forced round of make payment.");
                    // Note: There can be a mismatch of ordering between the main loop and the make payment loop because
                    // the instructions are sent via a task(channel.send().await). And there is no guarantee for the
                    // order to come in the same order as they were sent.
                    //
                    // We cannot just disobey the instruction inside the child loop, as the mainloop would be expecting
                    // a result back for a particular instruction.
                    if !make_payments {
                        got_a_previous_force_payment = true;
                        warn!(
                            "We were told to force make payment, but cost_map is empty, so we can't do that just yet. Waiting for a task to insert a quote into cost_map"
                        )
                    }

                    make_payments
                };

                if make_payments {
                    // reset force_make_payment
                    if got_a_previous_force_payment {
                        info!("A task inserted a quote into cost_map, so we can now make a forced round of payment!");
                        got_a_previous_force_payment = false;
                    }

                    let terminate_process = false;
                    let data_payments = std::mem::take(&mut to_be_paid_list);

                    let result = match wallet.pay_for_quotes(data_payments).await {
                        Ok(payments) => {
                            trace!("Made payments for {} records.", payments.len());

                            let payment_proofs =
                                payment_proof_from_quotes_and_payments(&cost_map, &payments);

                            TaskResult::MakePaymentsOk { payment_proofs }
                        }
                        Err(err) => {
                            let error = err.0;
                            let _succeeded_batch = err.1;

                            error!("When paying {} data, got error {error:?}", cost_map.len(),);
                            // TODO: match on insufficient gas/token error. and set terminate_process = true
                            TaskResult::MakePaymentsErr {
                                failed_xornames: cost_map
                                    .into_iter()
                                    .map(|(k, v)| (k, Box::new(v)))
                                    .collect(),
                            }
                        }
                    };
                    let result_sender = task_result_sender.clone();
                    let _handle = spawn(async move {
                        let _ = result_sender.send(result).await;
                    });

                    cost_map = HashMap::new();

                    if terminate_process {
                        // The error will trigger the entire upload process to be terminated.
                        // Hence here we shall terminate the inner loop first,
                        // to avoid the wallet going further to be potentially got corrupted.
                        warn!(
                            "Terminating make payment processing loop due to un-recoverable error."
                        );
                        break;
                    }
                }
            }
            debug!("Make payment processing loop terminated.");
        });
        Ok(())
    }

    // ====== Logic ======

    #[cfg(feature = "registers")]
    async fn get_register(client: Client, reg_addr: RegisterAddress) -> Result<Register> {
        let reg = client.register_get(reg_addr).await?;
        Ok(reg)
    }

    #[cfg(feature = "registers")]
    async fn push_register(
        client: Client,
        upload_item: UploadItem,
        verify_store: bool,
    ) -> Result<Register> {
        let register = if let UploadItem::Register { reg, .. } = upload_item {
            reg
        } else {
            error!("Invalid upload item found: {upload_item:?}");
            return Err(UploadError::InternalError);
        };

        let verification = if verify_store {
            let get_cfg = GetRecordCfg {
                get_quorum: Quorum::Majority,
                retry_strategy: Some(RetryStrategy::default()),
                target_record: None,
                expected_holders: Default::default(),
                is_register: true,
            };
            Some((VerificationKind::Network, get_cfg))
        } else {
            None
        };

        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: None,
            use_put_record_to: None,
            verification,
        };

        client.register_upload(&register, None, &put_cfg).await?;

        Ok(register)
    }

    async fn get_store_cost(
        client: Client,
        xorname: XorName,
        address: NetworkAddress,
        get_store_cost_strategy: GetStoreCostStrategy,
        previous_payments_to: Result<Vec<PeerId>>,
        max_repayments_for_failed_data: usize,
    ) -> Result<PayeeQuote> {
        let filter_list = match get_store_cost_strategy {
            GetStoreCostStrategy::Cheapest => vec![],
            GetStoreCostStrategy::SelectDifferentPayee => {
                let filter_list = previous_payments_to?;

                // if we have already made initial + max_repayments, then we should error out.
                if Self::have_we_reached_max_repayments(
                    filter_list.len(),
                    max_repayments_for_failed_data,
                ) {
                    // error is used by the caller.
                    return Err(UploadError::MaximumRepaymentsReached {
                        items: vec![xorname],
                    });
                }

                debug!("Filtering out payments from {filter_list:?} during get_store_cost for {xorname:?}");
                filter_list
            }
        };
        let quote = client
            .network
            .get_store_costs_from_network(address, filter_list)
            .await?;
        Ok(quote)
    }

    async fn upload_item(
        client: Client,
        upload_item: UploadItem,
        previous_payments: Option<ProofOfPayment>,
        verify_store: bool,
        retry_strategy: RetryStrategy,
    ) -> Result<()> {
        let xorname = upload_item.xorname();

        let payment_proof = previous_payments.ok_or_else(|| {
            error!("No payment proof found for {xorname:?}");
            UploadError::InternalError
        })?;
        let payee = payment_proof.to_peer_id_payee().ok_or_else(|| {
            error!("Invalid payment proof found, could not obtain peer_id {payment_proof:?}");
            UploadError::InternalError
        })?;

        debug!("Payments for upload item: {xorname:?} to {payee:?}:  {payment_proof:?}");

        match upload_item {
            #[cfg(feature = "data")]
            UploadItem::Chunk { address: _, chunk } => {
                let chunk = match chunk {
                    Either::Left(chunk) => chunk,
                    Either::Right(path) => {
                        let bytes = std::fs::read(&path).inspect_err(|err| {
                            error!("Error reading chunk at {path:?}: {err:?}");
                        })?;
                        Chunk::new(Bytes::from(bytes))
                    }
                };

                let verification = if verify_store {
                    let verification_cfg = GetRecordCfg {
                        get_quorum: Quorum::N(NonZero::new(2).expect("2 is non-zero")),
                        retry_strategy: Some(retry_strategy),
                        target_record: None,
                        expected_holders: Default::default(),
                        is_register: false,
                    };

                    let random_nonce = thread_rng().gen::<u64>();
                    let expected_proof =
                        ChunkProof::from_chunk(&chunk, random_nonce).map_err(|err| {
                            error!("Failed to create chunk proof: {err:?}");
                            UploadError::Serialization(format!(
                                "Failed to create chunk proof for {xorname:?}"
                            ))
                        })?;

                    Some((
                        VerificationKind::ChunkProof {
                            expected_proof,
                            nonce: random_nonce,
                        },
                        verification_cfg,
                    ))
                } else {
                    None
                };

                let put_cfg = PutRecordCfg {
                    put_quorum: Quorum::One,
                    retry_strategy: Some(retry_strategy),
                    use_put_record_to: Some(vec![payee]),
                    verification,
                };

                debug!("Client upload started for chunk: {xorname:?}");
                client
                    .chunk_upload_with_payment(chunk, payment_proof, Some(put_cfg))
                    .await?;

                debug!("Client upload completed for chunk: {xorname:?}");
            }
            #[cfg(feature = "registers")]
            UploadItem::Register { address: _, reg } => {
                debug!("Client upload started for register: {xorname:?}");
                let verification = if verify_store {
                    let get_cfg = GetRecordCfg {
                        get_quorum: Quorum::Majority,
                        retry_strategy: Some(retry_strategy),
                        target_record: None,
                        expected_holders: Default::default(),
                        is_register: true,
                    };
                    Some((VerificationKind::Network, get_cfg))
                } else {
                    None
                };

                let put_cfg = PutRecordCfg {
                    put_quorum: Quorum::All,
                    retry_strategy: Some(retry_strategy),
                    use_put_record_to: Some(vec![payee]),
                    verification,
                };
                client
                    .register_upload(&reg, Some(&payment_proof), &put_cfg)
                    .await?;
                debug!("Client upload completed for register: {xorname:?}");
            }
        }

        Ok(())
    }

    // ====== Misc ======

    fn emit_upload_event(&mut self, event: UploadEvent) {
        if let Some(sender) = self.event_sender.as_ref() {
            let sender_clone = sender.clone();
            let _handle = spawn(async move {
                if let Err(err) = sender_clone.send(event).await {
                    error!("Error emitting upload event: {err:?}");
                }
            });
        } else if !self.logged_event_sender_absence {
            info!("FilesUpload upload event sender is not set. Use get_upload_events() if you need to keep track of the progress");
            self.logged_event_sender_absence = true;
        }
    }

    /// If we have already made initial + max_repayments_allowed, then we should error out.
    // separate function as it is used in test.
    pub(super) fn have_we_reached_max_repayments(
        payments_made: usize,
        max_repayments_allowed: usize,
    ) -> bool {
        // if max_repayments_allowed = 1, then we have reached capacity = true if 2 payments have been made. i.e.,
        // i.e., 1 initial + 1 repayment.
        payments_made > max_repayments_allowed
    }

    fn validate_upload_cfg(&self) -> Result<()> {
        if self.cfg.payment_batch_size > PAYMENT_BATCH_SIZE {
            error!("Payment batch size is greater than the maximum allowed: {PAYMENT_BATCH_SIZE}");
            return Err(UploadError::InvalidCfg(format!(
                "Payment batch size is greater than the maximum allowed: {PAYMENT_BATCH_SIZE}"
            )));
        }
        if self.cfg.payment_batch_size < 1 {
            error!("Payment batch size cannot be less than 1");
            return Err(UploadError::InvalidCfg(
                "Payment batch size cannot be less than 1".to_string(),
            ));
        }
        if self.cfg.batch_size < 1 {
            error!("Batch size cannot be less than 1");
            return Err(UploadError::InvalidCfg(
                "Batch size cannot be less than 1".to_string(),
            ));
        }

        Ok(())
    }
}
