use crate::contract::chunk_payments::ChunkPaymentsContract::ChunkPaymentsContractInstance;
use alloy::primitives::{Address, TxHash, U256};
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::{RpcError, Transport, TransportErrorKind};
use thiserror::Error;

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    ChunkPaymentsContract,
    "artifacts/ChunkPayments.json"
);

#[derive(Error, Debug)]
pub enum ChunkPaymentsError {
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
}

pub struct ChunkPayments<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    pub contract: ChunkPaymentsContractInstance<T, P, N>,
}

impl<T, P, N> ChunkPayments<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    pub fn new(network: crate::Network, provider: P) -> Self {
        let contract = ChunkPaymentsContract::new(*network.network_token_address(), provider);
        ChunkPayments { contract }
    }

    /// Deploys the ChunkPayments smart contract to the network of the provider.
    pub async fn deploy(provider: P, token_address: Address, royalties_wallet: Address) -> Self {
        let contract = ChunkPaymentsContract::deploy(provider, token_address, royalties_wallet)
            .await
            .expect("Could not deploy contract");

        ChunkPayments { contract }
    }

    pub fn set_network(&mut self, network: crate::Network) {
        self.contract.set_address(*network.network_token_address());
    }

    pub fn set_provider(&mut self, provider: P) {
        let address = *self.contract.address();
        self.contract = ChunkPaymentsContract::new(address, provider);
    }

    // DevNote: could be deprecated if we only need bulk payments
    pub async fn submit_chunk_payment(
        &self,
        node: Address,
        quote_identifier: u32,
        quote_amount: U256,
    ) -> Result<TxHash, ChunkPaymentsError> {
        let tx_hash = self
            .contract
            .submitChunkPayment(node, quote_identifier, quote_amount)
            .send()
            .await?
            .watch()
            .await?;

        Ok(tx_hash)
    }

    pub async fn submit_bulk_chunk_payments(
        &self,
        nodes: Vec<Address>,
        quote_identifiers: Vec<u32>,
        quote_amounts: Vec<U256>,
    ) -> Result<TxHash, ChunkPaymentsError> {
        let tx_hash = self
            .contract
            .submitBulkChunkPayments(nodes, quote_identifiers, quote_amounts)
            .send()
            .await?
            .watch()
            .await?;

        Ok(tx_hash)
    }
}
