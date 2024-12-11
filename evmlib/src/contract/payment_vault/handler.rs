use crate::common::{Address, Amount, Calldata, TxHash};
use crate::contract::payment_vault::error::Error;
use crate::contract::payment_vault::interface::IPaymentVault::IPaymentVaultInstance;
use crate::contract::payment_vault::interface::{
    IPaymentVault, REQUIRED_PAYMENT_VERIFICATION_LENGTH,
};
use alloy::network::{Network, TransactionBuilder};
use alloy::providers::Provider;
use alloy::transports::Transport;

pub struct PaymentVaultHandler<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    pub contract: IPaymentVaultInstance<T, P, N>,
}

impl<T, P, N> PaymentVaultHandler<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    /// Create a new PaymentVaultHandler instance from a (proxy) contract's address
    pub fn new(contract_address: Address, provider: P) -> Self {
        let contract = IPaymentVault::new(contract_address, provider);
        Self { contract }
    }

    /// Set the provider
    pub fn set_provider(&mut self, provider: P) {
        let address = *self.contract.address();
        self.contract = IPaymentVault::new(address, provider);
    }

    /// Fetch a quote from the contract
    pub async fn get_quote<I: IntoIterator<Item: Into<IPaymentVault::QuotingMetrics>>>(
        &self,
        metrics: I,
    ) -> Result<Vec<Amount>, Error> {
        // NB TODO @mick we need to batch this smart contract call
        let mut amounts = vec![];

        // set rate limit to 2 req/s
        const TIME_BETWEEN_RPC_CALLS_IN_MS: u64 = 700;
        let mut maybe_last_call: Option<std::time::Instant> = None;
        for metric in metrics {
            // check if we have to wait for the rate limit
            if let Some(last_call) = maybe_last_call {
                let elapsed = std::time::Instant::now() - last_call;
                let time_to_sleep_ms = TIME_BETWEEN_RPC_CALLS_IN_MS as u128 - elapsed.as_millis();
                if time_to_sleep_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(time_to_sleep_ms as u64))
                        .await;
                }
            }

            let amount = self.contract.getQuote(metric.into()).call().await?.price;
            amounts.push(amount);
            maybe_last_call = Some(std::time::Instant::now());
        }

        Ok(amounts)
    }

    /// Pay for quotes.
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

    /// Returns the pay for quotes transaction calldata.
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

    /// Verify if payments are valid
    pub async fn verify_payment<I: IntoIterator<Item: Into<IPaymentVault::PaymentVerification>>>(
        &self,
        payment_verifications: I,
    ) -> Result<[IPaymentVault::PaymentVerificationResult; 3], Error> {
        let payment_verifications: Vec<IPaymentVault::PaymentVerification> = payment_verifications
            .into_iter()
            .map(|v| v.into())
            .collect();

        if payment_verifications.len() != REQUIRED_PAYMENT_VERIFICATION_LENGTH {
            return Err(Error::PaymentVerificationLengthInvalid);
        }

        let results = self
            .contract
            .verifyPayment(payment_verifications)
            .call()
            .await?
            .verificationResults;

        Ok(results)
    }
}
