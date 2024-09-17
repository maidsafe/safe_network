use std::time::Duration;

use crate::common::evm_network_from_env;
use autonomi::{Client, Wallet};
use tokio::time::sleep;

mod common;

#[tokio::test]
async fn file() -> Result<(), Box<dyn std::error::Error>> {
    common::enable_logging();

    let network = evm_network_from_env();
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let mut client = Client::connect(&[]).await?;
    let mut wallet = Wallet::new_from_private_key(network, private_key).unwrap();

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
