use alloy::primitives::{address, Address};

pub mod contract;

pub(crate) const PUBLIC_ARBITRUM_SEPOLIA_RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";
pub(crate) const ARBITRUM_SEPOLIA_NETWORK_TOKEN_ADDRESS: Address =
    address!("4bc1aCE0E66170375462cB4E6Af42Ad4D5EC689C");
pub(crate) const ARBITRUM_SEPOLIA_CHUNK_PAYMENTS_ADDRESS: Address =
    address!("330ad5eA0D8eff21098336D067524893A6801C67");

pub struct CustomNetwork {
    rpc_url: String,
    network_token_address: Address,
    chunk_payments_address: Address,
}

pub enum Network {
    ArbitrumSepolia,
    Custom(CustomNetwork),
}

impl Network {
    pub fn rpc_url(&self) -> &str {
        match self {
            Network::ArbitrumSepolia => PUBLIC_ARBITRUM_SEPOLIA_RPC_URL,
            Network::Custom(custom) => &custom.rpc_url,
        }
    }

    pub fn network_token_address(&self) -> &Address {
        match self {
            Network::ArbitrumSepolia => &ARBITRUM_SEPOLIA_NETWORK_TOKEN_ADDRESS,
            Network::Custom(custom) => &custom.network_token_address,
        }
    }

    pub fn chunk_payments_address(&self) -> &Address {
        match self {
            Network::ArbitrumSepolia => &ARBITRUM_SEPOLIA_CHUNK_PAYMENTS_ADDRESS,
            Network::Custom(custom) => &custom.chunk_payments_address,
        }
    }
}
