mod error;

use crate::common::{Address, Amount, Calldata, TxHash};
use crate::contract::payment_vault::error::Error;
use crate::contract::payment_vault::IPaymentVault::{IPaymentVaultInstance, QuotingMetrics};
use alloy::network::{Network, TransactionBuilder};
use alloy::providers::Provider;
use alloy::sol;
use alloy::transports::Transport;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IPaymentVault,
    "abi/IPaymentVault.json"
);

pub struct PaymentVaultHandler<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    pub contract: IPaymentVaultInstance<T, P, N>,
}

impl<T, P, N> PaymentVaultHandler<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    /// Create a new PaymentVaultHandler instance from a deployed contract's address
    pub fn new(contract_address: Address, provider: P) -> Self {
        let contract = IPaymentVault::new(contract_address, provider);
        Self { contract }
    }

    /// Fetch a quote from the contract
    pub async fn fetch_quote(&self, metrics: QuotingMetrics) -> Result<Amount, Error> {
        let amount = self.contract.getQuote(metrics).call().await?.price;
        Ok(amount)
    }

    /// Pay for quotes.
    /// Input: (quote_hash, reward_address, amount).
    pub async fn pay_for_quotes<I: IntoIterator<Item: Into<IPaymentVault::DataPayment>>>(
        &self,
        data_payments: I,
    ) -> Result<TxHash, Error> {
        let (calldata, to) = self.pay_for_quotes_calldata(data_payments)?;

        let transaction_request = self
            .contract
            .provider()
            .transaction_request()
            .with_to(to)
            .with_input(calldata);

        let tx_hash = self
            .contract
            .provider()
            .send_transaction(transaction_request)
            .await?
            .watch()
            .await?;

        Ok(tx_hash)
    }

    /// Pay for quotes.
    /// Input: (quote_hash, reward_address, amount).
    /// Returns the transaction calldata.
    pub fn pay_for_quotes_calldata<I: IntoIterator<Item: Into<IPaymentVault::DataPayment>>>(
        &self,
        data_payments: I,
    ) -> Result<(Calldata, Address), Error> {
        let data_payments: Vec<IPaymentVault::DataPayment> =
            data_payments.into_iter().map(|item| item.into()).collect();

        let calldata = self
            .contract
            .payForQuotes(data_payments)
            .calldata()
            .to_owned();

        Ok((calldata, *self.contract.address()))
    }

    /// Verify if a payment is valid
    pub async fn validate_payment<I: Into<IPaymentVault::DataPayment>>(
        &self,
        metrics: QuotingMetrics,
        payment: I,
    ) -> Result<bool, Error> {
        let is_valid = self
            .contract
            .verifyPayment(metrics, payment.into())
            .call()
            .await?
            .isValid;

        Ok(is_valid)
    }
}
