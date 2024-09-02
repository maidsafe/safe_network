pub mod error;
pub mod quote;

use crate::contract::chunk_payments::error::Error;
use crate::contract::chunk_payments::quote::SignedQuote;
use crate::contract::chunk_payments::ChunkPaymentsContract::ChunkPaymentsContractInstance;
use alloy::primitives::{Address, TxHash};
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::Transport;

/// The max amount of transfers within one chunk payments transaction.
const TRANSFER_LIMIT: u16 = 256;

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    ChunkPaymentsContract,
    "artifacts/ChunkPayments.json"
);

impl From<SignedQuote> for ChunkPaymentsContract::Quote {
    fn from(quote: SignedQuote) -> ChunkPaymentsContract::Quote {
        #[allow(clippy::all)]
        ChunkPaymentsContract::Quote {
            chunk_address_hash: quote.quote.chunk_address_hash,
            cost: quote.quote.cost,
            expiration_timestamp: quote.quote.expiration_timestamp,
            payment_address: quote.quote.payment_address,
            signature: ChunkPaymentsContract::Signature {
                r: quote.signature.r,
                s: quote.signature.s,
                v: quote.signature.v,
            },
        }
    }
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
    /// Create a new ChunkPayments contract instance.
    pub fn new(contract_address: Address, provider: P) -> Self {
        let contract = ChunkPaymentsContract::new(contract_address, provider);
        ChunkPayments { contract }
    }

    /// Deploys the ChunkPayments smart contract to the network of the provider.
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

    /// Pay for signed quotes.
    pub async fn pay_for_quotes(&self, quotes: Vec<SignedQuote>) -> Result<TxHash, Error> {
        if quotes.len() > TRANSFER_LIMIT as usize {
            return Err(Error::TransferLimitExceeded);
        }

        let quotes: Vec<ChunkPaymentsContract::Quote> = quotes
            .into_iter()
            .map(ChunkPaymentsContract::Quote::from)
            .collect();

        let tx_hash = self
            .contract
            .payForQuotes(quotes)
            .send()
            .await?
            .watch()
            .await?;

        Ok(tx_hash)
    }
}
