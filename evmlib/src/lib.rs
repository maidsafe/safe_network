use crate::contract::common::{QuoteHash, TxHash, U256};
use crate::transaction::verify_chunk_payment;
use alloy::primitives::{address, Address};
use alloy::transports::http::reqwest;
use std::sync::LazyLock;

pub mod contract;
pub mod cryptography;
pub mod transaction;
pub mod wallet;

static PUBLIC_ARBITRUM_ONE_HTTP_RPC_URL: LazyLock<reqwest::Url> = LazyLock::new(|| {
    "https://arb1.arbitrum.io/rpc"
        .parse()
        .expect("Invalid RPC URL")
});

const ARBITRUM_ONE_PAYMENT_TOKEN_ADDRESS: Address =
    address!("4bc1aCE0E66170375462cB4E6Af42Ad4D5EC689C");

const ARBITRUM_ONE_CHUNK_PAYMENTS_ADDRESS: Address =
    address!("F15BfEA73b6a551C5c2e66026e4eB3b69c1F602c");

pub struct CustomNetwork {
    rpc_url_http: reqwest::Url,
    payment_token_address: Address,
    chunk_payments_address: Address,
}

pub enum Network {
    ArbitrumOne,
    Custom(CustomNetwork),
}

impl Network {
    pub(crate) fn rpc_url(&self) -> &reqwest::Url {
        match self {
            Network::ArbitrumOne => &PUBLIC_ARBITRUM_ONE_HTTP_RPC_URL,
            Network::Custom(custom) => &custom.rpc_url_http,
        }
    }

    pub(crate) fn payment_token_address(&self) -> &Address {
        match self {
            Network::ArbitrumOne => &ARBITRUM_ONE_PAYMENT_TOKEN_ADDRESS,
            Network::Custom(custom) => &custom.payment_token_address,
        }
    }

    pub(crate) fn chunk_payments_address(&self) -> &Address {
        match self {
            Network::ArbitrumOne => &ARBITRUM_ONE_CHUNK_PAYMENTS_ADDRESS,
            Network::Custom(custom) => &custom.chunk_payments_address,
        }
    }

    pub async fn verify_chunk_payment(
        &self,
        tx_hash: TxHash,
        quote_hash: QuoteHash,
        reward_addr: Address,
        amount: U256,
    ) -> Result<(), transaction::Error> {
        verify_chunk_payment(self, tx_hash, quote_hash, reward_addr, amount).await
    }
}
