use std::time::Duration;

use crate::common::evm_network_from_env;
use autonomi::{Client, Wallet};
use tokio::time::sleep;

mod common;

#[tokio::test]
async fn put() {
    common::enable_logging();

    let network = evm_network_from_env();
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let mut client = Client::connect(&[]).await.unwrap();
    let mut wallet = Wallet::new_from_private_key(network, private_key).unwrap();

    let data = common::gen_random_data(1024 * 1024 * 10);

    // let quote = client.quote(data.clone()).await.unwrap();
    // let payment = client.pay(quote, &mut wallet).await.unwrap();
    let addr = client.put(data.clone(), &mut wallet).await.unwrap();

    sleep(Duration::from_secs(2)).await;

    let data_fetched = client.get(addr).await.unwrap();
    assert_eq!(data, data_fetched, "data fetched should match data put");
}
