// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    uploader::{
        upload::{start_upload, InnerUploader},
        GetStoreCostStrategy, TaskResult, UploadItem, UploaderInterface,
    },
    ClientRegister, UploadEvent,
};
use crate::{Client, Result as ClientResult, UploadSummary};
use assert_matches::assert_matches;
use bls::SecretKey;
use eyre::Result;
use libp2p::PeerId;
use libp2p_identity::Keypair;
use rand::thread_rng;
use sn_networking::{NetworkBuilder, PayeeQuote};
use sn_protocol::{storage::RetryStrategy, NetworkAddress};
use sn_registers::{Register, RegisterAddress};
use sn_transfers::{MainSecretKey, NanoTokens, PaymentQuote, WalletApi};
use std::{
    collections::{BTreeMap, VecDeque},
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
    batch_size: usize,
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
        let task_result_sender = self.task_result_sender.clone();

        println!("spawn_get_register called for: {xorname:?}. Step to execute: {step:?}");
        info!("TEST: spawn_get_register called for: {xorname:?}. Step to execute: {step:?}");
        match step {
            TestSteps::GetRegisterOk => {
                handle.spawn(async move {
                    let reg = Register::test_new_from_address(reg_addr);

                    task_result_sender
                        .send(TaskResult::GetRegisterFromNetworkOk {
                            remote_register: reg,
                        })
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
        _wallet_api: WalletApi,
        xorname: XorName,
        _address: NetworkAddress,
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
                    quote.cost = NanoTokens::from(10);
                }
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::GetStoreCostOk {
                            xorname,
                            quote: Box::new((
                                PeerId::random(),
                                MainSecretKey::random().main_pubkey(),
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
        let _make_payment = self.make_payment_collector.len() >= self.batch_size
            || (to_send.is_none() && !self.make_payment_collector.is_empty());

        match step {
            // TestSteps::MakePaymentJustCollectItem => {
            //     // The test expected for us to just collect item, but if the logic wants us to make payment, then it as
            //     // error
            //     assert!(!make_payment);
            // }
            TestSteps::MakePaymentOk => {
                let paid_xornames = std::mem::take(&mut self.make_payment_collector)
                    .into_iter()
                    .map(|(xorname, _)| xorname)
                    .collect::<Vec<_>>();
                // track the payments per xorname
                for xorname in paid_xornames.iter() {
                    let entry = self.payments_made_per_xorname.entry(*xorname).or_insert(0);
                    *entry += 1;
                }
                let batch_size = self.batch_size;

                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::MakePaymentsOk {
                            paid_xornames,
                            storage_cost: NanoTokens::from(batch_size as u64 * 10),
                            royalty_fees: NanoTokens::from(batch_size as u64 * 3),
                            new_balance: NanoTokens::from(batch_size as u64 * 1000),
                        })
                        .await
                        .expect("Failed to send task result");
                });
            }
            TestSteps::MakePaymentErr => {
                let failed_xornames = std::mem::take(&mut self.make_payment_collector);

                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::MakePaymentsErr {
                            failed_xornames,
                            insufficient_balance: None,
                        })
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
        _wallet_api: WalletApi,
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
            TestSteps::UploadItemErr => {
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::UploadErr { xorname })
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
    UploadItemErr,
}

pub fn get_inner_uploader(root_dir: PathBuf) -> Result<(InnerUploader, mpsc::Sender<TaskResult>)> {
    let client = build_unconnected_client(root_dir.clone())?;

    let mut inner = InnerUploader::new(client, root_dir);
    let (task_result_sender, task_result_receiver) = mpsc::channel(100);
    inner.testing_task_channels = Some((task_result_sender.clone(), task_result_receiver));

    Ok((inner, task_result_sender))
}

// Spawns two tasks. One is the actual upload task that will return an UploadStat when completed.
// The other is a one to collect all the UploadEvent emitted by the previous task.
pub fn start_uploading_with_steps(
    mut inner_uploader: InnerUploader,
    test_steps: VecDeque<TestSteps>,
    task_result_sender: mpsc::Sender<TaskResult>,
) -> (
    JoinHandle<ClientResult<UploadSummary>>,
    JoinHandle<Vec<UploadEvent>>,
) {
    let batch_size = inner_uploader.cfg.batch_size;
    let mut upload_event_rx = inner_uploader.get_event_receiver();

    let upload_handle = tokio::spawn(start_upload(Box::new(TestUploader {
        inner: Some(inner_uploader),
        test_steps,
        task_result_sender,
        make_payment_collector: Default::default(),
        payments_made_per_xorname: Default::default(),
        batch_size,
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
pub fn build_unconnected_client(root_dir: PathBuf) -> Result<Client> {
    let network_builder = NetworkBuilder::new(Keypair::generate_ed25519(), true, root_dir);
    let (network, ..) = network_builder.build_client()?;
    let client = Client {
        network: network.clone(),
        events_broadcaster: Default::default(),
        signer: Arc::new(SecretKey::random()),
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

pub fn get_dummy_registers(num: usize, client: Client) -> Vec<ClientRegister> {
    let mut rng = thread_rng();
    let mut registers = Vec::with_capacity(num);
    for _ in 0..num {
        let mut client_reg = ClientRegister::create(client.clone(), XorName::random(&mut rng));
        // test_new_from_address that is used during get_register, uses AnyoneCanWrite permission, so use the same here
        client_reg.register = Register::test_new_from_address(*client_reg.address());

        registers.push(client_reg);
    }
    registers
}
