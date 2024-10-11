// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bytes::Bytes;
use libp2p::kad::Quorum;

use std::collections::HashSet;
use xor_name::XorName;

use super::error::{GetError, PayError, PutError};
use crate::{self_encryption::encrypt, Client};
use sn_evm::EvmWallet;
use sn_evm::{Amount, AttoTokens};
use sn_networking::{GetRecordCfg, NetworkError};
use sn_protocol::{
    storage::{try_deserialize_record, Chunk, ChunkAddress, RecordHeader, RecordKind},
    NetworkAddress,
};

/// Raw Data Address (points to a [`DataMap`])
pub type DataAddr = XorName;
/// Raw Chunk Address (points to a [`Chunk`])
pub type ChunkAddr = XorName;

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

    /// Upload a piece of data to the network. This data will be self-encrypted.
    /// Returns the Data Address at which the data was stored.
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

        // Upload data map
        if let Some(proof) = payment_proofs.get(&map_xor_name) {
            debug!("Uploading data map chunk: {map_xor_name:?}");
            self.chunk_upload_with_payment(data_map_chunk.clone(), proof.clone())
                .await
                .inspect_err(|err| error!("Error uploading data map chunk: {err:?}"))?;
        }

        // Upload the rest of the chunks
        debug!("Uploading {} chunks", chunks.len());
        for chunk in chunks {
            if let Some(proof) = payment_proofs.get(chunk.name()) {
                let address = *chunk.address();
                self.chunk_upload_with_payment(chunk, proof.clone())
                    .await
                    .inspect_err(|err| error!("Error uploading chunk {address:?} :{err:?}"))?;
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
    pub async fn data_cost(&self, data: Bytes) -> Result<AttoTokens, PayError> {
        let now = std::time::Instant::now();
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
