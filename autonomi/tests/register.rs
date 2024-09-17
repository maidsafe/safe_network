use std::time::Duration;

use crate::common::evm_network_from_env;
use autonomi::{Client, Wallet};
use bytes::Bytes;
use tokio::time::sleep;
use xor_name::XorName;

mod common;

#[tokio::test]
async fn register() {
    common::enable_logging();

    let network = evm_network_from_env();
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let mut client = Client::connect(&[]).await.unwrap();
    let mut wallet = Wallet::new_from_private_key(network, private_key).unwrap();

    // Owner key of the register.
    let key = bls::SecretKey::random();

    // Create a register with the value [1, 2, 3, 4]
    let register = client
        .create_register(
            vec![1, 2, 3, 4].into(),
            XorName::random(&mut rand::thread_rng()),
            key.clone(),
            &mut wallet,
        )
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;

    // Fetch the register again
    let register = client.fetch_register(*register.address()).await.unwrap();

    // Update the register with the value [5, 6, 7, 8]
    client
        .update_register(register.clone(), vec![5, 6, 7, 8].into(), key)
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;

    // Fetch and verify the register contains the updated value
    let register = client.fetch_register(*register.address()).await.unwrap();
    assert_eq!(register.values(), vec![Bytes::from(vec![5, 6, 7, 8])]);
}
