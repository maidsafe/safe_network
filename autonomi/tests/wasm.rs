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

    let client = Client::connect(&test_utils::peers_from_env()?)
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
