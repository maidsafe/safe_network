use crate::contract::chunk_payments::quote::SignedQuote;
use crate::contract::chunk_payments::ChunkPayments;
use crate::contract::network_token::NetworkToken;
use crate::contract::{chunk_payments, network_token};
use crate::Network;
use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::primitives::{Address, TxHash, U256};
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider, WalletProvider};
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use alloy::transports::http::{reqwest, Client, Http};

pub fn random() -> EthereumWallet {
    let signer: PrivateKeySigner = LocalSigner::random();
    EthereumWallet::from(signer)
}

/// Creates a wallet from a private key in HEX format.
pub fn from_private_key(private_key: &str) -> EthereumWallet {
    let signer: PrivateKeySigner = private_key.parse().expect("Invalid private key");
    EthereumWallet::from(signer)
}

// TODO(optimization): Find a way to reuse/persist contracts and/or a provider without the wallet nonce going out of sync

fn http_provider_with_wallet(
    rpc_url: reqwest::Url,
    wallet: EthereumWallet,
) -> FillProvider<
    JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
    ReqwestProvider,
    Http<Client>,
    Ethereum,
> {
    ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url)
}

/// Returns the raw balance of tokens for this wallet.
pub async fn balance_of_tokens(
    wallet: EthereumWallet,
    network: &Network,
) -> Result<U256, network_token::Error> {
    let provider = http_provider_with_wallet(network.rpc_url().clone(), wallet);
    let network_token = NetworkToken::new(*network.payment_token_address(), provider);

    let account = <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(
        network_token.contract.provider().wallet(),
    );

    network_token.balance_of(account).await
}

/// Approve an address / smart contract to spend this wallet's tokens.
pub async fn approve_to_spend_tokens(
    wallet: EthereumWallet,
    network: &Network,
    spender: Address,
    amount: U256,
) -> Result<TxHash, network_token::Error> {
    let provider = http_provider_with_wallet(network.rpc_url().clone(), wallet);
    let network_token = NetworkToken::new(*network.payment_token_address(), provider);
    network_token.approve(spender, amount).await
}

/// Transfer tokens from the supplied wallet to an address.
pub async fn transfer_tokens(
    wallet: EthereumWallet,
    network: &Network,
    receiver: Address,
    amount: U256,
) -> Result<TxHash, network_token::Error> {
    let provider = http_provider_with_wallet(network.rpc_url().clone(), wallet);
    let network_token = NetworkToken::new(*network.payment_token_address(), provider);
    network_token.transfer(receiver, amount).await
}

/// Use this wallet to pay for quotes in batched transfer transactions.
/// If the amount of transfers is more than one transaction can contain, the transfers will be split up over multiple transactions.
pub async fn pay_for_quotes<T: IntoIterator<Item = SignedQuote>>(
    wallet: EthereumWallet,
    network: &Network,
    quotes: T,
) -> Result<Vec<TxHash>, chunk_payments::error::Error> {
    let provider = http_provider_with_wallet(network.rpc_url().clone(), wallet);
    let chunk_payments = ChunkPayments::new(*network.chunk_payments_address(), provider);

    let mut tx_hashes = Vec::new();

    // Max 256 at a time
    let quotes: Vec<_> = quotes.into_iter().collect();
    let chunks = quotes.chunks(256);

    for batch in chunks {
        let batch: Vec<SignedQuote> = batch.to_vec();
        let tx_hash = chunk_payments.pay_for_quotes(batch).await?;
        tx_hashes.push(tx_hash);
    }

    Ok(tx_hashes)
}

#[cfg(test)]
mod tests {
    use crate::wallet::from_private_key;
    use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
    use alloy::primitives::address;

    #[tokio::test]
    async fn test_from_private_key() {
        let private_key = "bf210844fa5463e373974f3d6fbedf451350c3e72b81b3c5b1718cb91f49c33d";
        let wallet = from_private_key(private_key);
        let account = <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(&wallet);

        // Assert that the addresses are the same, i.e. the wallet was successfully created from the private key
        assert_eq!(
            account,
            address!("1975d01f46D70AAc0dd3fCf942d92650eE63C79A")
        );
    }
}
