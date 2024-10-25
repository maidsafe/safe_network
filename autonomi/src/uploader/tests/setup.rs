// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    client::registers::Register,
    uploader::{
        upload::{start_upload, InnerUploader},
        GetStoreCostStrategy, TaskResult, UploadError, UploadEvent, UploadItem, UploadSummary,
        UploaderInterface,
    },
    Client,
};
use alloy::{primitives::TxHash, signers::local::PrivateKeySigner};
use assert_matches::assert_matches;
use bls::SecretKey as BlsSecretKey;
use eyre::Result;
use libp2p::{identity::Keypair, PeerId};
use rand::thread_rng;
use sn_evm::{EvmNetwork, EvmWallet, PaymentQuote, ProofOfPayment};
use sn_networking::{NetworkBuilder, PayeeQuote};
use sn_protocol::{storage::RetryStrategy, NetworkAddress};
use sn_registers::{RegisterAddress, SignedRegister};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
};
use tokio::{runtime::Handle, sync::mpsc, task::JoinHandle};
use xor_name::XorName;

struct TestUploader {
    inner: Option<InnerUploader>,
    test_steps: VecDeque<TestSteps>,
    task_result_sender: mpsc::Sender<TaskResult>,

    // test states
    make_payment_collector: Vec<(XorName, Box<PayeeQuote>)>,
    payments_made_per_xorname: BTreeMap<XorName, usize>,
    payment_batch_size: usize,
    register_sk: BlsSecretKey,
}

impl UploaderInterface for TestUploader {
    fn take_inner_uploader(&mut self) -> InnerUploader {
        self.inner.take().unwrap()
    }

    fn submit_get_register_task(
        &mut self,
        _client: Client,
        reg_addr: RegisterAddress,
        _task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = reg_addr.xorname();
        let step = self
            .test_steps
            .pop_front()
            .expect("TestSteps are empty. Expected a GetRegister step.");
        let handle = Handle::current();
        let register_sk = self.register_sk.clone();
        let task_result_sender = self.task_result_sender.clone();

        println!("spawn_get_register called for: {xorname:?}. Step to execute: {step:?}");
        info!("TEST: spawn_get_register called for: {xorname:?}. Step to execute: {step:?}");
        match step {
            TestSteps::GetRegisterOk => {
                handle.spawn(async move {
                    let remote_register =
                        SignedRegister::test_new_from_address(reg_addr, &register_sk);
                    let remote_register = Register::test_new_from_register(remote_register);
                    task_result_sender
                        .send(TaskResult::GetRegisterFromNetworkOk { remote_register })
                        .await
                        .expect("Failed to send task result");
                });
            }
            TestSteps::GetRegisterErr => {
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::GetRegisterFromNetworkErr(xorname))
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected GetRegister step. Got: {con:?}"),
        }
    }

    fn submit_push_register_task(
        &mut self,
        _client: Client,
        upload_item: UploadItem,
        _verify_store: bool,
        _task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = upload_item.xorname();
        let step = self
            .test_steps
            .pop_front()
            .expect("TestSteps are empty. Expected a PushRegister step.");
        let handle = Handle::current();
        let task_result_sender = self.task_result_sender.clone();

        println!("spawn_push_register called for: {xorname:?}. Step to execute: {step:?}");
        info!("TEST: spawn_push_register called for: {xorname:?}. Step to execute: {step:?}");
        match step {
            TestSteps::PushRegisterOk => {
                handle.spawn(async move {
                    let updated_register = match upload_item {
                        UploadItem::Register { reg, .. } => reg,
                        _ => panic!("Expected UploadItem::Register"),
                    };
                    task_result_sender
                        .send(TaskResult::PushRegisterOk {
                            // this register is just used for returning.
                            updated_register,
                        })
                        .await
                        .expect("Failed to send task result");
                });
            }
            TestSteps::PushRegisterErr => {
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::PushRegisterErr(xorname))
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected PushRegister step. Got: {con:?}"),
        }
    }

    fn submit_get_store_cost_task(
        &mut self,
        _client: Client,
        xorname: XorName,
        _address: NetworkAddress,
        _previous_payments: Option<&Vec<ProofOfPayment>>,
        get_store_cost_strategy: GetStoreCostStrategy,
        max_repayments_for_failed_data: usize,
        _task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let step = self
            .test_steps
            .pop_front()
            .expect("TestSteps are empty. Expected a GetStoreCost step.");
        let handle = Handle::current();
        let task_result_sender = self.task_result_sender.clone();

        println!("spawn_get_store_cost called for: {xorname:?}. Step to execute: {step:?}");
        info!("TEST: spawn_get_store_cost called for: {xorname:?}. Step to execute: {step:?}");

        let has_max_payments_reached_closure =
            |get_store_cost_strategy: &GetStoreCostStrategy| -> bool {
                match get_store_cost_strategy {
                    GetStoreCostStrategy::SelectDifferentPayee => {
                        if let Some(n_payments) = self.payments_made_per_xorname.get(&xorname) {
                            InnerUploader::have_we_reached_max_repayments(
                                *n_payments,
                                max_repayments_for_failed_data,
                            )
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            };

        // if select different payee, then it can possibly error out if max_repayments have been reached.
        // then the step should've been a GetStoreCostErr.
        if has_max_payments_reached_closure(&get_store_cost_strategy) {
            assert_matches!(step, TestSteps::GetStoreCostErr { .. }, "Max repayments have been reached, so we expect a GetStoreCostErr, not GetStoreCostOk");
        }

        match step {
            TestSteps::GetStoreCostOk {
                trigger_zero_cost,
                assert_select_different_payee,
            } => {
                // Make sure that the received strategy is the one defined in the step.
                assert!(match get_store_cost_strategy {
                    // match here to not miss out on any new strategies.
                    GetStoreCostStrategy::Cheapest => !assert_select_different_payee,
                    GetStoreCostStrategy::SelectDifferentPayee { .. } =>
                        assert_select_different_payee,
                });

                let mut quote = PaymentQuote::zero();
                if !trigger_zero_cost {
                    quote.cost = 1.into();
                }
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::GetStoreCostOk {
                            xorname,
                            quote: Box::new((
                                PeerId::random(),
                                PrivateKeySigner::random().address(),
                                quote,
                            )),
                        })
                        .await
                        .expect("Failed to send task result");
                });
            }
            TestSteps::GetStoreCostErr {
                assert_select_different_payee,
            } => {
                // Make sure that the received strategy is the one defined in the step.
                assert!(match get_store_cost_strategy {
                    // match here to not miss out on any new strategies.
                    GetStoreCostStrategy::Cheapest => !assert_select_different_payee,
                    GetStoreCostStrategy::SelectDifferentPayee { .. } =>
                        assert_select_different_payee,
                });
                let max_repayments_reached =
                    has_max_payments_reached_closure(&get_store_cost_strategy);

                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::GetStoreCostErr {
                            xorname,
                            get_store_cost_strategy,
                            max_repayments_reached,
                        })
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected GetStoreCost step. Got: {con:?}"),
        }
    }

    fn submit_make_payment_task(
        &mut self,
        to_send: Option<(UploadItem, Box<PayeeQuote>)>,
        _make_payment_sender: mpsc::Sender<Option<(UploadItem, Box<PayeeQuote>)>>,
    ) {
        let step = self
            .test_steps
            .pop_front()
            .expect("TestSteps are empty. Expected a MakePayment step.");
        let handle = Handle::current();
        let task_result_sender = self.task_result_sender.clone();
        match &to_send {
            Some((upload_item, quote)) => {
                let xorname = upload_item.xorname();
                println!("spawn_make_payment called for: {xorname:?}. Step to execute: {step:?}");
                info!(
                    "TEST: spawn_make_payment called for: {xorname:?}. Step to execute: {step:?}"
                );

                self.make_payment_collector
                    .push((upload_item.xorname(), quote.clone()));
            }
            None => {
                println!(
                    "spawn_make_payment called with force make payment. Step to execute: {step:?}"
                );
                info!("TEST: spawn_make_payment called with force make payment. Step to execute: {step:?}");
            }
        }

        // gotta collect batch size before sending task result.
        let _make_payment = self.make_payment_collector.len() >= self.payment_batch_size
            || (to_send.is_none() && !self.make_payment_collector.is_empty());

        match step {
            // TestSteps::MakePaymentJustCollectItem => {
            //     // The test expected for us to just collect item, but if the logic wants us to make payment, then it as
            //     // error
            //     assert!(!make_payment);
            // }
            TestSteps::MakePaymentOk => {
                let payment_proofs = std::mem::take(&mut self.make_payment_collector)
                    .into_iter()
                    .map(|(xorname, _)| {
                        (
                            xorname,
                            ProofOfPayment {
                                quote: PaymentQuote::zero(),
                                tx_hash: TxHash::repeat_byte(0),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>();
                // track the payments per xorname
                for xorname in payment_proofs.keys() {
                    let entry = self.payments_made_per_xorname.entry(*xorname).or_insert(0);
                    *entry += 1;
                }

                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::MakePaymentsOk { payment_proofs })
                        .await
                        .expect("Failed to send task result");
                });
            }
            TestSteps::MakePaymentErr => {
                let failed_xornames = std::mem::take(&mut self.make_payment_collector);

                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::MakePaymentsErr { failed_xornames })
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected MakePayment step. Got: {con:?}"),
        }
    }

    fn submit_upload_item_task(
        &mut self,
        upload_item: UploadItem,
        _client: Client,
        _previous_payments: Option<&Vec<ProofOfPayment>>,
        _verify_store: bool,
        _retry_strategy: RetryStrategy,
        _task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = upload_item.xorname();
        let step = self
            .test_steps
            .pop_front()
            .expect("TestSteps are empty. Expected a UploadItem step.");
        let handle = Handle::current();
        let task_result_sender = self.task_result_sender.clone();

        println!("spawn_upload_item called for: {xorname:?}. Step to execute: {step:?}");
        info!("TEST: spawn_upload_item called for: {xorname:?}. Step to execute: {step:?}");
        match step {
            TestSteps::UploadItemOk => {
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::UploadOk(xorname))
                        .await
                        .expect("Failed to send task result");
                });
            }
            TestSteps::UploadItemErr { io_error } => {
                handle.spawn(async move {
                    let io_error = if io_error {
                        Some(Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Test IO Error",
                        )))
                    } else {
                        None
                    };
                    task_result_sender
                        .send(TaskResult::UploadErr { xorname, io_error })
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected UploadItem step. Got: {con:?}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TestSteps {
    GetRegisterOk,
    GetRegisterErr,
    PushRegisterOk,
    PushRegisterErr,
    GetStoreCostOk {
        trigger_zero_cost: bool,
        assert_select_different_payee: bool,
    },
    GetStoreCostErr {
        assert_select_different_payee: bool,
    },
    // MakePaymentJustCollectItem,
    MakePaymentOk,
    MakePaymentErr,
    UploadItemOk,
    UploadItemErr {
        io_error: bool,
    },
}

pub fn get_inner_uploader() -> Result<(InnerUploader, mpsc::Sender<TaskResult>)> {
    let client = build_unconnected_client()?;

    let mut inner = InnerUploader::new(
        client,
        EvmWallet::new_with_random_wallet(EvmNetwork::new_custom(
            "http://localhost:63319/",
            "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "0x8464135c8F25Da09e49BC8782676a84730C318bC",
        ))
        .into(),
    );
    let (task_result_sender, task_result_receiver) = mpsc::channel(100);
    inner.testing_task_channels = Some((task_result_sender.clone(), task_result_receiver));

    Ok((inner, task_result_sender))
}

// Spawns two tasks. One is the actual upload task that will return an UploadStat when completed.
// The other is a one to collect all the UploadEvent emitted by the previous task.
pub fn start_uploading_with_steps(
    mut inner_uploader: InnerUploader,
    test_steps: VecDeque<TestSteps>,
    register_sk: BlsSecretKey,
    task_result_sender: mpsc::Sender<TaskResult>,
) -> (
    JoinHandle<Result<UploadSummary, UploadError>>,
    JoinHandle<Vec<UploadEvent>>,
) {
    let payment_batch_size = inner_uploader.cfg.payment_batch_size;
    let mut upload_event_rx = inner_uploader.get_event_receiver();

    let upload_handle = tokio::spawn(start_upload(Box::new(TestUploader {
        inner: Some(inner_uploader),
        test_steps,
        task_result_sender,
        make_payment_collector: Default::default(),
        payments_made_per_xorname: Default::default(),
        payment_batch_size,
        register_sk,
    })));

    let event_handle = tokio::spawn(async move {
        let mut events = vec![];
        while let Some(event) = upload_event_rx.recv().await {
            events.push(event);
        }
        events
    });

    (upload_handle, event_handle)
}

// Collect all the upload events into a list

// Build a very simple client struct for testing. This does not connect to any network.
// The UploaderInterface eliminates the need for direct networking in tests.
pub fn build_unconnected_client() -> Result<Client> {
    let network_builder = NetworkBuilder::new(Keypair::generate_ed25519(), true);
    let (network, ..) = network_builder.build_client()?;
    let client = Client {
        network,
        client_event_sender: Arc::new(None),
    };
    Ok(client)
}

// We don't perform any networking, so the paths can be dummy ones.
pub fn get_dummy_chunk_paths(num: usize, temp_dir: PathBuf) -> Vec<(XorName, PathBuf)> {
    let mut rng = thread_rng();
    let mut chunks = Vec::with_capacity(num);
    for _ in 0..num {
        chunks.push((XorName::random(&mut rng), temp_dir.clone()));
    }
    chunks
}

pub fn get_dummy_registers(num: usize, register_sk: &BlsSecretKey) -> Vec<Register> {
    let mut rng = thread_rng();
    let mut registers = Vec::with_capacity(num);
    for _ in 0..num {
        // test_new_from_address that is used during get_register,
        // uses AnyoneCanWrite permission, so use the same here
        let address = RegisterAddress::new(XorName::random(&mut rng), register_sk.public_key());
        let base_register = SignedRegister::test_new_from_address(address, register_sk);
        let register = Register::test_new_from_register(base_register);
        registers.push(register);
    }
    registers
}
