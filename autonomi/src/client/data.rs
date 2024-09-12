use crate::self_encryption::{encrypt, DataMapLevel};
use crate::Client;
use bytes::Bytes;
use evmlib::common::{QuoteHash, QuotePayment, TxHash};
use evmlib::contract::chunk_payments;
use evmlib::wallet::Wallet;
use libp2p::futures;
use libp2p::kad::{Quorum, Record};
use self_encryption::{decrypt_full_set, DataMap, EncryptedChunk};
use sn_client::networking::{GetRecordCfg, PutRecordCfg};
use sn_client::networking::{Network, NetworkError, PayeeQuote};
use sn_evm::ProofOfPayment;
use sn_protocol::{
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, RecordHeader, RecordKind,
    },
    NetworkAddress,
};
use std::collections::{HashMap, HashSet};
use xor_name::XorName;

/// Errors that can occur during the put operation.
#[derive(Debug, thiserror::Error)]
pub enum PutError {
    #[error("Failed to self-encrypt data.")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("Error serializing data.")]
    Serialization,
    #[error("A network error occurred.")]
    Network(#[from] NetworkError),
    #[error("A wallet error occurred.")]
    Wallet(#[from] sn_evm::WalletError),
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
    JoinError(tokio::task::JoinError),
    #[error("Chunk payments error: {0:?}")]
    ChunkPaymentsError(#[from] chunk_payments::error::Error),
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
    Protocol(#[from] sn_client::protocol::Error),
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

    /// Upload a piece of data to the network. This data will be self-encrypted,
    /// and the data map XOR address will be returned.
    pub async fn put(&mut self, data: Bytes, wallet: &Wallet) -> Result<XorName, PutError> {
        let now = std::time::Instant::now();
        let (data_map_chunk, chunks) = encrypt(data)?;

        tracing::debug!("Encryption took: {:.2?}", now.elapsed());

        let map_xor_name = *data_map_chunk.address().xorname();
        let mut xor_names = vec![map_xor_name];

        for chunk in &chunks {
            xor_names.push(*chunk.name());
        }

        let (payment_proofs, _free_chunks) = self.pay(xor_names.into_iter(), wallet).await?;

        for chunk in chunks {
            if let Some(proof) = payment_proofs.get(chunk.name()) {
                self.upload_chunk(chunk, proof.clone()).await?;
            } else {
                // TODO: Not sure what to do with free / already paid for chunks, since we can't (re)upload without payment proofs.
            }
        }

        Ok(map_xor_name)
    }

    // Fetch and decrypt all chunks in the data map.
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

    // Unpack a wrapped data map and fetch all bytes using self-encryption.
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

    pub(crate) async fn pay(
        &mut self,
        content_addrs: impl Iterator<Item = XorName>,
        wallet: &Wallet,
    ) -> Result<(HashMap<XorName, ProofOfPayment>, Vec<XorName>), PayError> {
        let cost_map = self.get_store_quotes(content_addrs).await?;
        let (quote_payments, skipped_chunks) = extract_quote_payments(&cost_map);

        // Execute chunk payments
        let payments = wallet
            .pay_for_quotes(quote_payments)
            .await
            .map_err(PayError::ChunkPaymentsError)?;

        let proofs = construct_proofs(&cost_map, &payments);

        tracing::trace!(
            "Chunk payments of {} chunks completed. {} chunks were free / already paid for",
            proofs.len(),
            skipped_chunks.len()
        );

        Ok((proofs, skipped_chunks))
    }

    async fn get_store_quotes(
        &mut self,
        content_addrs: impl Iterator<Item = XorName>,
    ) -> Result<HashMap<XorName, PayeeQuote>, PayError> {
        let futures: Vec<_> = content_addrs
            .into_iter()
            .map(|content_addr| fetch_store_quote_with_retries(&self.network, content_addr))
            .collect();

        let quotes = futures::future::try_join_all(futures).await?;

        Ok(quotes.into_iter().collect::<HashMap<XorName, PayeeQuote>>())
    }

    /// Directly writes Chunks to the network in the form of immutable self encrypted chunks.
    async fn upload_chunk(
        &self,
        chunk: Chunk,
        proof_of_payment: ProofOfPayment,
    ) -> Result<(), PutError> {
        self.store_chunk(chunk, proof_of_payment).await?;
        Ok(())
    }

    /// Actually store a chunk to a peer.
    async fn store_chunk(&self, chunk: Chunk, payment: ProofOfPayment) -> Result<(), PutError> {
        let storing_node = payment.to_peer_id_payee();

        tracing::debug!("Storing chunk: {chunk:?} to {:?}", storing_node);

        let key = chunk.network_address().to_record_key();

        let record_kind = RecordKind::ChunkWithPayment;
        let record = Record {
            key: key.clone(),
            value: try_serialize_record(&(payment, chunk.clone()), record_kind)
                .map_err(|_| PutError::Serialization)?
                .to_vec(),
            publisher: None,
            expires: None,
        };

        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::One,
            retry_strategy: None,
            use_put_record_to: Some(vec![storing_node]),
            verification: None,
        };
        Ok(self.network.put_record(record, &put_cfg).await?)
    }
}

/// Fetch a store quote for a content address with a retry strategy.
async fn fetch_store_quote_with_retries(
    network: &Network,
    content_addr: XorName,
) -> Result<(XorName, PayeeQuote), PayError> {
    let mut retries = 0;

    loop {
        match fetch_store_quote(network, content_addr).await {
            Ok(quote) => {
                break Ok((content_addr, quote));
            }
            Err(err) if retries < 2 => {
                retries += 1;
                tracing::error!("Error while fetching store quote: {err:?}, retry #{retries}");
            }
            Err(err) => {
                tracing::error!(
                    "Error while fetching store quote: {err:?}, stopping after {retries} retries"
                );
                break Err(PayError::CouldNotGetStoreQuote(content_addr));
            }
        }
    }
}

/// Fetch a store quote for a content address.
async fn fetch_store_quote(
    network: &Network,
    content_addr: XorName,
) -> Result<PayeeQuote, NetworkError> {
    network
        .get_store_costs_from_network(
            NetworkAddress::from_chunk_address(ChunkAddress::new(content_addr)),
            vec![],
        )
        .await
}

/// Form to be executed payments and already executed payments from a cost map.
fn extract_quote_payments(
    cost_map: &HashMap<XorName, PayeeQuote>,
) -> (Vec<QuotePayment>, Vec<XorName>) {
    let mut to_be_paid = vec![];
    let mut already_paid = vec![];

    for (chunk_address, quote) in cost_map.iter() {
        if quote.2.cost.is_zero() {
            already_paid.push(*chunk_address);
        } else {
            to_be_paid.push((
                quote.2.hash(),
                quote.2.rewards_address,
                quote.2.cost.as_atto(),
            ));
        }
    }

    (to_be_paid, already_paid)
}

/// Construct payment proofs from cost map and payments map.
fn construct_proofs(
    cost_map: &HashMap<XorName, PayeeQuote>,
    payments: &HashMap<QuoteHash, TxHash>,
) -> HashMap<XorName, ProofOfPayment> {
    cost_map
        .iter()
        .filter_map(|(xor_name, (_, _, quote))| {
            payments.get(&quote.hash()).map(|tx_hash| {
                (
                    *xor_name,
                    ProofOfPayment {
                        quote: quote.clone(),
                        tx_hash: *tx_hash,
                    },
                )
            })
        })
        .collect()
}
