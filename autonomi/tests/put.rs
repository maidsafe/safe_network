use std::time::Duration;

use crate::common::{deployer_wallet_from_network, evm_network_from_env};
use autonomi::Client;
use tokio::time::sleep;

mod common;

#[tokio::test]
async fn put() {
    common::enable_logging();

    let network = evm_network_from_env();
    let mut client = Client::connect(&[]).await.unwrap();
    let mut wallet = deployer_wallet_from_network(network);

    let data = common::gen_random_data(1024 * 1024 * 10);

    // let quote = client.quote(data.clone()).await.unwrap();
    // let payment = client.pay(quote, &mut wallet).await.unwrap();
    let addr = client.put(data.clone(), &mut wallet).await.unwrap();

    sleep(Duration::from_secs(2)).await;

    let data_fetched = client.get(addr).await.unwrap();
    assert_eq!(data, data_fetched, "data fetched should match data put");
}
