use alloy::providers::{Provider, ProviderBuilder, ReqwestProvider};
use alloy::transports::{RpcError, TransportErrorKind};
use thiserror::Error;

pub const ARBITRUM_SEPOLIA_RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";
pub const ARBITRUM_ONE_RPC_URL: &str = "https://arb1.arbitrum.io/rpc";

#[derive(Debug, Error)]
pub enum EvmRpcError {
    #[error("Could not parse RPC url")]
    RpcUrlInvalid,
    #[error("Transport error: {0}")]
    TransportError(#[from] RpcError<TransportErrorKind>),
}

pub struct EvmRpcClient {
    pub provider: ReqwestProvider,
}

impl EvmRpcClient {
    pub fn new(rpc_url: &str) -> Result<Self, EvmRpcError> {
        let provider = ProviderBuilder::new()
            .on_http(rpc_url.parse().map_err(|_| EvmRpcError::RpcUrlInvalid)?);
        Ok(EvmRpcClient { provider })
    }

    pub async fn chain_id(&self) -> Result<u64, EvmRpcError> {
        let chain_id = self.provider.get_chain_id().await?;
        Ok(chain_id)
    }

    pub fn as_provider(&self) -> &ReqwestProvider {
        &self.provider
    }
}
