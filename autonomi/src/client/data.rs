// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bytes::Bytes;
use futures::StreamExt as _;
use libp2p::kad::Quorum;

use std::collections::HashSet;
use xor_name::XorName;

use crate::client::{ClientEvent, UploadSummary};
use crate::{self_encryption::encrypt, Client};
use sn_evm::{Amount, AttoTokens};
use sn_evm::{EvmWallet, EvmWalletError};
use sn_networking::{GetRecordCfg, NetworkError};
use sn_protocol::{
    storage::{try_deserialize_record, Chunk, ChunkAddress, RecordHeader, RecordKind},
    NetworkAddress,
};

/// Raw Data Address (points to a DataMap)
pub type DataAddr = XorName;
/// Raw Chunk Address (points to a [`Chunk`])
pub type ChunkAddr = XorName;

/// Errors that can occur during the put operation.
#[derive(Debug, thiserror::Error)]
pub enum PutError {
    #[error("Failed to self-encrypt data.")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("A network error occurred.")]
    Network(#[from] NetworkError),
    #[error("Error occurred during cost estimation.")]
    CostError(#[from] CostError),
    #[error("Error occurred during payment.")]
    PayError(#[from] PayError),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("A wallet error occurred.")]
    Wallet(#[from] sn_evm::EvmError),
    #[error("The vault owner key does not match the client's public key")]
    VaultBadOwner,
    #[error("Payment unexpectedly invalid for {0:?}")]
    PaymentUnexpectedlyInvalid(NetworkAddress),
}

/// Errors that can occur during the pay operation.
#[derive(Debug, thiserror::Error)]
pub enum PayError {
    #[error("Wallet error: {0:?}")]
    EvmWalletError(#[from] EvmWalletError),
    #[error("Failed to self-encrypt data.")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("Cost error: {0:?}")]
    Cost(#[from] CostError),
}

/// Errors that can occur during the get operation.
#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error("Could not deserialize data map.")]
    InvalidDataMap(rmp_serde::decode::Error),
    #[error("Failed to decrypt data.")]
    Decryption(crate::self_encryption::Error),
    #[error("Failed to deserialize")]
    Deserialization(#[from] rmp_serde::decode::Error),
    #[error("General networking error: {0:?}")]
    Network(#[from] NetworkError),
    #[error("General protocol error: {0:?}")]
    Protocol(#[from] sn_protocol::Error),
}

/// Errors that can occur during the cost calculation.
#[derive(Debug, thiserror::Error)]
pub enum CostError {
    #[error("Failed to self-encrypt data.")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("Could not get store quote for: {0:?} after several retries")]
    CouldNotGetStoreQuote(XorName),
    #[error("Could not get store costs: {0:?}")]
    CouldNotGetStoreCosts(NetworkError),
    #[error("Failed to serialize {0}")]
    Serialization(String),
}

impl Client {
    /// Fetch a blob of data from the network
    pub async fn data_get(&self, addr: DataAddr) -> Result<Bytes, GetError> {
        info!("Fetching data from Data Address: {addr:?}");
        let data_map_chunk = self.chunk_get(addr).await?;
        let data = self
            .fetch_from_data_map_chunk(data_map_chunk.value())
            .await?;

        Ok(data)
    }

    /// Upload a piece of data to the network.
    /// Returns the Data Address at which the data was stored.
    /// This data is publicly accessible.
    pub async fn data_put(&self, data: Bytes, wallet: &EvmWallet) -> Result<DataAddr, PutError> {
        let now = sn_networking::target_arch::Instant::now();
        let (data_map_chunk, chunks) = encrypt(data)?;
        info!(
            "Uploading datamap chunk to the network at: {:?}",
            data_map_chunk.address()
        );

        debug!("Encryption took: {:.2?}", now.elapsed());

        let map_xor_name = *data_map_chunk.address().xorname();
        let mut xor_names = vec![map_xor_name];

        for chunk in &chunks {
            xor_names.push(*chunk.name());
        }

        // Pay for all chunks + data map chunk
        info!("Paying for {} addresses", xor_names.len());
        let (payment_proofs, _free_chunks) = self
            .pay(xor_names.into_iter(), wallet)
            .await
            .inspect_err(|err| error!("Error paying for data: {err:?}"))?;

        let mut record_count = 0;

        // Upload all the chunks in parallel including the data map chunk
        debug!("Uploading {} chunks", chunks.len());
        let mut tasks = futures::stream::FuturesUnordered::new();

        for chunk in chunks.into_iter().chain(std::iter::once(data_map_chunk)) {
            let self_clone = self.clone();
            let address = *chunk.address();
            if let Some(proof) = payment_proofs.get(chunk.name()) {
                let proof_clone = proof.clone();
                tasks.push(async move {
                    self_clone
                        .chunk_upload_with_payment(chunk, proof_clone)
                        .await
                        .inspect_err(|err| error!("Error uploading chunk {address:?} :{err:?}"))
                });
            } else {
                debug!("Chunk at {address:?} was already paid for so skipping");
            }
        }
        while let Some(result) = tasks.next().await {
            result.inspect_err(|err| error!("Error uploading chunk: {err:?}"))?;
            record_count += 1;
        }

        if let Some(channel) = self.client_event_sender.as_ref() {
            let tokens_spent = payment_proofs
                .values()
                .map(|proof| proof.quote.cost.as_atto())
                .sum::<Amount>();

            let summary = UploadSummary {
                record_count,
                tokens_spent,
            };
            if let Err(err) = channel.send(ClientEvent::UploadComplete(summary)).await {
                error!("Failed to send client event: {err:?}");
            }
        }

        Ok(map_xor_name)
    }

    /// Get a raw chunk from the network.
    pub async fn chunk_get(&self, addr: ChunkAddr) -> Result<Chunk, GetError> {
        info!("Getting chunk: {addr:?}");

        let key = NetworkAddress::from_chunk_address(ChunkAddress::new(addr)).to_record_key();

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: None,
            target_record: None,
            expected_holders: HashSet::new(),
            is_register: false,
        };

        let record = self
            .network
            .get_record_from_network(key, &get_cfg)
            .await
            .inspect_err(|err| error!("Error fetching chunk: {err:?}"))?;
        let header = RecordHeader::from_record(&record)?;

        if let RecordKind::Chunk = header.kind {
            let chunk: Chunk = try_deserialize_record(&record)?;
            Ok(chunk)
        } else {
            Err(NetworkError::RecordKindMismatch(RecordKind::Chunk).into())
        }
    }

    /// Get the estimated cost of storing a piece of data.
    pub async fn data_cost(&self, data: Bytes) -> Result<AttoTokens, CostError> {
        let now = sn_networking::target_arch::Instant::now();
        let (data_map_chunk, chunks) = encrypt(data)?;

        debug!("Encryption took: {:.2?}", now.elapsed());

        let map_xor_name = *data_map_chunk.address().xorname();
        let mut content_addrs = vec![map_xor_name];

        for chunk in &chunks {
            content_addrs.push(*chunk.name());
        }

        info!(
            "Calculating cost of storing {} chunks. Data map chunk at: {map_xor_name:?}",
            content_addrs.len()
        );

        let cost_map = self
            .get_store_quotes(content_addrs.into_iter())
            .await
            .inspect_err(|err| error!("Error getting store quotes: {err:?}"))?;
        let total_cost = AttoTokens::from_atto(
            cost_map
                .values()
                .map(|quote| quote.2.cost.as_atto())
                .sum::<Amount>(),
        );
        Ok(total_cost)
    }
}
