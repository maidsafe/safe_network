mod common;

use crate::common::quote::random_quote_payment;
use crate::common::ROYALTIES_WALLET;
use alloy::network::{Ethereum, EthereumWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::utils::parse_ether;
use alloy::providers::ext::AnvilApi;
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider, WalletProvider};
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use alloy::transports::http::{Client, Http};
use evmlib::common::U256;
use evmlib::contract::chunk_payments::{ChunkPayments, MAX_TRANSFERS_PER_TRANSACTION};
use evmlib::contract::network_token::NetworkToken;
use evmlib::testnet::{deploy_chunk_payments_contract, deploy_network_token_contract, start_node};
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
    ChunkPayments<
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

    let chunk_payments =
        deploy_chunk_payments_contract(&anvil, *network_token.contract.address(), ROYALTIES_WALLET)
            .await;

    (anvil, network_token, chunk_payments)
}

#[allow(clippy::unwrap_used)]
#[allow(clippy::type_complexity)]
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
    let (_anvil, network_token, mut chunk_payments) = setup().await;

    let mut quote_payments = vec![];

    for _ in 0..MAX_TRANSFERS_PER_TRANSACTION {
        let quote_payment = random_quote_payment();
        quote_payments.push(quote_payment);
    }

    let _ = network_token
        .approve(*chunk_payments.contract.address(), U256::MAX)
        .await
        .unwrap();

    // Contract provider has a different account coupled to it,
    // so we set it to the same as the network token contract
    chunk_payments.set_provider(network_token.contract.provider().clone());

    let result = chunk_payments.pay_for_quotes(quote_payments).await;

    assert!(result.is_ok(), "Failed with error: {:?}", result.err());
}
