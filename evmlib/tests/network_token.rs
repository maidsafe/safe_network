mod common;

use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::U256;
use alloy::providers::fillers::{
    BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy::providers::{Identity, ReqwestProvider, WalletProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};
use evmlib::contract::network_token::NetworkToken;
use evmlib::testnet::{deploy_network_token_contract, start_node};
use evmlib::wallet::wallet_address;
use std::str::FromStr;

async fn setup() -> (
    AnvilInstance,
    NetworkToken<
        Http<Client>,
        FillProvider<
            JoinFill<
                JoinFill<
                    Identity,
                    JoinFill<
                        GasFiller,
                        JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>,
                    >,
                >,
                WalletFiller<EthereumWallet>,
            >,
            ReqwestProvider,
            Http<Client>,
            Ethereum,
        >,
        Ethereum,
    >,
) {
    let (anvil, rpc_url) = start_node();

    let network_token = deploy_network_token_contract(&rpc_url, &anvil).await;

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

    let account = wallet_address(network_token.contract.provider().wallet());

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
