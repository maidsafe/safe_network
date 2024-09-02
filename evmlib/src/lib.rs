use alloy::primitives::{address, Address};
use alloy::transports::http::reqwest;
use std::sync::LazyLock;

pub mod contract;
pub mod signing;
pub mod wallet;

static PUBLIC_ARBITRUM_ONE_HTTP_RPC_URL: LazyLock<reqwest::Url> = LazyLock::new(|| {
    "https://arb1.arbitrum.io/rpc"
        .parse()
        .expect("Invalid RPC URL")
});

pub(crate) const ARBITRUM_ONE_PAYMENT_TOKEN_ADDRESS: Address =
    address!("4bc1aCE0E66170375462cB4E6Af42Ad4D5EC689C");

pub(crate) const ARBITRUM_ONE_CHUNK_PAYMENTS_ADDRESS: Address =
    address!("1513c4Ab34941D6e7fAbdb4e6F190d9712d6A350");

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
    pub fn rpc_url(&self) -> &reqwest::Url {
        match self {
            Network::ArbitrumOne => &PUBLIC_ARBITRUM_ONE_HTTP_RPC_URL,
            Network::Custom(custom) => &custom.rpc_url_http,
        }
    }

    pub fn payment_token_address(&self) -> &Address {
        match self {
            Network::ArbitrumOne => &ARBITRUM_ONE_PAYMENT_TOKEN_ADDRESS,
            Network::Custom(custom) => &custom.payment_token_address,
        }
    }

    pub fn chunk_payments_address(&self) -> &Address {
        match self {
            Network::ArbitrumOne => &ARBITRUM_ONE_CHUNK_PAYMENTS_ADDRESS,
            Network::Custom(custom) => &custom.chunk_payments_address,
        }
    }
}
