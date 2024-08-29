use std::collections::{BTreeMap, HashSet};

use crate::self_encryption::{encrypt, DataMapLevel};
use crate::Client;
use bytes::Bytes;
use libp2p::{
    kad::{Quorum, Record},
    PeerId,
};
use self_encryption::{decrypt_full_set, DataMap, EncryptedChunk};
use sn_client::{
    networking::{GetRecordCfg, NetworkError, PutRecordCfg},
    transfers::{HotWallet, MainPubkey, NanoTokens, PaymentQuote},
    StoragePaymentResult,
};
use sn_protocol::{
    storage::{
        try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, RecordHeader, RecordKind,
    },
    NetworkAddress,
};
use sn_transfers::Payment;
use tokio::task::{JoinError, JoinSet};
use xor_name::XorName;

use super::transfers::SendSpendsError;

#[derive(Debug, thiserror::Error)]
pub enum PutError {
    #[error("Failed to self-encrypt data.")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("Error serializing data.")]
    Serialization,
    #[error("A network error occurred.")]
    Network(#[from] NetworkError),
    #[error("A wallet error occurred.")]
    Wallet(#[from] sn_transfers::WalletError),
}

#[derive(Debug, thiserror::Error)]
pub enum PayError {
    #[error("Could not get store costs: {0:?}")]
    CouldNotGetStoreCosts(sn_client::networking::NetworkError),
    #[error("Could not simultaneously fetch store costs: {0:?}")]
    JoinError(JoinError),
    #[error("Hot wallet error")]
    WalletError(#[from] sn_transfers::WalletError),
    #[error("Failed to send spends")]
    SendSpendsError(#[from] SendSpendsError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error("General networking error: {0:?}")]
    Network(#[from] sn_client::networking::NetworkError),
    #[error("General protocol error: {0:?}")]
    Protocol(#[from] sn_client::protocol::Error),
}

impl Client {
    /// Fetch a file based on the DataMap XorName.
    pub async fn get(&self, addr: XorName) -> Result<Bytes, GetError> {
        let data_map_chunk = self.fetch_chunk(addr).await?;
        let data = self
            .fetch_from_data_map_chunk(data_map_chunk.value())
            .await?;

        Ok(data)
    }

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

    pub async fn put(&mut self, data: Bytes, wallet: &mut HotWallet) -> Result<XorName, PutError> {
        let now = std::time::Instant::now();
        let (map, chunks) = encrypt(data)?;
        tracing::debug!("Encryption took: {:.2?}", now.elapsed());

        let map_xor_name = *map.address().xorname();

        let mut xor_names = vec![];
        xor_names.push(map_xor_name);
        for chunk in &chunks {
            xor_names.push(*chunk.name());
        }

        let StoragePaymentResult { skipped_chunks, .. } = self
            .pay(xor_names.into_iter(), wallet)
            .await
            .expect("TODO: handle error");

        // TODO: Upload in parallel
        if !skipped_chunks.contains(map.name()) {
            self.upload_chunk(map, wallet).await?;
        }
        for chunk in chunks {
            if skipped_chunks.contains(chunk.name()) {
                continue;
            }
            self.upload_chunk(chunk, wallet).await?;
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

        let data = decrypt_full_set(data_map, &encrypted_chunks).expect("TODO");

        Ok(data)
    }

    // Unpack a wrapped data map and fetch all bytes using self-encryption.
    async fn fetch_from_data_map_chunk(&self, data_map_bytes: &Bytes) -> Result<Bytes, GetError> {
        let mut data_map_level: DataMapLevel = rmp_serde::from_slice(data_map_bytes).expect("TODO");

        loop {
            let data_map = match &data_map_level {
                DataMapLevel::First(map) => map,
                DataMapLevel::Additional(map) => map,
            };

            let data = self.fetch_from_data_map(data_map).await?;

            match &data_map_level {
                DataMapLevel::First(_) => break Ok(data),
                DataMapLevel::Additional(_) => {
                    data_map_level = rmp_serde::from_slice(&data).expect("TODO");
                    continue;
                }
            };
        }
    }

    pub(crate) async fn pay(
        &mut self,
        content_addrs: impl Iterator<Item = XorName>,
        wallet: &mut HotWallet,
    ) -> Result<StoragePaymentResult, PayError> {
        let mut tasks = JoinSet::new();
        for content_addr in content_addrs {
            let network = self.network.clone();
            tasks.spawn(async move {
                // TODO: retry, but where?
                let cost = network
                    .get_store_costs_from_network(
                        NetworkAddress::from_chunk_address(ChunkAddress::new(content_addr)),
                        vec![],
                    )
                    .await
                    .map_err(PayError::CouldNotGetStoreCosts);

                tracing::debug!("Storecosts retrieved for {content_addr:?} {cost:?}");
                (content_addr, cost)
            });
        }
        tracing::debug!("Pending store cost tasks: {:?}", tasks.len());

        // collect store costs
        let mut cost_map = BTreeMap::default();
        let mut skipped_chunks = vec![];
        while let Some(res) = tasks.join_next().await {
            match res {
                Ok((content_addr, Ok(cost))) => {
                    if cost.2.cost == NanoTokens::zero() {
                        skipped_chunks.push(content_addr);
                        tracing::debug!("Skipped existing chunk {content_addr:?}");
                    } else {
                        tracing::debug!("Storecost inserted into payment map for {content_addr:?}");
                        let _ = cost_map.insert(content_addr, (cost.1, cost.2, cost.0.to_bytes()));
                    }
                }
                Ok((content_addr, Err(err))) => {
                    tracing::warn!("Cannot get store cost for {content_addr:?} with error {err:?}");
                    return Err(err);
                }
                Err(e) => {
                    return Err(PayError::JoinError(e));
                }
            }
        }

        let (storage_cost, royalty_fees) = self.pay_for_records(&cost_map, wallet).await?;
        let res = StoragePaymentResult {
            storage_cost,
            royalty_fees,
            skipped_chunks,
        };
        Ok(res)
    }

    async fn pay_for_records(
        &mut self,
        cost_map: &BTreeMap<XorName, (MainPubkey, PaymentQuote, Vec<u8>)>,
        wallet: &mut HotWallet,
    ) -> Result<(NanoTokens, NanoTokens), PayError> {
        // Before wallet progress, there shall be no `unconfirmed_spend_requests`
        self.resend_pending_transactions(wallet).await;

        let total_cost = wallet.local_send_storage_payment(cost_map)?;

        tracing::trace!(
            "local_send_storage_payment of {} chunks completed",
            cost_map.len(),
        );

        // send to network
        tracing::trace!("Sending storage payment transfer to the network");
        let spend_attempt_result = self
            .send_spends(wallet.unconfirmed_spend_requests().iter())
            .await;

        tracing::trace!("send_spends of {} chunks completed", cost_map.len(),);

        // Here is bit risky that for the whole bunch of spends to the chunks' store_costs and royalty_fee
        // they will get re-paid again for ALL, if any one of the payment failed to be put.
        if let Err(error) = spend_attempt_result {
            tracing::warn!("The storage payment transfer was not successfully registered in the network: {error:?}. It will be retried later.");

            // if we have a DoubleSpend error, lets remove the CashNote from the wallet
            if let SendSpendsError::DoubleSpendAttemptedForCashNotes(spent_cash_notes) = &error {
                for cash_note_key in spent_cash_notes {
                    tracing::warn!(
                        "Removing double spends CashNote from wallet: {cash_note_key:?}"
                    );
                    wallet.mark_notes_as_spent([cash_note_key]);
                    wallet.clear_specific_spend_request(*cash_note_key);
                }
            }

            wallet.store_unconfirmed_spend_requests()?;

            return Err(PayError::SendSpendsError(error));
        } else {
            tracing::info!("Spend has completed: {:?}", spend_attempt_result);
            wallet.clear_confirmed_spend_requests();
        }
        tracing::trace!("clear up spends of {} chunks completed", cost_map.len(),);

        Ok(total_cost)
    }

    /// Directly writes Chunks to the network in the form of immutable self encrypted chunks.
    async fn upload_chunk(&self, chunk: Chunk, wallet: &mut HotWallet) -> Result<(), PutError> {
        let xor_name = *chunk.name();
        let (payment, payee) = self.get_recent_payment_for_addr(&xor_name, wallet)?;

        self.store_chunk(chunk, payee, payment).await?;

        wallet.api().remove_payment_transaction(&xor_name);

        Ok(())
    }

    /// Actually store a chunk to a peer.
    async fn store_chunk(
        &self,
        chunk: Chunk,
        payee: PeerId,
        payment: Payment,
    ) -> Result<(), PutError> {
        tracing::debug!("Storing chunk: {chunk:?} to {payee:?}");

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
            use_put_record_to: Some(vec![payee]),
            verification: None,
        };
        Ok(self.network.put_record(record, &put_cfg).await?)
    }
}
