// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod setup;

use crate::{
    uploader::tests::setup::{
        get_dummy_chunk_paths, get_dummy_registers, get_inner_uploader, start_uploading_with_steps,
        TestSteps,
    },
    Error as ClientError, UploadEvent,
};
use assert_matches::assert_matches;
use eyre::Result;
use sn_logging::LogBuilder;
use std::collections::VecDeque;
use tempfile::tempdir;

// ===== HAPPY PATH =======

/// 1. Chunk: if cost =0, then chunk is present in the network.
#[tokio::test]
async fn chunk_that_already_exists_in_the_network_should_return_zero_store_cost() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_chunk_paths(get_dummy_chunk_paths(1, temp_dir.path().to_path_buf()));

    // the path to test
    let steps = vec![TestSteps::GetStoreCostOk {
        trigger_zero_cost: true,
        assert_select_different_payee: false,
    }];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    let _stats = upload_handle.await??;
    let events = events_handle.await?;

    assert_eq!(events.len(), 1);
    assert_matches!(events[0], UploadEvent::ChunkAlreadyExistsInNetwork(_));
    Ok(())
}

/// 2. Chunk: if cost !=0, then make payment upload to the network.
#[tokio::test]
async fn chunk_should_be_paid_for_and_uploaded_if_cost_is_not_zero() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_chunk_paths(get_dummy_chunk_paths(1, temp_dir.path().to_path_buf()));

    // the path to test
    let steps = vec![
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemOk,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    let _stats = upload_handle.await??;
    let events = events_handle.await?;

    assert_eq!(events.len(), 2);
    assert_matches!(events[0], UploadEvent::PaymentMade { .. });
    assert_matches!(events[1], UploadEvent::ChunkUploaded(..));
    Ok(())
}

/// 3. Register: if GET register = ok, then merge and push the register.
#[tokio::test]
async fn register_should_be_merged_and_pushed_if_it_already_exists_in_the_network() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_register(get_dummy_registers(1, inner_uploader.client.clone()));

    // the path to test
    let steps = vec![TestSteps::GetRegisterOk, TestSteps::PushRegisterOk];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    let _stats = upload_handle.await??;
    let events = events_handle.await?;

    assert_eq!(events.len(), 1);
    assert_matches!(events[0], UploadEvent::RegisterUpdated { .. });
    Ok(())
}

/// 4. Register: if Get register = err, then get store cost and upload.
#[tokio::test]
async fn register_should_be_paid_and_uploaded_if_it_does_not_exists() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_register(get_dummy_registers(1, inner_uploader.client.clone()));

    // the path to test
    // todo: what if cost = 0 even after GetRegister returns error. check that
    let steps = vec![
        TestSteps::GetRegisterErr,
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemOk,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    let _stats = upload_handle.await??;
    let events = events_handle.await?;

    assert_eq!(events.len(), 2);
    assert_matches!(events[0], UploadEvent::PaymentMade { .. });
    assert_matches!(events[1], UploadEvent::RegisterUploaded(..));
    Ok(())
}

// ===== REPAYMENTS ======

/// 1. Chunks: if upload task fails > threshold, then get store cost should be triggered with SelectDifferentStrategy
/// and then uploaded.
#[tokio::test]
async fn chunks_should_perform_repayment_if_the_upload_fails_multiple_times() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_chunk_paths(get_dummy_chunk_paths(1, temp_dir.path().to_path_buf()));

    // the path to test
    let steps = vec![
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemErr,
        TestSteps::UploadItemErr,
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: true,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemOk,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    let _stats = upload_handle.await??;
    let events = events_handle.await?;

    assert_eq!(events.len(), 3);
    assert_matches!(events[0], UploadEvent::PaymentMade { .. });
    assert_matches!(events[1], UploadEvent::PaymentMade { .. });
    assert_matches!(events[2], UploadEvent::ChunkUploaded(..));
    Ok(())
}

/// 2. Register: if upload task fails > threshold, then get store cost should be triggered with SelectDifferentStrategy
/// and then uploaded.
#[tokio::test]
async fn registers_should_perform_repayment_if_the_upload_fails_multiple_times() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_register(get_dummy_registers(1, inner_uploader.client.clone()));

    // the path to test
    let steps = vec![
        TestSteps::GetRegisterErr,
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemErr,
        TestSteps::UploadItemErr,
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: true,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemOk,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    let _stats = upload_handle.await??;
    let events = events_handle.await?;

    assert_eq!(events.len(), 3);
    assert_matches!(events[0], UploadEvent::PaymentMade { .. });
    assert_matches!(events[1], UploadEvent::PaymentMade { .. });
    assert_matches!(events[2], UploadEvent::RegisterUploaded(..));
    Ok(())
}

// ===== ERRORS =======
/// 1. Registers: Multiple PushRegisterErr should result in Error::SequentialNetworkErrors
#[tokio::test]
async fn register_upload_should_error_out_if_there_are_multiple_push_failures() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_register(get_dummy_registers(1, inner_uploader.client.clone()));

    // the path to test
    let steps = vec![
        TestSteps::GetRegisterOk,
        TestSteps::PushRegisterErr,
        TestSteps::PushRegisterErr,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    assert_matches!(
        upload_handle.await?,
        Err(ClientError::SequentialNetworkErrors)
    );
    let events = events_handle.await?;

    // UploadEvent::Error is performed by the caller of start_upload, so we can't check that one here.
    assert_eq!(events.len(), 0);
    Ok(())
}

/// 2. Chunk: Multiple errors during get store cost should result in Error::SequentialNetworkErrors
#[tokio::test]
async fn chunk_should_error_out_if_there_are_multiple_errors_during_get_store_cost() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_chunk_paths(get_dummy_chunk_paths(1, temp_dir.path().to_path_buf()));

    // the path to test
    let steps = vec![
        TestSteps::GetStoreCostErr {
            assert_select_different_payee: false,
        },
        TestSteps::GetStoreCostErr {
            assert_select_different_payee: false,
        },
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    assert_matches!(
        upload_handle.await?,
        Err(ClientError::SequentialNetworkErrors)
    );
    let events = events_handle.await?;

    // UploadEvent::Error is performed by the caller of start_upload, so we can't check that one here.
    assert_eq!(events.len(), 0);
    Ok(())
}

/// 3. Register: Multiple errors during get store cost should result in Error::SequentialNetworkErrors
#[tokio::test]
async fn register_should_error_out_if_there_are_multiple_errors_during_get_store_cost() -> Result<()>
{
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_register(get_dummy_registers(1, inner_uploader.client.clone()));

    // the path to test
    let steps = vec![
        TestSteps::GetRegisterErr,
        TestSteps::GetStoreCostErr {
            assert_select_different_payee: false,
        },
        TestSteps::GetStoreCostErr {
            assert_select_different_payee: false,
        },
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    assert_matches!(
        upload_handle.await?,
        Err(ClientError::SequentialNetworkErrors)
    );
    let events = events_handle.await?;

    // UploadEvent::Error is performed by the caller of start_upload, so we can't check that one here.
    assert_eq!(events.len(), 0);
    Ok(())
}

/// 4. Chunk: Multiple errors during make payment should result in Error::SequentialUploadPaymentError
#[tokio::test]
async fn chunk_should_error_out_if_there_are_multiple_errors_during_make_payment() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_chunk_paths(get_dummy_chunk_paths(1, temp_dir.path().to_path_buf()));

    // the path to test
    let steps = vec![
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentErr,
        TestSteps::MakePaymentErr,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    assert_matches!(
        upload_handle.await?,
        Err(ClientError::SequentialUploadPaymentError)
    );
    let events = events_handle.await?;

    // UploadEvent::Error is performed by the caller of start_upload, so we can't check that one here.
    assert_eq!(events.len(), 0);
    Ok(())
}

/// 5. Register: Multiple errors during make payment should result in Error::SequentialUploadPaymentError
#[tokio::test]
async fn register_should_error_out_if_there_are_multiple_errors_during_make_payment() -> Result<()>
{
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_register(get_dummy_registers(1, inner_uploader.client.clone()));

    // the path to test
    let steps = vec![
        TestSteps::GetRegisterErr,
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentErr,
        TestSteps::MakePaymentErr,
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    assert_matches!(
        upload_handle.await?,
        Err(ClientError::SequentialUploadPaymentError)
    );
    let events = events_handle.await?;

    // UploadEvent::Error is performed by the caller of start_upload, so we can't check that one here.
    assert_eq!(events.len(), 0);
    Ok(())
}

// 6: Chunks + Registers: if the number of repayments exceed a threshold, it should return MaximumRepaymentsReached error.
#[tokio::test]
async fn maximum_repayment_error_should_be_triggered_during_get_store_cost() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("uploader", true);
    let temp_dir = tempdir()?;
    let (mut inner_uploader, task_result_rx) = get_inner_uploader(temp_dir.path().to_path_buf())?;

    // cfg
    inner_uploader.set_batch_size(1);
    inner_uploader.insert_chunk_paths(get_dummy_chunk_paths(1, temp_dir.path().to_path_buf()));

    // the path to test
    let steps = vec![
        // initial payment done
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: false,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemErr,
        TestSteps::UploadItemErr,
        // first repayment
        TestSteps::GetStoreCostOk {
            trigger_zero_cost: false,
            assert_select_different_payee: true,
        },
        TestSteps::MakePaymentOk,
        TestSteps::UploadItemErr,
        TestSteps::UploadItemErr,
        // thus after reaching max repayments, we should error out during get store cost.
        TestSteps::GetStoreCostErr {
            assert_select_different_payee: true,
        },
    ];

    let (upload_handle, events_handle) =
        start_uploading_with_steps(inner_uploader, VecDeque::from(steps), task_result_rx);

    assert_matches!(
        upload_handle.await?,
        Err(ClientError::UploadFailedWithMaximumRepaymentsReached { .. })
    );
    let events = events_handle.await?;

    assert_eq!(events.len(), 2);
    assert_matches!(events[0], UploadEvent::PaymentMade { .. });
    assert_matches!(events[1], UploadEvent::PaymentMade { .. });
    Ok(())
}
