use std::collections::BTreeMap;

use crate::common::{Address, QuoteHash, QuotePayment, TxHash, U256};
use crate::contract::chunk_payments::{ChunkPayments, MAX_TRANSFERS_PER_TRANSACTION};
use crate::contract::network_token::NetworkToken;
use crate::contract::{chunk_payments, network_token};
use crate::utils::calculate_royalties_from_amount;
use crate::Network;
use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider};
use alloy::signers::local::{LocalSigner, PrivateKeySigner};
use alloy::transports::http::{reqwest, Client, Http};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Private key is invalid")]
    PrivateKeyInvalid,
}

pub struct Wallet {
    wallet: EthereumWallet,
    network: Network,
}

impl Wallet {
    /// Creates a new Wallet object with the specific Network and EthereumWallet.
    pub fn new(network: Network, wallet: EthereumWallet) -> Self {
        Self { wallet, network }
    }

    /// Convenience function that creates a new Wallet with a random EthereumWallet.
    pub fn new_with_random_wallet(network: Network) -> Self {
        Self::new(network, random())
    }

    /// Creates a new Wallet based on the given private_key. It will fail with Error::PrivateKeyInvalid if private_key is invalid.
    pub fn new_from_private_key(network: Network, private_key: &str) -> Result<Self, Error> {
        let wallet = from_private_key(private_key)?;
        Ok(Self::new(network, wallet))
    }

    /// Returns the address of this wallet.
    pub fn address(&self) -> Address {
        wallet_address(&self.wallet)
    }

    /// Returns the raw balance of tokens for this wallet.
    pub async fn balance_of_tokens(&self) -> Result<U256, network_token::Error> {
        balance_of_tokens(self.wallet.clone(), &self.network).await
    }

    /// Transfer a raw amount of tokens to another address.
    pub async fn transfer_tokens(
        &self,
        to: Address,
        amount: U256,
    ) -> Result<TxHash, network_token::Error> {
        transfer_tokens(self.wallet.clone(), &self.network, to, amount).await
    }

    /// Pays for a single quote. Returns transaction hash of the payment.
    pub async fn pay_for_quote(
        &self,
        quote_hash: QuoteHash,
        rewards_addr: Address,
        amount: U256,
    ) -> Result<TxHash, chunk_payments::error::Error> {
        self.pay_for_quotes([(quote_hash, rewards_addr, amount)])
            .await
            .map(|v| v.values().last().cloned().expect("Infallible"))
    }

    /// Function for batch payments of quotes. It accepts an iterator of QuotePayment and returns
    /// transaction hashes of the payments by quotes.
    pub async fn pay_for_quotes<I: IntoIterator<Item = QuotePayment>>(
        &self,
        chunk_payments: I,
    ) -> Result<BTreeMap<QuoteHash, TxHash>, chunk_payments::error::Error> {
        pay_for_quotes(self.wallet.clone(), &self.network, chunk_payments).await
    }
}

/// Generate an EthereumWallet with a random private key.
fn random() -> EthereumWallet {
    let signer: PrivateKeySigner = LocalSigner::random();
    EthereumWallet::from(signer)
}

/// Creates a wallet from a private key in HEX format.
fn from_private_key(private_key: &str) -> Result<EthereumWallet, Error> {
    let signer: PrivateKeySigner = private_key.parse().map_err(|_| Error::PrivateKeyInvalid)?;
    Ok(EthereumWallet::from(signer))
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

/// Returns the address of this wallet.
pub fn wallet_address(wallet: &EthereumWallet) -> Address {
    <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(wallet)
}

/// Returns the raw balance of tokens for this wallet.
pub async fn balance_of_tokens(
    wallet: EthereumWallet,
    network: &Network,
) -> Result<U256, network_token::Error> {
    let account = wallet_address(&wallet);
    let provider = http_provider_with_wallet(network.rpc_url().clone(), wallet);
    let network_token = NetworkToken::new(*network.payment_token_address(), provider);
    network_token.balance_of(account).await
}

/// Approve an address / smart contract to spend this wallet's tokens.
async fn approve_to_spend_tokens(
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

/// Use this wallet to pay for chunks in batched transfer transactions.
/// If the amount of transfers is more than one transaction can contain, the transfers will be split up over multiple transactions.
pub async fn pay_for_quotes<T: IntoIterator<Item = QuotePayment>>(
    wallet: EthereumWallet,
    network: &Network,
    payments: T,
) -> Result<BTreeMap<QuoteHash, TxHash>, chunk_payments::error::Error> {
    let payments: Vec<_> = payments.into_iter().collect();
    let total_amount = payments.iter().map(|(_, _, amount)| amount).sum();
    let royalties = calculate_royalties_from_amount(total_amount);

    // 2 * royalties to have a small buffer for different rounding in the smart contract.
    let total_amount_with_royalties = total_amount + (U256::from(2) * royalties);

    // Approve the contract to spend enough of the client's tokens.
    approve_to_spend_tokens(
        wallet.clone(),
        network,
        *network.chunk_payments_address(),
        total_amount_with_royalties,
    )
    .await?;

    let provider = http_provider_with_wallet(network.rpc_url().clone(), wallet);
    let chunk_payments = ChunkPayments::new(*network.chunk_payments_address(), provider);

    let mut tx_hashes_by_quote = BTreeMap::new();

    // Divide transfers over multiple transactions if they exceed the max per transaction.
    let chunks = payments.chunks(MAX_TRANSFERS_PER_TRANSACTION);

    for batch in chunks {
        let batch: Vec<QuotePayment> = batch.to_vec();
        let tx_hash = chunk_payments.pay_for_quotes(batch.clone()).await?;

        for (quote_hash, _, _) in batch {
            tx_hashes_by_quote.insert(quote_hash, tx_hash);
        }
    }

    Ok(tx_hashes_by_quote)
}

#[cfg(test)]
mod tests {
    use crate::wallet::from_private_key;
    use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
    use alloy::primitives::address;

    #[tokio::test]
    async fn test_from_private_key() {
        let private_key = "bf210844fa5463e373974f3d6fbedf451350c3e72b81b3c5b1718cb91f49c33d";
        let wallet = from_private_key(private_key).unwrap();
        let account = <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(&wallet);

        // Assert that the addresses are the same, i.e. the wallet was successfully created from the private key
        assert_eq!(
            account,
            address!("1975d01f46D70AAc0dd3fCf942d92650eE63C79A")
        );
    }
}
