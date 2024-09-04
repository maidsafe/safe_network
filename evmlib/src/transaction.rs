use crate::contract::common::{Address, QuoteHash, TxHash, U256};
use crate::Network;
use alloy::primitives::FixedBytes;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Filter, Log, Transaction};
use alloy::transports::{RpcError, TransportErrorKind};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
    #[error("Transaction was not found")]
    TransactionNotFound,
    #[error("Transaction has not been included in a block yet")]
    TransactionNotInBlock,
    #[error("No event proof found")]
    EventProofNotFound,
}

/// Get a transaction by its hash.
async fn get_transaction_by_hash(
    network: &Network,
    transaction_hash: TxHash,
) -> Result<Option<Transaction>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let maybe_transaction = provider.get_transaction_by_hash(transaction_hash).await?;
    Ok(maybe_transaction)
}

/// Get transaction logs using a filter.
async fn get_transaction_logs(network: &Network, filter: Filter) -> Result<Vec<Log>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}

/// Get a ChunkPaymentMade event, filtered by a hashed chunk address and a node address.
/// Useful for a node if it wants to check if payment for a certain chunk has been made.
async fn get_chunk_payment_event(
    network: &Network,
    block_number: u64,
    quote_hash: QuoteHash,
    reward_addr: Address,
    amount: U256,
) -> Result<Vec<Log>, Error> {
    let event = "ChunkPaymentMade(address,uint256,uint64)";

    let topic1: FixedBytes<32> = FixedBytes::left_padding_from(reward_addr.as_slice());
    let topic3: FixedBytes<32> = FixedBytes::left_padding_from(&quote_hash);

    let filter = Filter::new()
        .event(event)
        .topic1(topic1)
        .topic2(amount)
        .topic3(topic3)
        .from_block(block_number)
        .to_block(block_number);

    get_transaction_logs(network, filter).await
}

/// verify if a chunk payment is confirmed
pub async fn verify_chunk_payment(
    network: &Network,
    tx_hash: TxHash,
    quote_hash: QuoteHash,
    reward_addr: Address,
    amount: U256,
) -> Result<(), Error> {
    let block_number = get_transaction_by_hash(network, tx_hash)
        .await?
        .ok_or(Error::TransactionNotFound)?
        .block_number
        .ok_or(Error::TransactionNotInBlock)?;

    if let Ok(logs) =
        get_chunk_payment_event(network, block_number, quote_hash, reward_addr, amount).await
    {
        for _log in logs {
            // TODO: convert log to ChunkPaymentEvent Struct.
            // TODO: verify if event.amount, event.rewardAddress & event.quoteHash match amount, reward_addr and quote_hash.
            // Return OK if ANY log matches.
        }
    }

    Err(Error::EventProofNotFound)
}

#[cfg(test)]
mod tests {
    use crate::contract::common::TxHash;
    use crate::transaction::get_transaction_by_hash;
    use crate::Network;
    use alloy::hex::FromHex;

    #[tokio::test]
    async fn test_get_transaction_by_hash() {
        let network = Network::ArbitrumOne;

        let tx_hash =
            TxHash::from_hex("0358cb4d135926b28b4f831653cf92f29c7d3f12d6227cad894f2257d600f1c8")
                .unwrap();

        assert!(get_transaction_by_hash(&network, tx_hash)
            .await
            .unwrap()
            .is_some());
    }
}
