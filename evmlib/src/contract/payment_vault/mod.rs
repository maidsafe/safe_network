use crate::common::Amount;
use crate::contract::payment_vault::handler::PaymentVaultHandler;
use crate::quoting_metrics::QuotingMetrics;
use crate::utils::http_provider;
use crate::Network;

pub mod error;
pub mod handler;
pub mod implementation;
pub mod interface;

pub const MAX_TRANSFERS_PER_TRANSACTION: usize = 256;

/// Helper function to return a quote for the given quoting metrics
pub async fn get_quote(
    network: &Network,
    quoting_metrics: QuotingMetrics,
) -> Result<Amount, error::Error> {
    let provider = http_provider(network.rpc_url().clone());
    let payment_vault = PaymentVaultHandler::new(*network.data_payments_address(), provider);
    payment_vault.get_quote(quoting_metrics).await
}
