use crate::cryptography::public_key_to_address;
use crate::Network;
use alloy::primitives::{FixedBytes, TxHash};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Filter, Log, Transaction, TransactionReceipt};
use alloy::signers::k256::ecdsa::VerifyingKey;
use alloy::transports::{RpcError, TransportErrorKind};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
}

/// Get a transaction by its hash.
pub async fn get_transaction_by_hash(
    network: &Network,
    transaction_hash: TxHash,
) -> Result<Option<Transaction>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let maybe_transaction = provider.get_transaction_by_hash(transaction_hash).await?;
    Ok(maybe_transaction)
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

/// Get transaction logs using a filter.
pub async fn get_transaction_logs(network: &Network, filter: Filter) -> Result<Vec<Log>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}

/// Get a ChunkPaymentMade event, filtered by a hashed chunk address and a node address.
/// Useful for a node if it wants to check if payment for a certain chunk has been made.
pub async fn get_chunk_payment_event_for_public_key_and_chunk_address_hash(
    network: &Network,
    public_key: &VerifyingKey,
    chunk_address_hash: [u8; 32],
) -> Result<Vec<Log>, Error> {
    let event = "ChunkPaymentMade(address,bytes32,address,tuple)";

    // Chunk address hash
    let topic2: FixedBytes<32> = FixedBytes::from(chunk_address_hash);

    // Node address with leading zeroes
    let topic3: FixedBytes<32> =
        FixedBytes::left_padding_from(public_key_to_address(public_key).as_slice());

    // Create a filter for the event
    let filter = Filter::new()
        .address(*network.chunk_payments_address())
        .event(event)
        .topic2(topic2)
        .topic3(topic3);

    get_transaction_logs(network, filter).await
}

// TODO: implement this
pub async fn verify_chunk_payment_made(
    network: &Network,
    public_key: &VerifyingKey,
    chunk_address_hash: [u8; 32],
) -> bool {
    if let Ok(logs) = get_chunk_payment_event_for_public_key_and_chunk_address_hash(
        network,
        public_key,
        chunk_address_hash,
    )
    .await
    {}

    false
}

#[cfg(test)]
mod tests {
    use crate::contract::chunk_payments::quote::{Quote, Signature, SignedQuote};
    use crate::transaction::{
        get_chunk_payment_event_for_public_key_and_chunk_address_hash, get_transaction_by_hash,
        get_transaction_receipt_by_hash,
    };
    use crate::Network;
    use alloy::hex;
    use alloy::hex::FromHex;
    use alloy::primitives::{address, FixedBytes, TxHash, U256};

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

    #[tokio::test]
    async fn test_get_transaction_receipt_by_hash() {
        let network = Network::ArbitrumOne;

        let tx_hash =
            TxHash::from_hex("0358cb4d135926b28b4f831653cf92f29c7d3f12d6227cad894f2257d600f1c8")
                .unwrap();

        assert!(get_transaction_receipt_by_hash(&network, tx_hash)
            .await
            .unwrap()
            .is_some());
    }

    // TODO: Make this test pass
    #[tokio::test]
    async fn test_get_chunk_payment_event_for_public_key_and_chunk_address_hash() {
        let network = Network::ArbitrumOne;

        let chunk_address_hash: FixedBytes<32> = FixedBytes::from(hex!(
            "ac5a8cabca7e1ce296548b4db22f9ac6b385c36cb079a40d5fcbfccb08967921"
        ));

        let quote = Quote {
            chunk_address_hash,
            cost: U256::from(138),
            expiration_timestamp: U256::from(2725358967_u32),
            payment_address: address!("10a67586c328660b55ef112fa38bbe8a56de1441"),
        };

        let signed_quote = SignedQuote {
            quote,
            signature: Signature {
                r: FixedBytes::from(hex!(
                    "ddb311b86c44b900f0af5d66d27e74b048b4e0e01d0bc949fb732dbc1bb30705"
                )),
                s: FixedBytes::from(hex!(
                    "1393b9e383c387bc4c32c3bd3e1d87e2f2a3b05a4a7d3a777aa9732a1e17cdcd"
                )),
                v: 27,
            },
        };

        let public_key = signed_quote.recover_public_key().unwrap();

        let logs = get_chunk_payment_event_for_public_key_and_chunk_address_hash(
            &network,
            &public_key,
            chunk_address_hash.0,
        )
        .await
        .unwrap();

        println!("{logs:?}");
    }
}
