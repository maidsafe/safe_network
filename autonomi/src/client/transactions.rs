// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::client::data::PayError;
use crate::client::Client;
use crate::client::ClientEvent;
use crate::client::UploadSummary;

use ant_evm::Amount;
use ant_evm::AttoTokens;
pub use ant_protocol::storage::Transaction;
use ant_protocol::storage::TransactionAddress;
pub use bls::SecretKey;

use ant_evm::{EvmWallet, EvmWalletError};
use ant_networking::{GetRecordCfg, NetworkError, PutRecordCfg, VerificationKind};
use ant_protocol::{
    storage::{try_serialize_record, RecordKind, RetryStrategy},
    NetworkAddress,
};
use libp2p::kad::{Quorum, Record};

use super::data::CostError;

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("Cost error: {0}")]
    Cost(#[from] CostError),
    #[error("Network error")]
    Network(#[from] NetworkError),
    #[error("Serialization error")]
    Serialization,
    #[error("Transaction could not be verified (corrupt)")]
    FailedVerification,
    #[error("Payment failure occurred during transaction creation.")]
    Pay(#[from] PayError),
    #[error("Failed to retrieve wallet payment")]
    Wallet(#[from] EvmWalletError),
    #[error("Received invalid quote from node, this node is possibly malfunctioning, try another node by trying another transaction name")]
    InvalidQuote,
    #[error("Transaction already exists at this address: {0:?}")]
    TransactionAlreadyExists(TransactionAddress),
}

impl Client {
    /// Fetches a Transaction from the network.
    pub async fn transaction_get(
        &self,
        address: TransactionAddress,
    ) -> Result<Vec<Transaction>, TransactionError> {
        let transactions = self.network.get_transactions(address).await?;

        Ok(transactions)
    }

    pub async fn transaction_put(
        &self,
        transaction: Transaction,
        wallet: &EvmWallet,
    ) -> Result<(), TransactionError> {
        let address = transaction.address();

        // pay for the transaction
        let xor_name = address.xorname();
        debug!("Paying for transaction at address: {address:?}");
        let payment_proofs = self
            .pay(std::iter::once(*xor_name), wallet)
            .await
            .inspect_err(|err| {
                error!("Failed to pay for transaction at address: {address:?} : {err}")
            })?;

        // make sure the transaction was paid for
        let (proof, price) = match payment_proofs.get(xor_name) {
            Some((proof, price)) => (proof, price),
            None => {
                // transaction was skipped, meaning it was already paid for
                error!("Transaction at address: {address:?} was already paid for");
                return Err(TransactionError::TransactionAlreadyExists(address));
            }
        };

        // prepare the record for network storage
        let payees = proof.payees();
        let record = Record {
            key: NetworkAddress::from_transaction_address(address).to_record_key(),
            value: try_serialize_record(&(proof, &transaction), RecordKind::TransactionWithPayment)
                .map_err(|_| TransactionError::Serialization)?
                .to_vec(),
            publisher: None,
            expires: None,
        };
        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: Some(RetryStrategy::default()),
            target_record: None,
            expected_holders: Default::default(),
            is_register: false,
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: None,
            use_put_record_to: Some(payees),
            verification: Some((VerificationKind::Crdt, get_cfg)),
        };

        // put the record to the network
        debug!("Storing transaction at address {address:?} to the network");
        self.network
            .put_record(record, &put_cfg)
            .await
            .inspect_err(|err| {
                error!("Failed to put record - transaction {address:?} to the network: {err}")
            })?;

        // send client event
        if let Some(channel) = self.client_event_sender.as_ref() {
            let summary = UploadSummary {
                record_count: 1,
                tokens_spent: price.as_atto(),
            };
            if let Err(err) = channel.send(ClientEvent::UploadComplete(summary)).await {
                error!("Failed to send client event: {err}");
            }
        }

        Ok(())
    }

    /// Get the cost to create a transaction
    pub async fn transaction_cost(&self, key: SecretKey) -> Result<AttoTokens, TransactionError> {
        let pk = key.public_key();
        trace!("Getting cost for transaction of {pk:?}");

        let address = TransactionAddress::from_owner(pk);
        let xor = *address.xorname();
        let store_quote = self.get_store_quotes(std::iter::once(xor)).await?;
        let total_cost = AttoTokens::from_atto(
            store_quote
                .0
                .values()
                .map(|quote| quote.price())
                .sum::<Amount>(),
        );
        debug!("Calculated the cost to create transaction of {pk:?} is {total_cost}");
        Ok(total_cost)
    }
}
