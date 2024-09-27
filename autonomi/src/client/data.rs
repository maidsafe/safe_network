use crate::client::{Client, ClientWrapper};
use crate::self_encryption::DataMapLevel;
use bytes::Bytes;
use evmlib::wallet;
use libp2p::kad::Quorum;
use self_encryption::{decrypt_full_set, DataMap, EncryptedChunk};
use sn_protocol::storage::{try_deserialize_record, Chunk, ChunkAddress, RecordHeader, RecordKind};
use sn_protocol::NetworkAddress;
use std::collections::HashSet;
use sn_networking::{GetRecordCfg, NetworkError, PutRecordCfg};
use sn_transfers::Payment;
use sn_transfers::{HotWallet, MainPubkey, NanoTokens, PaymentQuote};
use tokio::task::{JoinError, JoinSet};
use xor_name::XorName;

/// Errors that can occur during the put operation.
#[derive(Debug, thiserror::Error)]
pub enum PutError {
    #[error("Failed to self-encrypt data.")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("Error serializing data.")]
    Serialization,
    #[error("Error getting Vault XorName data.")]
    VaultXorName,
    #[error("A network error occurred.")]
    Network(#[from] NetworkError),
    #[cfg(feature = "native-payments")]
    #[error("A wallet error occurred.")]
    Wallet(#[from] sn_transfers::WalletError),
    #[cfg(feature = "evm-payments")]
    #[error("A wallet error occurred.")]
    EvmWallet(#[from] sn_evm::EvmError),
    #[error("Error occurred during payment.")]
    PayError(#[from] PayError),
}

/// Errors that can occur during the pay operation.
#[derive(Debug, thiserror::Error)]
pub enum PayError {
    #[error("Could not get store quote for: {0:?} after several retries")]
    CouldNotGetStoreQuote(XorName),
    #[error("Could not get store costs: {0:?}")]
    CouldNotGetStoreCosts(NetworkError),
    #[error("Could not simultaneously fetch store costs: {0:?}")]
    JoinError(JoinError),
    #[cfg(feature = "native-payments")]
    #[error("Hot wallet error")]
    WalletError(#[from] sn_transfers::WalletError),
    #[cfg(feature = "evm-payments")]
    #[error("Wallet error: {0:?}")]
    EvmWalletError(#[from] wallet::Error),
    #[cfg(feature = "native-payments")]
    #[error("Failed to send spends")]
    SendSpendsError(#[from] crate::native::client::transfers::SendSpendsError),
}

/// Errors that can occur during the get operation.
#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error("Could not deserialize data map.")]
    InvalidDataMap(rmp_serde::decode::Error),
    #[error("Failed to decrypt data.")]
    Decryption(crate::self_encryption::Error),
    #[error("General networking error: {0:?}")]
    Network(#[from] NetworkError),
    #[error("General protocol error: {0:?}")]
    Protocol(#[from] sn_protocol::Error),
}

impl Client {
    /// Fetch a piece of self-encrypted data from the network, by its data map
    /// XOR address.
    pub async fn get(&self, data_map_addr: XorName) -> Result<Bytes, GetError> {
        let data_map_chunk = self.fetch_chunk(data_map_addr).await?;
        let data = self
            .fetch_from_data_map_chunk(data_map_chunk.value())
            .await?;

        Ok(data)
    }

    /// Get a raw chunk from the network.
    pub async fn fetch_chunk(&self, addr: XorName) -> Result<Chunk, GetError> {
        tracing::info!("Getting chunk: {addr:?}");

        let key = NetworkAddress::from_chunk_address(ChunkAddress::new(addr)).to_record_key();

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: None,
            target_record: None,
            expected_holders: HashSet::new(),
            is_register: false,
        };

        let record = self.network.get_record_from_network(key, &get_cfg).await?;
        let header = RecordHeader::from_record(&record)?;

        if let RecordKind::Chunk = header.kind {
            let chunk: Chunk = try_deserialize_record(&record)?;
            Ok(chunk)
        } else {
            Err(NetworkError::RecordKindMismatch(RecordKind::Chunk).into())
        }
    }

    /// Fetch and decrypt all chunks in the data map.
    async fn fetch_from_data_map(&self, data_map: &DataMap) -> Result<Bytes, GetError> {
        let mut encrypted_chunks = vec![];

        for info in data_map.infos() {
            let chunk = self.fetch_chunk(info.dst_hash).await?;
            let chunk = EncryptedChunk {
                index: info.index,
                content: chunk.value,
            };
            encrypted_chunks.push(chunk);
        }

        let data = decrypt_full_set(data_map, &encrypted_chunks)
            .map_err(|e| GetError::Decryption(crate::self_encryption::Error::SelfEncryption(e)))?;

        Ok(data)
    }

    /// Unpack a wrapped data map and fetch all bytes using self-encryption.
    async fn fetch_from_data_map_chunk(&self, data_map_bytes: &Bytes) -> Result<Bytes, GetError> {
        let mut data_map_level: DataMapLevel =
            rmp_serde::from_slice(data_map_bytes).map_err(GetError::InvalidDataMap)?;

        loop {
            let data_map = match &data_map_level {
                DataMapLevel::First(map) => map,
                DataMapLevel::Additional(map) => map,
            };

            let data = self.fetch_from_data_map(data_map).await?;

            match &data_map_level {
                DataMapLevel::First(_) => break Ok(data),
                DataMapLevel::Additional(_) => {
                    data_map_level =
                        rmp_serde::from_slice(&data).map_err(GetError::InvalidDataMap)?;
                    continue;
                }
            };
        }
    }
}

pub trait Data: ClientWrapper {
    async fn get(&self, data_map_addr: XorName) -> Result<Bytes, GetError> {
        self.client().get(data_map_addr).await
    }

    async fn fetch_chunk(&self, addr: XorName) -> Result<Chunk, GetError> {
        self.client().fetch_chunk(addr).await
    }

    async fn fetch_from_data_map(&self, data_map: &DataMap) -> Result<Bytes, GetError> {
        self.client().fetch_from_data_map(data_map).await
    }

    async fn fetch_from_data_map_chunk(&self, data_map_bytes: &Bytes) -> Result<Bytes, GetError> {
        self.client()
            .fetch_from_data_map_chunk(data_map_bytes)
            .await
    }
}
