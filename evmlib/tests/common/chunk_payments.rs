use alloy::network::{Ethereum, EthereumWallet};
use alloy::node_bindings::AnvilInstance;
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};
use evmlib::common::{Address, Amount, QuotePayment};
use evmlib::contract::chunk_payments::ChunkPayments;
use evmlib::utils::{dummy_address, dummy_hash};

#[allow(clippy::unwrap_used)]
pub async fn deploy_chunk_payments_contract(
    anvil: &AnvilInstance,
    token_address: Address,
    royalties_wallet: Address,
) -> ChunkPayments<
    Http<Client>,
    FillProvider<
        JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
        ReqwestProvider,
        Http<Client>,
        Ethereum,
    >,
    Ethereum,
> {
    // Set up signer from the second default Anvil account (Bob).
    let signer: PrivateKeySigner = anvil.keys()[1].clone().into();
    let wallet = EthereumWallet::from(signer);

    let rpc_url = anvil.endpoint().parse().unwrap();

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    // Deploy the contract.
    ChunkPayments::deploy(provider, token_address, royalties_wallet).await
}

pub fn random_quote_payment() -> QuotePayment {
    let quote_hash = dummy_hash();
    let reward_address = dummy_address();
    let amount = Amount::from(200);
    (quote_hash, reward_address, amount)
}
