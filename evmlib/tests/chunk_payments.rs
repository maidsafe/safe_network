mod common;

use crate::common::quote::random_quote_payment;
use alloy::network::{Ethereum, EthereumWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::utils::parse_ether;
use alloy::providers::ext::AnvilApi;
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider, WalletProvider};
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use alloy::transports::http::{Client, Http};
use evmlib::common::U256;
use evmlib::contract::data_payments::{DataPayments, MAX_TRANSFERS_PER_TRANSACTION};
use evmlib::contract::network_token::NetworkToken;
use evmlib::testnet::{deploy_data_payments_contract, deploy_network_token_contract, start_node};
use evmlib::wallet::wallet_address;

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
    DataPayments<
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
    let anvil = start_node();

    let network_token = deploy_network_token_contract(&anvil).await;

    let data_payments =
        deploy_data_payments_contract(&anvil, *network_token.contract.address()).await;

    (anvil, network_token, data_payments)
}

#[allow(clippy::unwrap_used)]
#[allow(clippy::type_complexity)]
#[allow(dead_code)]
async fn provider_with_gas_funded_wallet(
    anvil: &AnvilInstance,
) -> FillProvider<
    JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
    ReqwestProvider,
    Http<Client>,
    Ethereum,
> {
    let signer: PrivateKeySigner = LocalSigner::random();
    let wallet = EthereumWallet::from(signer);

    let rpc_url = anvil.endpoint().parse().unwrap();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    let account = wallet_address(provider.wallet());

    // Fund the wallet with plenty of gas tokens
    provider
        .anvil_set_balance(account, parse_ether("1000").expect(""))
        .await
        .unwrap();

    provider
}

#[tokio::test]
async fn test_deploy() {
    setup().await;
}

#[tokio::test]
async fn test_pay_for_quotes() {
    let (_anvil, network_token, mut data_payments) = setup().await;

    let mut quote_payments = vec![];

    for _ in 0..MAX_TRANSFERS_PER_TRANSACTION {
        let quote_payment = random_quote_payment();
        quote_payments.push(quote_payment);
    }

    let _ = network_token
        .approve(*data_payments.contract.address(), U256::MAX)
        .await
        .unwrap();

    // Contract provider has a different account coupled to it,
    // so we set it to the same as the network token contract
    data_payments.set_provider(network_token.contract.provider().clone());

    let result = data_payments.pay_for_quotes(quote_payments).await;

    assert!(result.is_ok(), "Failed with error: {:?}", result.err());
}
