use crate::common::{Address, Amount, QuoteHash};
use crate::contract::payment_vault::handler::PaymentVaultHandler;
use crate::contract::payment_vault::interface::PaymentVerification;
use crate::quoting_metrics::QuotingMetrics;
use crate::utils::http_provider;
use crate::Network;

pub mod error;
pub mod handler;
pub mod implementation;
pub mod interface;

pub const MAX_TRANSFERS_PER_TRANSACTION: usize = 256;

/// Helper function to return a quote for the given quoting metrics
pub async fn get_market_price(
    network: &Network,
    quoting_metrics: QuotingMetrics,
) -> Result<Amount, error::Error> {
    let provider = http_provider(network.rpc_url().clone());
    let payment_vault = PaymentVaultHandler::new(*network.data_payments_address(), provider);
    payment_vault.get_quote(quoting_metrics).await
}

/// Helper function to verify whether a data payment is valid
pub async fn verify_data_payment(
    network: &Network,
    owned_quote_hashes: Vec<QuoteHash>,
    payment: Vec<(QuoteHash, QuotingMetrics, Address)>,
) -> Result<Amount, error::Error> {
    let provider = http_provider(network.rpc_url().clone());
    let payment_vault = PaymentVaultHandler::new(*network.data_payments_address(), provider);

    let mut amount = Amount::ZERO;

    // TODO: @mick change this for loop to a batch when the smart contract changes
    for (quote_hash, quoting_metrics, rewards_address) in payment {
        let payment_verification: PaymentVerification = payment_vault
            .verify_payment(quoting_metrics, (quote_hash, rewards_address, Amount::ZERO))
            .await
            .map(|is_valid| PaymentVerification {
                quote_hash,
                amount_paid: Amount::from(1), // TODO: update placeholder amount when the smart contract changes
                is_valid,
            })?;

        // CODE REVIEW: should we fail on a single invalid payment?
        if !payment_verification.is_valid {
            return Err(error::Error::PaymentInvalid);
        }

        if owned_quote_hashes.contains(&quote_hash) {
            amount += payment_verification.amount_paid;
        }
    }

    Ok(amount)
}
