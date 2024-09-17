use std::time::Duration;

use crate::common::{deployer_wallet_from_network, evm_network_from_env};
use autonomi::Client;
use tokio::time::sleep;

mod common;

#[tokio::test]
async fn file() -> Result<(), Box<dyn std::error::Error>> {
    common::enable_logging();

    let network = evm_network_from_env();
    let mut client = Client::connect(&[]).await.unwrap();
    let mut wallet = deployer_wallet_from_network(network);

    // let data = common::gen_random_data(1024 * 1024 * 1000);
    // let user_key = common::gen_random_data(32);

    let (root, addr) = client
        .upload_from_dir("tests/file/test_dir".into(), &mut wallet)
        .await?;

    sleep(Duration::from_secs(10)).await;

    let root_fetched = client.fetch_root(addr).await?;

    assert_eq!(
        root.map, root_fetched.map,
        "root fetched should match root put"
    );

    Ok(())
}
