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
use test_utils::{evm::get_funded_wallet, gen_random_data, peers_from_env};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn put() -> Result<(), Box<dyn std::error::Error>> {
    enable_logging_wasm("sn_networking,autonomi,wasm");

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();
    let data = gen_random_data(1024 * 1024 * 10);

    let addr = client.data_put(data.clone(), wallet.into()).await?;

    sleep(Duration::from_secs(10)).await;

    let data_fetched = client.data_get(addr).await?;
    assert_eq!(data, data_fetched, "data fetched should match data put");

    Ok(())
}

fn enable_logging_wasm(directive: impl AsRef<str>) {
    use tracing_subscriber::prelude::*;

    console_error_panic_hook::set_once();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // Only partially supported across browsers
        .without_time() // std::time is not available in browsers
        .with_writer(tracing_web::MakeWebConsoleWriter::new()); // write events to the console
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::new(directive))
        .init();
}
