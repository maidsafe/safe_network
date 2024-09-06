pub mod error;

use crate::common;
use crate::common::{Address, TxHash};
use crate::contract::chunk_payments::error::Error;
use crate::contract::chunk_payments::ChunkPaymentsContract::ChunkPaymentsContractInstance;
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::Transport;

/// The max amount of transfers within one chunk payments transaction.
pub const MAX_TRANSFERS_PER_TRANSACTION: usize = 512;

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    ChunkPaymentsContract,
    "artifacts/ChunkPayments.json"
);

pub struct ChunkPayments<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    pub contract: ChunkPaymentsContractInstance<T, P, N>,
}

impl<T, P, N> ChunkPayments<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    /// Create a new ChunkPayments contract instance.
    pub fn new(contract_address: Address, provider: P) -> Self {
        let contract = ChunkPaymentsContract::new(contract_address, provider);
        ChunkPayments { contract }
    }

    /// Deploys the ChunkPayments smart contract to the network of the provider.
    /// ONLY DO THIS IF YOU KNOW WHAT YOU ARE DOING!
    pub async fn deploy(
        provider: P,
        payment_token_address: Address,
        royalties_wallet: Address,
    ) -> Self {
        let contract =
            ChunkPaymentsContract::deploy(provider, payment_token_address, royalties_wallet)
                .await
                .expect("Could not deploy contract");

        ChunkPayments { contract }
    }

    pub fn set_provider(&mut self, provider: P) {
        let address = *self.contract.address();
        self.contract = ChunkPaymentsContract::new(address, provider);
    }

    /// Pay for chunks.
    /// Input: (quote_id, reward_address, amount).
    pub async fn pay_for_quotes<I: IntoIterator<Item = common::QuotePayment>>(
        &self,
        chunk_payments: I,
    ) -> Result<TxHash, Error> {
        let chunk_payments: Vec<ChunkPaymentsContract::ChunkPayment> = chunk_payments
            .into_iter()
            .map(|(hash, addr, amount)| ChunkPaymentsContract::ChunkPayment {
                rewardAddress: addr,
                amount,
                quoteHash: hash,
            })
            .collect();

        if chunk_payments.len() > MAX_TRANSFERS_PER_TRANSACTION {
            return Err(Error::TransferLimitExceeded);
        }

        let tx_hash = self
            .contract
            .submitChunkPayments(chunk_payments)
            .send()
            .await?
            .watch()
            .await?;

        Ok(tx_hash)
    }
}
