mod common;

use std::time::Duration;

use crate::common::{evm_network_from_env, evm_wallet_from_env_or_default};
use autonomi::Client;
use tokio::time::sleep;

#[cfg(feature = "data")]
#[tokio::test]
async fn put() {
    common::enable_logging();

    let network = evm_network_from_env();
    let mut client = Client::connect(&[]).await.unwrap();
    let wallet = evm_wallet_from_env_or_default(network);
    let data = common::gen_random_data(1024 * 1024 * 10);

    // let quote = client.quote(data.clone()).await.unwrap();
    // let payment = client.pay(quote, &mut wallet).await.unwrap();
    let addr = client.put(data.clone(), &wallet).await.unwrap();

    sleep(Duration::from_secs(10)).await;

    let data_fetched = client.get(addr).await.unwrap();
    assert_eq!(data, data_fetched, "data fetched should match data put");
}
