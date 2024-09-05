mod common;

use crate::common::local_testnet::start_anvil_node;
use crate::common::network_token::deploy_network_token_contract;
use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::U256;
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ReqwestProvider, WalletProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};
use evmlib::contract::network_token::NetworkToken;
use std::str::FromStr;

async fn setup() -> (
    AnvilInstance,
    NetworkToken<
        Http<Client>,
        FillProvider<
            JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
            ReqwestProvider,
            Http<Client>,
            Ethereum,
        >,
        Ethereum,
    >,
) {
    let anvil = start_anvil_node()
        .await
        .expect("Could not start anvil node");

    let network_token = deploy_network_token_contract(&anvil)
        .await
        .expect("Could not deploy AutonomiNetworkToken contract");

    (anvil, network_token)
}

#[tokio::test]
async fn test_deploy() {
    setup().await;
}

#[tokio::test]
async fn test_balance_of() {
    let (_anvil, contract) = setup().await;

    let account = <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(
        contract.contract.provider().wallet(),
    );

    let balance = contract.balance_of(account).await.unwrap();

    assert_eq!(
        balance,
        U256::from_str("20000000000000000000000000").unwrap()
    );
}

#[tokio::test]
async fn test_approve() {
    let (_anvil, network_token) = setup().await;

    let account = <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(
        network_token.contract.provider().wallet(),
    );

    let spend_value = U256::from(1);
    let spender = PrivateKeySigner::random();

    // Approve for the spender to spend a value from the funds of the owner (our default account).
    let approval_result = network_token.approve(spender.address(), spend_value).await;

    assert!(
        approval_result.is_ok(),
        "Approval failed with error: {:?}",
        approval_result.err()
    );

    let allowance = network_token
        .contract
        .allowance(account, spender.address())
        .call()
        .await
        .unwrap()
        ._0;

    assert_eq!(allowance, spend_value);
}
