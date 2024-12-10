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

pub use ant_protocol::storage::Transaction;
use ant_protocol::storage::TransactionAddress;
pub use bls::SecretKey as TransactionSecretKey;

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

        let xor_name = address.xorname();
        debug!("Paying for transaction at address: {address:?}");
        let (payment_proofs, _skipped) = self
            .pay(std::iter::once(*xor_name), wallet)
            .await
            .inspect_err(|err| {
                error!("Failed to pay for transaction at address: {address:?} : {err}")
            })?;
        let proof = if let Some(proof) = payment_proofs.get(xor_name) {
            proof
        } else {
            // transaction was skipped, meaning it was already paid for
            error!("Transaction at address: {address:?} was already paid for");
            return Err(TransactionError::Network(
                NetworkError::TransactionAlreadyExists,
            ));
        };
        let payee = proof
            .to_peer_id_payee()
            .ok_or(TransactionError::InvalidQuote)
            .inspect_err(|err| error!("Failed to get payee from payment proof: {err}"))?;

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
            use_put_record_to: Some(vec![payee]),
            verification: Some((VerificationKind::Network, get_cfg)),
        };

        debug!("Storing transaction at address {address:?} to the network");
        self.network
            .put_record(record, &put_cfg)
            .await
            .inspect_err(|err| {
                error!("Failed to put record - transaction {address:?} to the network: {err}")
            })?;

        if let Some(channel) = self.client_event_sender.as_ref() {
            let summary = UploadSummary {
                record_count: 1,
                tokens_spent: proof.quote.cost.as_atto(),
            };
            if let Err(err) = channel.send(ClientEvent::UploadComplete(summary)).await {
                error!("Failed to send client event: {err}");
            }
        }

        Ok(())
    }

    // /// Get the cost to create a transaction
    // pub async fn transaction_cost(
    //     &self,
    //     name: String,
    //     owner: TransactionSecretKey,
    // ) -> Result<AttoTokens, TransactionError> {
    //     info!("Getting cost for transaction with name: {name}");
    //     // get transaction address
    //     let pk = owner.public_key();
    //     let name = XorName::from_content_parts(&[name.as_bytes()]);
    //     let transaction = Transaction::new(None, name, owner, permissions)?;
    //     let reg_xor = transaction.address().xorname();

    //     // get cost to store transaction
    //     // NB TODO: transaction should be priced differently from other data
    //     let cost_map = self.get_store_quotes(std::iter::once(reg_xor)).await?;
    //     let total_cost = AttoTokens::from_atto(
    //         cost_map
    //             .values()
    //             .map(|quote| quote.2.cost.as_atto())
    //             .sum::<Amount>(),
    //     );
    //     debug!("Calculated the cost to create transaction with name: {name} is {total_cost}");
    //     Ok(total_cost)
    // }
}
