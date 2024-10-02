#![cfg(feature = "data")]

mod common;

use autonomi::Client;
use std::time::Duration;
use test_utils::evm::get_funded_wallet;
use tokio::time::sleep;

#[tokio::test]
async fn put() {
    common::enable_logging();

    let client = Client::connect(&[]).await.unwrap();
    let wallet = get_funded_wallet();
    let data = common::gen_random_data(1024 * 1024 * 10);

    let addr = client.put(data.clone(), &wallet).await.unwrap();

    sleep(Duration::from_secs(10)).await;

    let data_fetched = client.get(addr).await.unwrap();
    assert_eq!(data, data_fetched, "data fetched should match data put");
}
