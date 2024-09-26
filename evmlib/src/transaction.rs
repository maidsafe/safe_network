use crate::common::{Address, QuoteHash, TxHash, U256};
use crate::event::{ChunkPaymentEvent, CHUNK_PAYMENT_EVENT_SIGNATURE};
use crate::Network;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::FixedBytes;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Block, Filter, Log, TransactionReceipt};
use alloy::transports::{RpcError, TransportErrorKind};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
    #[error("Transaction is not confirmed")]
    TransactionUnconfirmed,
    #[error("Transaction was not found")]
    TransactionNotFound,
    #[error("Transaction has not been included in a block yet")]
    TransactionNotInBlock,
    #[error("Block was not found")]
    BlockNotFound,
    #[error("No event proof found")]
    EventProofNotFound,
    #[error("Payment was done after the quote expired")]
    QuoteExpired,
}

/// Get a transaction receipt by its hash.
pub async fn get_transaction_receipt_by_hash(
    network: &Network,
    transaction_hash: TxHash,
) -> Result<Option<TransactionReceipt>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let maybe_receipt = provider.get_transaction_receipt(transaction_hash).await?;
    Ok(maybe_receipt)
}

/// Get a block by its block number.
async fn get_block_by_number(network: &Network, block_number: u64) -> Result<Option<Block>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let block = provider
        .get_block_by_number(BlockNumberOrTag::Number(block_number), true)
        .await?;
    Ok(block)
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
    let topic1: FixedBytes<32> = FixedBytes::left_padding_from(reward_addr.as_slice());

    let filter = Filter::new()
        .event_signature(CHUNK_PAYMENT_EVENT_SIGNATURE)
        .topic1(topic1)
        .topic2(amount)
        .topic3(quote_hash)
        .from_block(block_number)
        .to_block(block_number);

    get_transaction_logs(network, filter).await
}

/// Verify if a chunk payment is confirmed.
pub async fn verify_chunk_payment(
    network: &Network,
    tx_hash: TxHash,
    quote_hash: QuoteHash,
    reward_addr: Address,
    amount: U256,
    quote_expiration_timestamp_in_secs: u64,
) -> Result<(), Error> {
    let transaction = get_transaction_receipt_by_hash(network, tx_hash)
        .await?
        .ok_or(Error::TransactionNotFound)?;

    // If the status is True, it means the tx is confirmed.
    if !transaction.status() {
        return Err(Error::TransactionUnconfirmed);
    }

    let block_number = transaction
        .block_number
        .ok_or(Error::TransactionNotInBlock)?;

    let block = get_block_by_number(network, block_number)
        .await?
        .ok_or(Error::BlockNotFound)?;

    // Check if payment was done within the quote expiration timeframe.
    if quote_expiration_timestamp_in_secs < block.header.timestamp {
        return Err(Error::QuoteExpired);
    }

    let logs =
        get_chunk_payment_event(network, block_number, quote_hash, reward_addr, amount).await?;

    for log in logs {
        if log.transaction_hash != Some(tx_hash) {
            // Wrong transaction.
            continue;
        }

        if let Ok(event) = ChunkPaymentEvent::try_from(log) {
            // Check if the event matches what we expect.
            // The smart contract handles royalties, so we don't have to check that.
            if event.quote_hash == quote_hash
                && event.reward_address == reward_addr
                && event.amount >= amount
            {
                return Ok(());
            }
        }
    }

    Err(Error::EventProofNotFound)
}

#[cfg(test)]
mod tests {
    use crate::common::{Address, U256};
    use crate::transaction::{
        get_chunk_payment_event, get_transaction_receipt_by_hash, verify_chunk_payment,
    };
    use crate::Network;
    use alloy::hex::FromHex;
    use alloy::primitives::b256;

    #[tokio::test]
    async fn test_get_transaction_receipt_by_hash() {
        let network = Network::ArbitrumOne;

        let tx_hash = b256!("462ff33b01d7930b05dc87826b485f6f19884f1cf1c15694477be68ff7dda066");

        assert!(get_transaction_receipt_by_hash(&network, tx_hash)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn test_get_chunk_payment_event() {
        let network = Network::ArbitrumOne;

        let block_number: u64 = 250043261;
        let reward_address = Address::from_hex("fdd33ec6f2325b742c1f32ed5b1da19547cb2f30").unwrap();
        let amount = U256::from(200);
        let quote_hash = b256!("477a32ca129183ebaa7e0a082813f8f9b121a1f9ba5dd83104bae44b6e32658c");

        let logs =
            get_chunk_payment_event(&network, block_number, quote_hash, reward_address, amount)
                .await
                .unwrap();

        assert_eq!(logs.len(), 1);
    }

    #[tokio::test]
    async fn test_verify_chunk_payment() {
        let network = Network::ArbitrumOne;

        let tx_hash = b256!("462ff33b01d7930b05dc87826b485f6f19884f1cf1c15694477be68ff7dda066");
        let quote_hash = b256!("477a32ca129183ebaa7e0a082813f8f9b121a1f9ba5dd83104bae44b6e32658c");
        let reward_address = Address::from_hex("fdd33ec6f2325b742c1f32ed5b1da19547cb2f30").unwrap();
        let amount = U256::from(200);

        let result = verify_chunk_payment(
            &network,
            tx_hash,
            quote_hash,
            reward_address,
            amount,
            4102441200,
        )
        .await;

        assert!(result.is_ok(), "Error: {:?}", result.err());
    }
}
