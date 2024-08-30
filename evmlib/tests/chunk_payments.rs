mod common;

use crate::common::local_testnet::start_anvil_node;
use crate::common::network_token::deploy_network_token_contract;
use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::utils::parse_ether;
use alloy::primitives::{address, Address, U256};
use alloy::providers::ext::AnvilApi;
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider, WalletProvider};
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use alloy::transports::http::{Client, Http};
use evmlib::contract::chunk_payments::ChunkPayments;
use evmlib::contract::network_token::NetworkToken;

const ROYALTIES_WALLET: Address = address!("385e7887E5b41750E3679Da787B943EC42f37d75");

pub async fn deploy_chunk_payments_contract(
    anvil: &AnvilInstance,
    token_address: Address,
    royalties_wallet: Address,
) -> eyre::Result<
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
> {
    // Set up signer from the second default Anvil account (Bob).
    let signer: PrivateKeySigner = anvil.keys()[1].clone().into();
    let wallet = EthereumWallet::from(signer);

    let rpc_url = anvil.endpoint().parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    // Deploy the contract.
    let contract = ChunkPayments::deploy(provider, token_address, royalties_wallet).await;

    Ok(contract)
}

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
    let anvil = start_anvil_node()
        .await
        .expect("Could not start anvil node");

    let network_token = deploy_network_token_contract(&anvil)
        .await
        .expect("Could not deploy AutonomiNetworkToken");

    let chunk_payments =
        deploy_chunk_payments_contract(&anvil, *network_token.contract.address(), ROYALTIES_WALLET)
            .await
            .expect("Could not deploy ChunkPaymentsContract");

    (anvil, network_token, chunk_payments)
}

#[allow(clippy::type_complexity)]
async fn provider_with_funded_wallet(
    anvil: &AnvilInstance,
) -> eyre::Result<
    FillProvider<
        JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
        ReqwestProvider,
        Http<Client>,
        Ethereum,
    >,
> {
    let signer: PrivateKeySigner = LocalSigner::random();
    let wallet = EthereumWallet::from(signer);

    let rpc_url = anvil.endpoint().parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    let account =
        <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(provider.wallet());

    // Fund the wallet with plenty of gas tokens
    provider
        .anvil_set_balance(account, parse_ether("1000").expect(""))
        .await?;

    Ok(provider)
}

#[tokio::test]
async fn test_deploy() {
    setup().await;
}

#[tokio::test]
async fn test_submit_chunk_payment() {
    let (_anvil, network_token, mut chunk_payments) = setup().await;

    let node = LocalSigner::random().address();
    let quote_amount = U256::from(1);

    network_token
        .approve(*chunk_payments.contract.address(), quote_amount)
        .await
        .unwrap();

    // Contract provider has a different account coupled to it,
    // so we set it to the same as the network token contract
    chunk_payments.set_provider(network_token.contract.provider().clone());

    let submit_chunk_payment_result = chunk_payments
        .submit_chunk_payment(node, 1, U256::from(1))
        .await;

    assert!(
        submit_chunk_payment_result.is_ok(),
        "Submit chunk failed with error: {:?}",
        submit_chunk_payment_result.err()
    );
}

#[tokio::test]
async fn test_submit_chunk_payment_should_fail() {
    let (_anvil, _network_token, chunk_payments) = setup().await;

    let node = LocalSigner::random().address();

    let submit_chunk_payment_result = chunk_payments
        .submit_chunk_payment(node, 1, U256::from(1))
        .await;

    assert!(submit_chunk_payment_result.is_err());
}

#[tokio::test]
async fn test_submit_bulk_chunk_payments() {
    let (_anvil, network_token, mut chunk_payments) = setup().await;

    let nodes = vec![
        LocalSigner::random().address(),
        LocalSigner::random().address(),
    ];
    let quote_identifiers = vec![1, 2];
    let quote_amounts = vec![U256::from(1), U256::from(2)];

    network_token
        .approve(
            *chunk_payments.contract.address(),
            quote_amounts.iter().sum(),
        )
        .await
        .unwrap();

    // Contract provider has a different account coupled to it,
    // so we set it to the same as the network token contract
    chunk_payments.set_provider(network_token.contract.provider().clone());

    let submit_bulk_chunk_payment_result = chunk_payments
        .submit_bulk_chunk_payments(nodes, quote_identifiers, quote_amounts)
        .await;

    assert!(
        submit_bulk_chunk_payment_result.is_ok(),
        "Submit bulk chunk payments failed with error: {:?}",
        submit_bulk_chunk_payment_result.err()
    );
}
