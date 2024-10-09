// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![cfg(target_arch = "wasm32")]

use std::time::Duration;

use autonomi::Client;
use sn_networking::target_arch::sleep;
use wasm_bindgen_test::*;

mod common;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn put() -> Result<(), Box<dyn std::error::Error>> {
    common::enable_logging_wasm("sn_networking,autonomi,wasm");

    let client = Client::connect(&common::peers_from_run_or_compile_time_env()?)
        .await
        .unwrap();
    let wallet = test_utils::evm::get_funded_wallet();

    let data = common::gen_random_data(1024 * 1024 * 2); // 2MiB
    let addr = client.put(data.clone(), &wallet).await.unwrap();

    sleep(Duration::from_secs(2)).await;

    let data_fetched = client.get(addr).await.unwrap();
    assert_eq!(data, data_fetched, "data fetched should match data put");

    Ok(())
}
