#![cfg(feature = "registers")]

mod common;

use autonomi::Client;
use bytes::Bytes;
use std::time::Duration;
use test_utils::evm::get_funded_wallet;
use tokio::time::sleep;
use xor_name::XorName;

#[tokio::test]
async fn register() {
    common::enable_logging();

    let client = Client::connect(&[]).await.unwrap();
    let wallet = get_funded_wallet();

    // Owner key of the register.
    let key = bls::SecretKey::random();

    // Create a register with the value [1, 2, 3, 4]
    let register = client
        .create_register(
            vec![1, 2, 3, 4].into(),
            XorName::random(&mut rand::thread_rng()),
            key.clone(),
            &wallet,
        )
        .await
        .unwrap();

    sleep(Duration::from_secs(10)).await;

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
