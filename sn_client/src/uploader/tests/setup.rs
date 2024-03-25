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
use crate::{Client, Result as ClientResult, UploadStats};
use bls::SecretKey;
use eyre::Result;
use libp2p::PeerId;
use libp2p_identity::Keypair;
use rand::thread_rng;
use sn_networking::{NetworkBuilder, PayeeQuote};
use sn_protocol::storage::RetryStrategy;
use sn_registers::{Register, RegisterAddress};
use sn_transfers::{MainSecretKey, NanoTokens, PaymentQuote};
use std::{
    collections::{BTreeMap, VecDeque},
    env::temp_dir,
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
    batch_size: usize,
}

impl UploaderInterface for TestUploader {
    fn take_inner_uploader(&mut self) -> InnerUploader {
        self.inner.take().unwrap()
    }

    fn spawn_get_register(
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

    fn spawn_push_register(
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

    fn spawn_get_store_cost(
        &mut self,
        _client: Client,
        _wallet_dir: PathBuf,
        upload_item: UploadItem,
        get_store_cost_strategy: GetStoreCostStrategy,
        _task_result_sender: mpsc::Sender<TaskResult>,
    ) {
        let xorname = upload_item.xorname();
        let step = self
            .test_steps
            .pop_front()
            .expect("TestSteps are empty. Expected a GetStoreCost step.");
        let handle = Handle::current();
        let task_result_sender = self.task_result_sender.clone();

        println!("spawn_get_store_cost called for: {xorname:?}. Step to execute: {step:?}");
        info!("TEST: spawn_get_store_cost called for: {xorname:?}. Step to execute: {step:?}");
        match step {
            TestSteps::GetStoreCostOk { zero_cost } => {
                let mut quote = PaymentQuote::zero();
                if !zero_cost {
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
                max_repayments_reached,
            } => {
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

    fn spawn_make_payment(
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
        let make_payment = self.make_payment_collector.len() >= self.batch_size
            || (to_send.is_none() && !self.make_payment_collector.is_empty());

        match step {
            TestSteps::MakePaymentJustCollectItem => {
                // The test expected for us to just collect item, but if the logic wants us to make payment, then it as
                // error
                assert!(!make_payment);
            }
            TestSteps::MakePaymentOk => {
                let paid_xornames = std::mem::take(&mut self.make_payment_collector)
                    .into_iter()
                    .map(|(xorname, _)| xorname)
                    .collect();
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
                let failed_list = std::mem::take(&mut self.make_payment_collector);

                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::MakePaymentsErr(failed_list))
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected MakePayment step. Got: {con:?}"),
        }
    }

    fn spawn_upload_item(
        &mut self,
        upload_item: UploadItem,
        _client: Client,
        _wallet_dir: PathBuf,
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
            TestSteps::UploadItemErr {
                trigger_quote_expired,
            } => {
                handle.spawn(async move {
                    task_result_sender
                        .send(TaskResult::UploadErr {
                            xorname,
                            quote_expired: trigger_quote_expired,
                        })
                        .await
                        .expect("Failed to send task result");
                });
            }
            con => panic!("Test failed: Expected UploadItem step. Got: {con:?}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TestSteps {
    GetRegisterOk,
    GetRegisterErr,
    PushRegisterOk,
    PushRegisterErr,
    GetStoreCostOk { zero_cost: bool },
    GetStoreCostErr { max_repayments_reached: bool },
    MakePaymentJustCollectItem,
    MakePaymentOk,
    MakePaymentErr,
    UploadItemOk,
    UploadItemErr {
        trigger_quote_expired: bool,
    },
}

pub fn get_inner_uploader() -> Result<(InnerUploader, mpsc::Sender<TaskResult>)> {
    let client = build_unconnected_client()?;

    let mut inner = InnerUploader::new(client, temp_dir());
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
    JoinHandle<ClientResult<UploadStats>>,
    JoinHandle<Vec<UploadEvent>>,
) {
    let batch_size = inner_uploader.batch_size;
    let mut upload_event_rx = inner_uploader.get_event_receiver();

    let upload_handle = tokio::spawn(start_upload(Box::new(TestUploader {
        inner: Some(inner_uploader),
        test_steps,
        task_result_sender,
        make_payment_collector: Default::default(),
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
pub fn build_unconnected_client() -> Result<Client> {
    let network_builder = NetworkBuilder::new(Keypair::generate_ed25519(), true, temp_dir());
    let (network, ..) = network_builder.build_client()?;
    let client = Client {
        network: network.clone(),
        events_broadcaster: Default::default(),
        signer: Arc::new(SecretKey::random()),
    };
    Ok(client)
}

// We don't perform any networking, so the paths can be dummy ones.
pub fn get_dummy_chunk_paths(num: usize) -> Vec<(XorName, PathBuf)> {
    let path = temp_dir();
    let mut rng = thread_rng();
    let mut chunks = Vec::with_capacity(num);
    for _ in 0..num {
        chunks.push((XorName::random(&mut rng), path.clone()));
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
