use std::collections::BTreeMap;

use super::transfers::SendSpendsError;
use crate::client::data::{Data, PayError, PutError};
use crate::client::ClientWrapper;
use crate::native::client::NativeClient;
use crate::self_encryption::encrypt;
use bytes::Bytes;
use libp2p::{
    kad::{Quorum, Record},
    PeerId,
};
use sn_networking::PutRecordCfg;
use sn_protocol::{
    storage::{try_serialize_record, Chunk, ChunkAddress, RecordKind},
    NetworkAddress,
};
use sn_transfers::{HotWallet, MainPubkey, NanoTokens, Payment, PaymentQuote};
use tokio::task::JoinSet;
use xor_name::XorName;

impl Data for NativeClient {}

impl NativeClient {
    /// Upload a piece of data to the network. This data will be self-encrypted,
    /// and the data map XOR address will be returned.
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

        let (_, skipped_chunks) = self.pay(xor_names.into_iter(), wallet).await?;

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

    pub(crate) async fn pay(
        &mut self,
        content_addrs: impl Iterator<Item = XorName>,
        wallet: &mut HotWallet,
    ) -> Result<(NanoTokens, Vec<XorName>), PayError> {
        let mut tasks = JoinSet::new();

        for content_addr in content_addrs {
            let network = self.network().clone();

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

        let storage_cost = if cost_map.is_empty() {
            NanoTokens::zero()
        } else {
            self.pay_for_records(&cost_map, wallet).await?
        };

        Ok((storage_cost, skipped_chunks))
    }

    async fn pay_for_records(
        &mut self,
        cost_map: &BTreeMap<XorName, (MainPubkey, PaymentQuote, Vec<u8>)>,
        wallet: &mut HotWallet,
    ) -> Result<NanoTokens, PayError> {
        // Before wallet progress, there shall be no `unconfirmed_spend_requests`
        self.resend_pending_transactions(wallet).await;

        let total_cost = wallet.local_send_storage_payment(cost_map)?;

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

        Ok(total_cost.0)
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

        Ok(self.network().put_record(record, &put_cfg).await?)
    }
}
