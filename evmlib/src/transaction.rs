// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::common::{Address, Amount, QuoteHash};
use crate::contract::payment_vault::handler::PaymentVaultHandler;
use crate::quoting_metrics::QuotingMetrics;
use crate::utils::http_provider;
use crate::{contract, Network};
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
    #[error(transparent)]
    PaymentVaultError(#[from] contract::payment_vault::error::Error),
    #[error("Payment missing")]
    PaymentMissing,
}

/// Get a transaction receipt by its hash.
pub async fn get_transaction_receipt_by_hash(
    network: &Network,
    transaction_hash: TxHash,
) -> Result<Option<TransactionReceipt>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let maybe_receipt = provider
        .get_transaction_receipt(transaction_hash)
        .await
        .inspect_err(|err| error!("Error getting transaction receipt for transaction_hash: {transaction_hash:?} : {err:?}", ))?;
    debug!("Transaction receipt for {transaction_hash:?}: {maybe_receipt:?}");
    Ok(maybe_receipt)
}

/// Get a block by its block number.
async fn get_block_by_number(network: &Network, block_number: u64) -> Result<Option<Block>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let block = provider
        .get_block_by_number(
            BlockNumberOrTag::Number(block_number),
            BlockTransactionsKind::Full,
        )
        .await
        .inspect_err(|err| error!("Error getting block by number for {block_number} : {err:?}",))?;
    Ok(block)
}

/// Get transaction logs using a filter.
async fn get_transaction_logs(network: &Network, filter: Filter) -> Result<Vec<Log>, Error> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(network.rpc_url().clone());
    let logs = provider
        .get_logs(&filter)
        .await
        .inspect_err(|err| error!("Error getting logs for filter: {filter:?} : {err:?}"))?;
    Ok(logs)
}

/// Get a DataPaymentMade event, filtered by a hashed chunk address and a node address.
/// Useful for a node if it wants to check if payment for a certain chunk has been made.
async fn get_data_payment_event(
    network: &Network,
    block_number: u64,
    quote_hash: QuoteHash,
    reward_addr: Address,
    amount: U256,
) -> Result<Vec<Log>, Error> {
    debug!(
        "Getting data payment event for quote_hash: {quote_hash:?}, reward_addr: {reward_addr:?}"
    );
    let topic1: FixedBytes<32> = FixedBytes::left_padding_from(reward_addr.as_slice());

    let filter = Filter::new()
        .event_signature(DATA_PAYMENT_EVENT_SIGNATURE)
        .topic1(topic1)
        .topic2(amount)
        .topic3(quote_hash)
        .from_block(block_number)
        .to_block(block_number);

    get_transaction_logs(network, filter).await
}

/// Verify if a data payment is confirmed.
pub async fn verify_data_payment(
    network: &Network,
    quote_hash: QuoteHash,
    reward_addr: Address,
    quoting_metrics: QuotingMetrics,
) -> Result<Amount, Error> {
    let provider = http_provider(network.rpc_url().clone());
    let payment_vault = PaymentVaultHandler::new(*network.data_payments_address(), provider);

    let is_paid = payment_vault
        .verify_payment(quoting_metrics, (quote_hash, reward_addr, Amount::ZERO))
        .await?;

    let amount_paid = Amount::ZERO; // NB TODO @mick we need to get the amount paid from the contract

    if is_paid {
        Ok(amount_paid)
    } else {
        Err(Error::PaymentMissing)
    }
}

#[cfg(test)]
mod tests {
    use crate::common::Address;
    use crate::quoting_metrics::QuotingMetrics;
    use crate::transaction::verify_data_payment;
    use crate::Network;
    use alloy::hex::FromHex;
    use alloy::primitives::b256;

    #[tokio::test]
    async fn test_verify_data_payment() {
        let network = Network::ArbitrumOne;

        let quote_hash = b256!("EBD943C38C0422901D4CF22E677DD95F2591CA8D6EBFEA8BAF1BFE9FF5506ECE"); // DevSkim: ignore DS173237
        let reward_address = Address::from_hex("8AB15A43305854e4AE4E6FBEa0CD1CC0AB4ecB2A").unwrap(); // DevSkim: ignore DS173237

        let result = verify_data_payment(
            &network,
            quote_hash,
            reward_address,
            QuotingMetrics::default(),
        )
        .await;

        assert!(result.is_ok(), "Error: {:?}", result.err());
    }
}
