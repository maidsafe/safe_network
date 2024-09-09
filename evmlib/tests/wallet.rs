mod common;

use crate::common::chunk_payments::{deploy_chunk_payments_contract, random_quote_payment};
use crate::common::local_testnet::start_anvil_node;
use crate::common::network_token::deploy_network_token_contract;
use crate::common::ROYALTIES_WALLET;
use alloy::network::EthereumWallet;
use alloy::node_bindings::AnvilInstance;
use alloy::primitives::utils::parse_ether;
use alloy::providers::ext::AnvilApi;
use alloy::providers::{ProviderBuilder, WalletProvider};
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use evmlib::common::Amount;
use evmlib::contract::chunk_payments::MAX_TRANSFERS_PER_TRANSACTION;
use evmlib::transaction::verify_chunk_payment;
use evmlib::wallet::{transfer_tokens, wallet_address, Wallet};
use evmlib::{CustomNetwork, Network};

#[allow(clippy::unwrap_used)]
async fn local_testnet() -> (AnvilInstance, Network, EthereumWallet) {
    let anvil = start_anvil_node().await;
    let rpc_url = anvil.endpoint().parse().unwrap();
    let network_token = deploy_network_token_contract(&anvil).await;
    let payment_token_address = *network_token.contract.address();
    let chunk_payments =
        deploy_chunk_payments_contract(&anvil, payment_token_address, ROYALTIES_WALLET).await;

    (
        anvil,
        Network::Custom(CustomNetwork {
            rpc_url_http: rpc_url,
            payment_token_address,
            chunk_payments_address: *chunk_payments.contract.address(),
        }),
        network_token.contract.provider().wallet().clone(),
    )
}

#[allow(clippy::unwrap_used)]
async fn funded_wallet(network: &Network, genesis_wallet: EthereumWallet) -> Wallet {
    let signer: PrivateKeySigner = LocalSigner::random();
    let wallet = EthereumWallet::from(signer);
    let account = wallet_address(&wallet);

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(genesis_wallet.clone())
        .on_http(network.rpc_url().clone());

    // Fund the wallet with plenty of gas tokens
    provider
        .anvil_set_balance(account, parse_ether("1000").expect(""))
        .await
        .unwrap();

    // Fund the wallet with plenty of ERC20 tokens
    transfer_tokens(
        genesis_wallet,
        network,
        account,
        Amount::from(9999999999_u64),
    )
    .await
    .unwrap();

    Wallet::new(network.clone(), wallet)
}

#[tokio::test]
async fn test_pay_for_quotes_and_chunk_payment_verification() {
    const TRANSFERS: usize = 600;

    let (_anvil, network, genesis_wallet) = local_testnet().await;
    let wallet = funded_wallet(&network, genesis_wallet).await;

    let mut quote_payments = vec![];

    for _ in 0..TRANSFERS {
        let quote = random_quote_payment();
        quote_payments.push(quote);
    }

    let tx_hashes = wallet.pay_for_quotes(quote_payments.clone()).await.unwrap();

    assert_eq!(
        tx_hashes.len(),
        TRANSFERS.div_ceil(MAX_TRANSFERS_PER_TRANSACTION)
    );

    for (i, quote_payment) in quote_payments.iter().enumerate() {
        let tx_index = i / MAX_TRANSFERS_PER_TRANSACTION;

        let tx_hash = *tx_hashes.get(tx_index).unwrap();

        let result = verify_chunk_payment(
            &network,
            tx_hash,
            quote_payment.0,
            quote_payment.1,
            quote_payment.2,
        )
        .await;

        assert!(
            result.is_ok(),
            "Verification failed for({i}): {quote_payment:?}. Error: {:?}",
            result.err()
        );
    }
}
