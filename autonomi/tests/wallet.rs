use crate::common::{evm_network_from_env, evm_wallet_from_env_or_default};
use autonomi::Wallet;
use const_hex::traits::FromHex;
use evmlib::common::{Address, Amount};

mod common;

#[tokio::test]
async fn from_private_key() {
    let private_key = "0xdb1049e76a813c94be0df47ec3e20533ca676b1b9fef2ddbce9daa117e4da4aa";
    let network = evm_network_from_env();
    let wallet = Wallet::new_from_private_key(network, private_key).unwrap();

    assert_eq!(
        wallet.address(),
        Address::from_hex("0x69D5BF2Bc42bca8782b8D2b4FdfF2b1Fa7644Fe7").unwrap()
    )
}

#[tokio::test]
async fn send_tokens() {
    let network = evm_network_from_env();
    let wallet = evm_wallet_from_env_or_default(network.clone());

    let receiving_wallet = Wallet::new_with_random_wallet(network);

    let initial_balance = receiving_wallet.balance_of_tokens().await.unwrap();

    assert_eq!(initial_balance, Amount::from(0));

    let _ = wallet
        .transfer_tokens(receiving_wallet.address(), Amount::from(10))
        .await;

    let final_balance = receiving_wallet.balance_of_tokens().await.unwrap();

    assert_eq!(final_balance, Amount::from(10));
}
