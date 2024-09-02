mod common;

use crate::common::local_testnet::start_anvil_node;
use crate::common::network_token::deploy_network_token_contract;
use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::utils::parse_ether;
use alloy::primitives::{address, Address, FixedBytes, U256};
use alloy::providers::ext::AnvilApi;
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider, WalletProvider};
use alloy::signers::k256::ecdsa::SigningKey;
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use alloy::transports::http::{Client, Http};
use evmlib::contract::chunk_payments::quote::{Quote, Signature, SignedQuote};
use evmlib::contract::chunk_payments::ChunkPayments;
use evmlib::contract::network_token::NetworkToken;
use evmlib::signing::{generate_ecdsa_keypair, sign_message_recoverable};
use rand::Rng;

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

// Function to generate a random quote with a valid signature
fn generate_random_quote(secret_key: &SigningKey) -> SignedQuote {
    let mut rng = rand::rngs::OsRng;

    let chunk_address_hash = FixedBytes::new(rng.gen());
    let cost = U256::from(20);
    let expiration_timestamp = U256::from(1214604971);
    let payment_address = Address::new(rng.gen());

    let mut message: Vec<u8> = vec![];
    message.extend_from_slice(chunk_address_hash.as_slice());
    message.extend_from_slice(cost.as_le_slice());
    message.extend_from_slice(expiration_timestamp.as_le_slice());
    message.extend_from_slice(payment_address.as_slice());

    let (signature, recovery_id) =
        sign_message_recoverable(secret_key, message.as_slice()).expect("Could not sign message");

    let quote = Quote {
        chunk_address_hash,
        cost,
        expiration_timestamp,
        payment_address,
    };

    SignedQuote {
        quote,
        signature: Signature {
            r: FixedBytes::from_slice(signature.r().to_bytes().as_slice()),
            s: FixedBytes::from_slice(signature.s().to_bytes().as_slice()),
            v: u8::from(recovery_id),
        },
    }
}

#[tokio::test]
async fn test_deploy() {
    setup().await;
}

#[tokio::test]
async fn test_pay_for_quotes() {
    let (_anvil, network_token, mut chunk_payments) = setup().await;

    let node = generate_ecdsa_keypair();
    let quote = generate_random_quote(&node.0);

    let _ = network_token
        .approve(*chunk_payments.contract.address(), U256::MAX)
        .await
        .unwrap();

    // Contract provider has a different account coupled to it,
    // so we set it to the same as the network token contract
    chunk_payments.set_provider(network_token.contract.provider().clone());

    let submit_bulk_chunk_payment_result = chunk_payments.pay_for_quotes(vec![quote]).await;

    assert!(
        submit_bulk_chunk_payment_result.is_ok(),
        "Submit bulk chunk payments failed with error: {:?}",
        submit_bulk_chunk_payment_result.err()
    );
}
