// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod error;

use crate::common;
use crate::common::{Address, TxHash};
use crate::contract::data_payments::error::Error;
use crate::contract::data_payments::DataPaymentsContract::DataPaymentsContractInstance;
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::Transport;

/// The max amount of transfers within one data payments transaction.
pub const MAX_TRANSFERS_PER_TRANSACTION: usize = 512;

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    DataPaymentsContract,
    "artifacts/DataPayments.json"
);

pub struct DataPaymentsHandler<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    pub contract: DataPaymentsContractInstance<T, P, N>,
}

impl<T, P, N> DataPaymentsHandler<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    /// Create a new ChunkPayments contract instance.
    pub fn new(contract_address: Address, provider: P) -> Self {
        let contract = DataPaymentsContract::new(contract_address, provider);
        DataPaymentsHandler { contract }
    }

    /// Deploys the ChunkPayments smart contract to the network of the provider.
    /// ONLY DO THIS IF YOU KNOW WHAT YOU ARE DOING!
    pub async fn deploy(provider: P, payment_token_address: Address) -> Self {
        let contract = DataPaymentsContract::deploy(provider, payment_token_address)
            .await
            .expect("Could not deploy contract");

        DataPaymentsHandler { contract }
    }

    pub fn set_provider(&mut self, provider: P) {
        let address = *self.contract.address();
        self.contract = DataPaymentsContract::new(address, provider);
    }

    /// Pay for quotes.
    /// Input: (quote_id, reward_address, amount).
    pub async fn pay_for_quotes<I: IntoIterator<Item = common::QuotePayment>>(
        &self,
        data_payments: I,
    ) -> Result<TxHash, Error> {
        let data_payments: Vec<DataPayments::DataPayment> = data_payments
            .into_iter()
            .map(|(hash, addr, amount)| DataPayments::DataPayment {
                rewardsAddress: addr,
                amount,
                quoteHash: hash,
            })
            .collect();

        if data_payments.len() > MAX_TRANSFERS_PER_TRANSACTION {
            error!(
                "Data payments limit exceeded: {} > {}",
                data_payments.len(),
                MAX_TRANSFERS_PER_TRANSACTION
            );
            return Err(Error::TransferLimitExceeded);
        }

        let tx_hash = self
            .contract
            .submitDataPayments(data_payments)
            .send()
            .await
            .inspect_err(|e| error!("Failed to submit data payments during pay_for_quotes: {e:?}"))?
            .watch()
            .await
            .inspect_err(|e| {
                error!("Failed to watch data payments during pay_for_quotes: {e:?}")
            })?;

        Ok(tx_hash)
    }
}
