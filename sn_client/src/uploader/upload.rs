// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    GetStoreCostStrategy, TaskResult, UploadEvent, UploadItem, UploadStats, UploaderInterface,
};
use crate::{Client, ClientRegister, Error as ClientError, FilesApi, Result, Uploader, BATCH_SIZE};
use bytes::Bytes;
use itertools::Either;
use sn_networking::PayeeQuote;
use sn_protocol::{
    messages::RegisterCmd,
    storage::{Chunk, RetryStrategy},
    NetworkAddress,
};
use sn_registers::{Register, RegisterAddress};
use sn_transfers::NanoTokens;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tiny_keccak::{Hasher, Sha3};
use tokio::sync::mpsc;
use xor_name::XorName;

/// The number of repayments to attempt for a failed item before returning an error.
/// If value = 1, we do an initial payment & 1 repayment. Thus we make a max 2 payments per data item.
pub(super) const MAX_REPAYMENTS_PER_FAILED_ITEM: usize = 1;

/// The maximum number of sequential payment failures before aborting the upload process.
#[cfg(not(test))]
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 3;
#[cfg(test)]
const MAX_SEQUENTIAL_PAYMENT_FAILS: usize = 1;

/// The maximum number of sequential network failures before aborting the upload process.
#[cfg(not(test))]
const MAX_SEQUENTIAL_NETWORK_ERRORS: usize = 5;
#[cfg(test)]
const MAX_SEQUENTIAL_NETWORK_ERRORS: usize = 1;

/// The number of upload failures for a single data item before
#[cfg(not(test))]
const UPLOAD_FAILURES_BEFORE_SELECTING_DIFFERENT_PAYEE: usize = 3;
#[cfg(test)]
const UPLOAD_FAILURES_BEFORE_SELECTING_DIFFERENT_PAYEE: usize = 1;

/// The main loop that performs the upload process.
/// An interface is passed here for easy testing.
pub(super) async fn start_upload(mut interface: Box<dyn UploaderInterface>) -> Result<UploadStats> {
    let mut uploader = interface.take_inner_uploader();
    // Take out the testing task senders if any. This is only set for tests.
    let (task_result_sender, mut task_result_receiver) =
        if let Some(channels) = uploader.testing_task_channels.take() {
            channels
        } else {
            // 6 because of the 6 pipelines, 1 for redundancy.
            mpsc::channel(uploader.batch_size * 6 + 1)
        };
    let (make_payment_sender, make_payment_receiver) = mpsc::channel(uploader.batch_size);

    uploader.spawn_paying_thread(
        make_payment_receiver,
        task_result_sender.clone(),
        uploader.batch_size,
    );

    // chunks can be pushed to pending_get_store_cost directly
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

    // registers have to be verified + merged with remote replica, so we have to fetch it first.
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

    loop {
        // Break if we have uploaded all the items.
        // The loop also breaks if we fail to get_store_cost / make payment / upload for n consecutive times.
        if uploader.all_upload_items.is_empty() {
            debug!("Upload items are empty, exiting main upload loop.");
            // To avoid empty final_balance when all items are skipped.
            uploader.upload_final_balance = uploader.api.wallet()?.balance();
            #[cfg(test)]
            trace!("UPLOADER STATE: finished uploading all items {uploader:?}");

            let stats = UploadStats {
                storage_cost: uploader.upload_storage_cost,
                royalty_fees: uploader.upload_royalty_fees,
                final_balance: uploader.upload_final_balance,
                uploaded_count: uploader.uploaded_count,
                skipped_count: uploader.skipped_count,
            };
            return Ok(stats);
        }

        // try to GET register if we have enough buffer.
        // The results of the get & push register steps are used to fill up `pending_to_get_store` cost
        // Since the get store cost list is the init state, we don't have to check if it is not full.
        while !uploader.pending_to_get_register.is_empty()
            && uploader.on_going_get_register.len() < uploader.batch_size
        {
            if let Some(reg_addr) = uploader.pending_to_get_register.pop() {
                trace!("Conditions met for GET registers {:?}", reg_addr.xorname());
                let _ = uploader.on_going_get_register.insert(reg_addr.xorname());
                interface.spawn_get_register(
                    uploader.api.client.clone(),
                    reg_addr,
                    task_result_sender.clone(),
                );
            }
        }

        // try to push register if we have enough buffer.
        // No other checks for the same reason as the above step.
        while !uploader.pending_to_push_register.is_empty()
            && uploader.on_going_get_register.len() < uploader.batch_size
        {
            let upload_item = uploader.pop_item_for_push_register()?;
            trace!(
                "Conditions met for push registers {:?}",
                upload_item.xorname()
            );
            let _ = uploader
                .on_going_push_register
                .insert(upload_item.xorname());
            interface.spawn_push_register(
                upload_item,
                uploader.verify_store,
                task_result_sender.clone(),
            );
        }

        // try to get store cost for an item if pending_to_pay needs items & if we have enough buffer.
        while !uploader.pending_to_get_store_cost.is_empty()
            && uploader.on_going_get_cost.len() < uploader.batch_size
            && uploader.pending_to_pay.len() < uploader.batch_size
        {
            let (upload_item, get_store_cost_strategy) = uploader.pop_item_for_get_store_cost()?;
            trace!(
                "Conditions met for get store cost. {:?} {get_store_cost_strategy:?}",
                upload_item.xorname()
            );

            let _ = uploader.on_going_get_cost.insert(upload_item.xorname());
            interface.spawn_get_store_cost(
                uploader.api.clone(),
                upload_item,
                get_store_cost_strategy,
                task_result_sender.clone(),
            );
        }

        // try to make payment for an item if pending_to_upload needs items & if we have enough buffer.
        while !uploader.pending_to_pay.is_empty()
            && uploader.on_going_payments.len() < uploader.batch_size
            && uploader.pending_to_upload.len() < uploader.batch_size
        {
            let (upload_item, quote) = uploader.pop_item_for_make_payment()?;
            trace!(
                "Conditions met for making payments. {:?} {quote:?}",
                upload_item.xorname()
            );
            let _ = uploader.on_going_payments.insert(upload_item.xorname());

            interface.spawn_make_payment(Some((upload_item, quote)), make_payment_sender.clone());
        }

        // try to upload if we have enough buffer to upload.
        while !uploader.pending_to_upload.is_empty()
            && uploader.on_going_uploads.len() < uploader.batch_size
        {
            let upload_item = uploader.pop_item_for_upload_item()?;
            trace!("Conditions met for uploading. {:?}", upload_item.xorname());
            let _ = uploader.on_going_uploads.insert(upload_item.xorname());
            interface.spawn_upload_item(
                upload_item,
                uploader.api.clone(),
                uploader.verify_store,
                uploader.retry_strategy,
                task_result_sender.clone(),
            );
        }

        // Fire None to trigger a forced round of making leftover payments, if there are not enough store cost tasks
        // to fill up the buffer.
        if uploader.pending_to_get_store_cost.is_empty()
            && uploader.on_going_get_cost.is_empty()
            && !uploader.on_going_payments.is_empty()
            && uploader.on_going_payments.len() < uploader.batch_size
        {
            #[cfg(test)]
            trace!("UPLOADER STATE: make_payment (forced): {uploader:?}");

            debug!("There are not enough on going payments to trigger a batch Payment and no get_store_costs to fill the batch. Triggering forced round of payment");
            interface.spawn_make_payment(None, make_payment_sender.clone());
        }

        #[cfg(test)]
        trace!("UPLOADER STATE: before await task result: {uploader:?}");

        trace!("Fetching task result");
        let task_result = task_result_receiver
            .recv()
            .await
            .ok_or(ClientError::InternalTaskChannelDropped)?;
        trace!("Received task result: {task_result:?}");
        match task_result {
            TaskResult::FailedToAccessWallet => return Err(ClientError::FailedToAccessWallet),
            TaskResult::GetRegisterFromNetworkOk { remote_register } => {
                // if we got back the register, then merge & PUT it.
                let xorname = remote_register.address().xorname();
                trace!("TaskResult::GetRegisterFromNetworkOk for remote register: {xorname:?} \n{remote_register:?}");
                let _ = uploader.on_going_get_register.remove(&xorname);

                let reg = uploader
                    .all_upload_items
                    .get_mut(&xorname)
                    .ok_or(ClientError::UploadableItemNotFound(xorname))?;
                if let UploadItem::Register { reg, .. } = reg {
                    // todo: not error out here
                    reg.register.merge(&remote_register)?;
                    uploader.pending_to_push_register.push(xorname);
                }
            }
            TaskResult::GetRegisterFromNetworkErr(xorname) => {
                // then the register is a new one. It can follow the same flow as chunks now.
                let _ = uploader.on_going_get_register.remove(&xorname);

                uploader
                    .pending_to_get_store_cost
                    .push((xorname, GetStoreCostStrategy::Cheapest));
            }
            TaskResult::PushRegisterOk { updated_register } => {
                // push modifies the register, so we return this instead of the one from all_upload_items
                // todo: keep track of these updated registers inside Uploader (if a flag is set). Because not
                // everyone will track the events.
                let xorname = updated_register.address().xorname();
                let _ = uploader.on_going_push_register.remove(&xorname);
                uploader.skipped_count += 1;

                let _old_register = uploader
                    .all_upload_items
                    .remove(&xorname)
                    .ok_or(ClientError::UploadableItemNotFound(xorname))?;

                uploader.emit_upload_event(UploadEvent::RegisterUpdated(updated_register));
            }
            TaskResult::PushRegisterErr(xorname) => {
                // the register failed to be Pushed. Retry until failure.
                let _ = uploader.on_going_push_register.remove(&xorname);
                uploader.pending_to_push_register.push(xorname);

                let n_errors = uploader
                    .n_errors_during_push_register
                    .entry(xorname)
                    .or_insert(0);
                *n_errors += 1;
                if *n_errors > uploader.retry_strategy.get_count() {
                    // todo: clear this up
                    return Err(ClientError::AmountIsZero);
                }
            }
            TaskResult::GetStoreCostOk { xorname, quote } => {
                let _ = uploader.on_going_get_cost.remove(&xorname);
                uploader.get_store_cost_errors = 0; // reset error if Ok. We only throw error after 'n' sequential errors

                trace!("GetStoreCostOk for {xorname:?}'s store_cost {:?}", quote.2);

                if quote.2.cost != NanoTokens::zero() {
                    uploader.pending_to_pay.push((xorname, quote));
                }
                // if cost is 0, then it already in the network.
                else {
                    // check if it as item that has prior failures. This item has been successfully uploaded during
                    // a retry. TODO: This assumption was wrong, because the repay payee flow kicks in during upload failure.
                    let prior_failure = uploader.n_errors_during_uploads.contains_key(&xorname);
                    if prior_failure {
                        // remove the item since we have uploaded it.
                        let removed_item = uploader
                            .all_upload_items
                            .remove(&xorname)
                            .ok_or(ClientError::UploadableItemNotFound(xorname))?;
                        match removed_item {
                            UploadItem::Chunk { address, .. } => {
                                uploader.emit_upload_event(UploadEvent::ChunkUploaded(address));
                            }
                            // todo: this cannot happen? If this is a register and if exists in the network,
                            // we should've merged + pushed it again. Maybe if this happens, mark it to be pushed
                            // again?
                            UploadItem::Register { reg, .. } => {
                                uploader.emit_upload_event(UploadEvent::RegisterUploaded(reg));
                            }
                        }
                        trace!("{xorname:?} has store cost of 0. It has been uploaded on a retry");
                        uploader.uploaded_count += 1;
                    } else {
                        // remove the item since we have uploaded it.
                        let removed_item = uploader
                            .all_upload_items
                            .remove(&xorname)
                            .ok_or(ClientError::UploadableItemNotFound(xorname))?;
                        // if during the first try we skip the item, then it is already present in the network.
                        match removed_item {
                            UploadItem::Chunk { address, .. } => {
                                uploader.emit_upload_event(
                                    UploadEvent::ChunkAlreadyExistsInNetwork(address),
                                );
                            }
                            UploadItem::Register { reg, .. } => {
                                uploader.emit_upload_event(UploadEvent::RegisterUpdated(reg));
                            }
                        }

                        trace!(
                            "{xorname:?} has store cost of 0 and it already exists on the network"
                        );
                        uploader.skipped_count += 1;
                    }
                }
            }
            TaskResult::GetStoreCostErr {
                xorname,
                get_store_cost_strategy,
                max_repayments_reached,
            } => {
                let _ = uploader.on_going_get_cost.remove(&xorname);
                // use the same strategy. The repay different payee is set only if upload fails.
                uploader
                    .pending_to_get_store_cost
                    .push((xorname, get_store_cost_strategy.clone()));
                trace!("GetStoreCostErr for {xorname:?} , get_store_cost_strategy: {get_store_cost_strategy:?}, max_repayments_reached: {max_repayments_reached:?}");

                // should we do something more here?
                if max_repayments_reached {
                    error!("Max repayments reached for {xorname:?}");
                    return Err(ClientError::MaximumRepaymentsReached(xorname));
                }
                uploader.get_store_cost_errors += 1;
                if uploader.get_store_cost_errors > MAX_SEQUENTIAL_NETWORK_ERRORS {
                    error!("Max sequential network failures reached during GetStoreCostErr.");
                    return Err(ClientError::SequentialNetworkErrors);
                }

                // keep track of the failure
                *uploader
                    .n_errors_during_get_cost
                    .entry(xorname)
                    .or_insert(0) += 1;
                // if error > threshold, then return from fn?
            }
            TaskResult::MakePaymentsOk {
                paid_xornames,
                storage_cost,
                royalty_fees,
                new_balance,
            } => {
                trace!("MakePaymentsOk for {} items: hash({:?}), with {storage_cost:?} store_cost and {royalty_fees:?} royalty_fees, and new_balance is {new_balance:?}",
                paid_xornames.len(), InnerUploader::hash_of_xornames(paid_xornames.iter()));
                for xorname in paid_xornames.iter() {
                    let _ = uploader.on_going_payments.remove(xorname);
                }
                uploader.pending_to_upload.extend(paid_xornames);
                uploader.make_payments_errors = 0;
                uploader.upload_final_balance = new_balance;
                uploader.upload_storage_cost = uploader
                    .upload_storage_cost
                    .checked_add(storage_cost)
                    .ok_or(ClientError::TotalPriceTooHigh)?;
                uploader.upload_royalty_fees = uploader
                    .upload_royalty_fees
                    .checked_add(royalty_fees)
                    .ok_or(ClientError::TotalPriceTooHigh)?;

                // reset sequential payment fail error if ok. We throw error if payment fails continuously more than
                // MAX_SEQUENTIAL_PAYMENT_FAILS errors.
                uploader.emit_upload_event(UploadEvent::PaymentMade {
                    storage_cost,
                    royalty_fees,
                    new_balance,
                });
            }
            TaskResult::MakePaymentsErr(xornames) => {
                trace!(
                    "MakePaymentsErr for {:?} items: hash({:?})",
                    xornames.len(),
                    InnerUploader::hash_of_xornames(xornames.iter().map(|(name, _)| name))
                );
                for (xorname, quote) in xornames {
                    let _ = uploader.on_going_payments.remove(&xorname);
                    uploader.pending_to_pay.push((xorname, quote));
                    // keep track of the failure
                    *uploader.n_errors_during_pay.entry(xorname).or_insert(0) += 1;
                    // if error > threshold, then return from fn? or should we select a different payee (i dont think so)
                }
                uploader.make_payments_errors += 1;

                if uploader.make_payments_errors >= MAX_SEQUENTIAL_PAYMENT_FAILS {
                    error!("Max sequential upload failures reached during MakePaymentsErr.");
                    // Too many sequential overall payment failure indicating
                    // unrecoverable failure of spend tx continuously rejected by network.
                    // The entire upload process shall be terminated.
                    return Err(ClientError::SequentialUploadPaymentError);
                }
            }
            TaskResult::UploadOk(xorname) => {
                let _ = uploader.on_going_uploads.remove(&xorname);
                uploader.uploaded_count += 1;
                trace!("UploadOk for {xorname:?}");

                // remove the item since we have uploaded it.
                let removed_item = uploader
                    .all_upload_items
                    .remove(&xorname)
                    .ok_or(ClientError::UploadableItemNotFound(xorname))?;
                match removed_item {
                    UploadItem::Chunk { address, .. } => {
                        uploader.emit_upload_event(UploadEvent::ChunkUploaded(address));
                    }
                    UploadItem::Register { reg, .. } => {
                        uploader.emit_upload_event(UploadEvent::RegisterUploaded(reg));
                    }
                }
            }
            TaskResult::UploadErr(xorname) => {
                let _ = uploader.on_going_uploads.remove(&xorname);
                trace!("UploadErr for {xorname:?}");

                // keep track of the failure
                let n_errors = uploader.n_errors_during_uploads.entry(xorname).or_insert(0);
                *n_errors += 1;
                // if error > threshold, then select different payee. else retry again
                if *n_errors > UPLOAD_FAILURES_BEFORE_SELECTING_DIFFERENT_PAYEE {
                    debug!(
                    "Max error during upload reached for {xorname:?}. Selecting a different payee."
                );
                    uploader.pending_to_get_store_cost.push((
                        xorname,
                        GetStoreCostStrategy::SelectDifferentPayee {
                            max_repayments: uploader.max_repayments_for_failed_data,
                        },
                    ));
                } else {
                    uploader.pending_to_upload.push(xorname);
                }
            }
        }
    }
}

impl UploaderInterface for Uploader {
    fn take_inner_uploader(&mut self) -> InnerUploader {
        // todo: make this an error
        self.inner.take().expect("Uploader should be present")
    }

    fn spawn_get_store_cost(
        &mut self,
        api: FilesApi,
        upload_item: UploadItem,
        get_store_cost_strategy: GetStoreCostStrategy,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        trace!("Spawning get_store_cost for {:?}", upload_item.xorname());
        let _handle = tokio::spawn(async move {
            let xorname = upload_item.xorname();

            let task_result = match InnerUploader::get_store_cost(
                api,
                upload_item,
                get_store_cost_strategy.clone(),
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
                        matches!(&err, ClientError::MaximumRepaymentsReached(_));

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

    fn spawn_get_register(
        &mut self,
        client: Client,
        reg_addr: RegisterAddress,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = reg_addr.xorname();
        trace!("Spawning get_register for {xorname:?}");
        let _handle = tokio::spawn(async move {
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

    fn spawn_push_register(
        &mut self,
        upload_item: UploadItem,
        verify_store: bool,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = upload_item.xorname();
        trace!("Spawning push_register for {xorname:?}");
        let _handle = tokio::spawn(async move {
            let task_result = match InnerUploader::push_register(upload_item, verify_store).await {
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

    fn spawn_make_payment(
        &mut self,
        to_send: Option<(UploadItem, Box<PayeeQuote>)>,
        make_payment_sender: mpsc::Sender<Option<(UploadItem, Box<PayeeQuote>)>>,
    ) {
        let _handle = tokio::spawn(async move {
            let _ = make_payment_sender.send(to_send).await;
        });
    }

    fn spawn_upload_item(
        &mut self,
        upload_item: UploadItem,
        api: FilesApi,
        verify_store: bool,
        retry_strategy: RetryStrategy,
        task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        trace!("Spawning upload item task for {:?}", upload_item.xorname());

        let _handle = tokio::spawn(async move {
            let xorname = upload_item.xorname();
            let result =
                InnerUploader::upload_item(api, upload_item, verify_store, retry_strategy).await;

            trace!("Upload item {xorname:?} uploaded with result {result:?}");
            if result.is_ok() {
                let _ = task_result_sender.send(TaskResult::UploadOk(xorname)).await;
            } else {
                let _ = task_result_sender
                    .send(TaskResult::UploadErr(xorname))
                    .await;
            }
        });
    }
}

// TODO:
// 1. each call to files_api::wallet() tries to load a lot of file. in our code during each upload/get store cost, it is
// invoked, don't do that.
// 2. add more debug lines
// 3. terminate early if there are not enough funds in the wallet and return that error.
// 4. we cna just return address form pop_item_for_get_store_cost. we don't need the whole item.

/// `Uploader` provides functionality for uploading both Chunks and Registers with support for retries and queuing.
/// This struct is not cloneable. To create a new instance with default configuration, use the `new` function.
/// To modify the configuration, use the provided setter methods (`set_...` functions).
#[derive(custom_debug::Debug)]
pub(super) struct InnerUploader {
    // Configurations
    pub(super) batch_size: usize,
    pub(super) verify_store: bool,
    pub(super) show_holders: bool,
    pub(super) retry_strategy: RetryStrategy,
    pub(super) max_repayments_for_failed_data: usize,
    // API
    #[debug(skip)]
    pub(super) api: FilesApi,

    // states
    pub(super) all_upload_items: HashMap<XorName, UploadItem>,
    pub(super) pending_to_get_register: Vec<RegisterAddress>,
    pub(super) pending_to_push_register: Vec<XorName>,
    pub(super) pending_to_get_store_cost: Vec<(XorName, GetStoreCostStrategy)>,
    pub(super) pending_to_pay: Vec<(XorName, Box<PayeeQuote>)>,
    pub(super) pending_to_upload: Vec<XorName>,

    // trackers
    pub(super) on_going_get_register: BTreeSet<XorName>,
    pub(super) on_going_push_register: BTreeSet<XorName>,
    pub(super) on_going_get_cost: BTreeSet<XorName>,
    pub(super) on_going_payments: BTreeSet<XorName>,
    pub(super) on_going_uploads: BTreeSet<XorName>,

    pub(super) n_errors_during_push_register: BTreeMap<XorName, usize>,
    pub(super) n_errors_during_get_cost: BTreeMap<XorName, usize>,
    pub(super) n_errors_during_pay: BTreeMap<XorName, usize>,
    pub(super) n_errors_during_uploads: BTreeMap<XorName, usize>,

    // error trackers
    pub(super) get_store_cost_errors: usize,
    pub(super) make_payments_errors: usize,

    // Upload stats
    pub(super) upload_storage_cost: NanoTokens,
    pub(super) upload_royalty_fees: NanoTokens,
    pub(super) upload_final_balance: NanoTokens,
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
    pub(super) fn new(files_api: FilesApi) -> Self {
        Self {
            batch_size: BATCH_SIZE,
            verify_store: true,
            show_holders: false,
            retry_strategy: RetryStrategy::Balanced,
            max_repayments_for_failed_data: MAX_REPAYMENTS_PER_FAILED_ITEM,
            api: files_api,

            all_upload_items: Default::default(),
            pending_to_get_register: Default::default(),
            pending_to_push_register: Default::default(),
            pending_to_get_store_cost: Default::default(),
            pending_to_pay: Default::default(),
            pending_to_upload: Default::default(),

            on_going_get_register: Default::default(),
            on_going_push_register: Default::default(),
            on_going_get_cost: Default::default(),
            on_going_payments: Default::default(),
            on_going_uploads: Default::default(),

            n_errors_during_push_register: Default::default(),
            n_errors_during_get_cost: Default::default(),
            n_errors_during_pay: Default::default(),
            n_errors_during_uploads: Default::default(),

            get_store_cost_errors: Default::default(),
            make_payments_errors: Default::default(),
            upload_storage_cost: NanoTokens::zero(),
            upload_royalty_fees: NanoTokens::zero(),
            upload_final_balance: NanoTokens::zero(),
            uploaded_count: Default::default(),
            skipped_count: Default::default(),
            testing_task_channels: None,
            logged_event_sender_absence: Default::default(),
            event_sender: Default::default(),
        }
    }

    // helpers to pop items from the pending/failed states.
    fn pop_item_for_push_register(&mut self) -> Result<UploadItem> {
        if let Some(name) = self.pending_to_push_register.pop() {
            let upload_item = self
                .all_upload_items
                .get(&name)
                .cloned()
                .ok_or(ClientError::UploadableItemNotFound(name))?;
            Ok(upload_item)
        } else {
            // the caller will be making sure this does not happen.
            Err(ClientError::UploadStateTrackerIsEmpty)
        }
    }

    // helpers to pop items from the pending/failed states.
    fn pop_item_for_get_store_cost(&mut self) -> Result<(UploadItem, GetStoreCostStrategy)> {
        if let Some((name, strategy)) = self.pending_to_get_store_cost.pop() {
            let upload_item = self
                .all_upload_items
                .get(&name)
                .cloned()
                .ok_or(ClientError::UploadableItemNotFound(name))?;
            Ok((upload_item, strategy))
        } else {
            // the caller will be making sure this does not happen.
            Err(ClientError::UploadStateTrackerIsEmpty)
        }
    }

    fn pop_item_for_make_payment(&mut self) -> Result<(UploadItem, Box<PayeeQuote>)> {
        if let Some((name, quote)) = self.pending_to_pay.pop() {
            let upload_item = self
                .all_upload_items
                .get(&name)
                .cloned()
                .ok_or(ClientError::UploadableItemNotFound(name))?;
            Ok((upload_item, quote))
        } else {
            // the caller will be making sure this does not happen.
            Err(ClientError::UploadStateTrackerIsEmpty)
        }
    }

    fn pop_item_for_upload_item(&mut self) -> Result<UploadItem> {
        if let Some(name) = self.pending_to_upload.pop() {
            let upload_item = self
                .all_upload_items
                .get(&name)
                .cloned()
                .ok_or(ClientError::UploadableItemNotFound(name))?;
            Ok(upload_item)
        } else {
            // the caller will be making sure this does not happen.
            Err(ClientError::UploadStateTrackerIsEmpty)
        }
    }

    fn emit_upload_event(&mut self, event: UploadEvent) {
        if let Some(sender) = self.event_sender.as_ref() {
            let sender_clone = sender.clone();
            let _handle = tokio::spawn(async move {
                if let Err(err) = sender_clone.send(event).await {
                    error!("Error emitting upload event: {err:?}");
                }
            });
        } else if !self.logged_event_sender_absence {
            info!("FilesUpload upload event sender is not set. Use get_upload_events() if you need to keep track of the progress");
            self.logged_event_sender_absence = true;
        }
    }

    async fn get_register(client: Client, reg_addr: RegisterAddress) -> Result<Register> {
        let reg = client.verify_register_stored(reg_addr).await?;
        let reg = reg.register()?;
        Ok(reg)
    }

    async fn push_register(upload_item: UploadItem, verify_store: bool) -> Result<ClientRegister> {
        let mut reg = if let UploadItem::Register { reg, .. } = upload_item {
            reg
        } else {
            return Err(ClientError::InvalidUploadItemFound);
        };
        reg.push(verify_store).await?;
        Ok(reg)
    }

    async fn get_store_cost(
        files_api: FilesApi,
        upload_item: UploadItem,
        get_store_cost_strategy: GetStoreCostStrategy,
    ) -> Result<PayeeQuote> {
        let address = upload_item.address();

        let filter_list = match get_store_cost_strategy {
            GetStoreCostStrategy::Cheapest => vec![],
            GetStoreCostStrategy::SelectDifferentPayee { max_repayments } => {
                // Check if we have already made payment for the provided xorname. If so filter out those payee
                let wallet = files_api.wallet()?;
                // todo: should we get non_expired here? maybe have a flag.
                let all_payments = wallet.get_all_payments_for_addr(&address, true)?;

                // if we have already made initial + max_repayments, then we should error out.
                if all_payments.len() > max_repayments + 1 {
                    return Err(ClientError::MaximumRepaymentsReached(upload_item.xorname()));
                }
                let filter_list = all_payments
                    .into_iter()
                    .map(|(_, peer_id)| peer_id)
                    .collect();
                debug!("Filtering out payments from {filter_list:?} during get_store_cost for {address:?}");
                filter_list
            }
        };
        let quote = files_api
            .client
            .network
            .get_store_costs_from_network(address, filter_list)
            .await?;
        Ok(quote)
    }

    // This is spawned as a long running task to prevent us from reading the wallet files
    // each time we have to make a payment.
    fn spawn_paying_thread(
        &self,
        mut paying_work_receiver: mpsc::Receiver<Option<(UploadItem, Box<PayeeQuote>)>>,
        task_result_sender: mpsc::Sender<TaskResult>,
        batch_size: usize,
    ) {
        let files_api = self.api.clone();
        let verify_store = self.verify_store;
        let _handle = tokio::spawn(async move {
            debug!("Spawning the long running payment thread.");
            let mut wallet_client = match files_api.wallet() {
                Ok(wallet) => wallet,
                Err(err) => {
                    error!("Failed to open wallet when handling {err:?}");
                    let _ = task_result_sender
                        .send(TaskResult::FailedToAccessWallet)
                        .await;
                    return;
                }
            };
            let mut cost_map = BTreeMap::new();
            let mut current_batch = vec![];

            while let Some(payment) = paying_work_receiver.recv().await {
                let make_payments = if let Some((item, quote)) = payment {
                    let xorname = item.xorname();
                    trace!("Inserted {xorname:?} into cost_map");

                    current_batch.push((xorname, quote.clone()));
                    let _ = cost_map.insert(xorname, (quote.1, quote.2, quote.0.to_bytes()));
                    cost_map.len() >= batch_size
                } else {
                    // using None to indicate as all paid.
                    let make_payments = !cost_map.is_empty();
                    trace!("Got a forced forced round of make payment. make_payments: {make_payments:?}");
                    make_payments
                };

                if make_payments {
                    let result = match wallet_client.pay_for_records(&cost_map, verify_store).await
                    {
                        Ok((storage_cost, royalty_fees)) => {
                            let paid_xornames = std::mem::take(&mut current_batch);
                            let paid_xornames = paid_xornames
                                .into_iter()
                                .map(|(xorname, _)| xorname)
                                .collect::<Vec<_>>();
                            trace!(
                                "Made payments for {} records: hash({:?})",
                                cost_map.len(),
                                Self::hash_of_xornames(paid_xornames.iter())
                            );
                            TaskResult::MakePaymentsOk {
                                paid_xornames,
                                storage_cost,
                                royalty_fees,
                                new_balance: wallet_client.balance(),
                            }
                        }
                        Err(err) => {
                            let paid_xornames = std::mem::take(&mut current_batch);
                            error!(
                                "When paying {} data: hash({:?}) got error {err:?}",
                                paid_xornames.len(),
                                Self::hash_of_xornames(paid_xornames.iter().map(|(name, _)| name))
                            );

                            TaskResult::MakePaymentsErr(paid_xornames)
                        }
                    };
                    let pay_for_chunk_sender_clone = task_result_sender.clone();
                    let _handle = tokio::spawn(async move {
                        let _ = pay_for_chunk_sender_clone.send(result).await;
                    });

                    cost_map = BTreeMap::new();
                }
            }
            debug!("Paying thread terminated");
        });
    }

    async fn upload_item(
        files_api: FilesApi,
        upload_item: UploadItem,
        verify_store: bool,
        retry_strategy: RetryStrategy,
    ) -> Result<()> {
        match upload_item {
            UploadItem::Chunk { address, chunk } => {
                let chunk = match chunk {
                    Either::Left(chunk) => chunk,
                    Either::Right(path) => {
                        let bytes = std::fs::read(path)?;
                        Chunk::new(Bytes::from(bytes))
                    }
                };

                trace!("Client upload started for chunk: {:?}", address.xorname());
                files_api
                    .get_local_payment_and_upload_chunk(chunk, verify_store, Some(retry_strategy))
                    .await?;
                Ok(())
            }
            UploadItem::Register { address, reg } => {
                let network_address = NetworkAddress::from_register_address(*reg.address());
                let signature = files_api.client.sign(reg.register.bytes()?);

                trace!(
                    "Client upload started for register: {:?}",
                    address.xorname()
                );
                let wallet_client = files_api.wallet()?;
                let (payment, payee) =
                    wallet_client.get_recent_payment_for_addr(&network_address, true)?;

                trace!(
                    "Payments for register: {:?} to {payee:?}:  {payment:?}. Now uploading it.",
                    address.xorname()
                );

                ClientRegister::publish_register(
                    files_api.client.clone(),
                    RegisterCmd::Create {
                        register: reg.register,
                        signature,
                    },
                    Some((payment, payee)),
                    verify_store,
                )
                .await?;

                Ok(())
            }
        }
    }

    // Used to debug a list of xornames.
    fn hash_of_xornames<'a>(xornames: impl Iterator<Item = &'a XorName>) -> String {
        let mut output = [0; 32];
        let mut hasher = Sha3::v256();
        for xorname in xornames {
            hasher.update(xorname);
        }
        hasher.finalize(&mut output);

        hex::encode(output)
    }
}
