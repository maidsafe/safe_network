use crate::client::data::{DataAddr, PutError};
use crate::client::utils::extract_quote_payments;
use crate::self_encryption::encrypt;
use crate::Client;
use bytes::Bytes;
use sn_evm::{ProofOfPayment, QuotePayment};
use sn_networking::PayeeQuote;
use sn_protocol::storage::Chunk;
use std::collections::HashMap;
use xor_name::XorName;

#[allow(unused_imports)]
pub use sn_evm::external_signer::*;

impl Client {
    /// Upload a piece of data to the network. This data will be self-encrypted.
    /// Payment will not be done automatically as opposed to the regular `data_put`, so the proof of payment has to be provided.
    /// Returns the Data Address at which the data was stored.
    pub async fn data_put_with_proof_of_payment(
        &self,
        data: Bytes,
        proof: HashMap<XorName, ProofOfPayment>,
    ) -> Result<DataAddr, PutError> {
        let (data_map_chunk, chunks, _) = encrypt_data(data)?;
        self.upload_data_map(&proof, &data_map_chunk).await?;
        self.upload_chunks(&chunks, &proof).await?;
        Ok(*data_map_chunk.address().xorname())
    }

    /// Get quotes for data.
    /// Returns a cost map, data payments to be executed and a list of free (already paid for) chunks.
    pub async fn get_quotes_for_data(
        &self,
        data: Bytes,
    ) -> Result<
        (
            HashMap<XorName, PayeeQuote>,
            Vec<QuotePayment>,
            Vec<XorName>,
        ),
        PutError,
    > {
        // Encrypt the data as chunks
        let (_data_map_chunk, _chunks, xor_names) = encrypt_data(data)?;
        let cost_map = self.get_store_quotes(xor_names.into_iter()).await?;
        let (quote_payments, free_chunks) = extract_quote_payments(&cost_map);
        Ok((cost_map, quote_payments, free_chunks))
    }

    async fn upload_data_map(
        &self,
        payment_proofs: &HashMap<XorName, ProofOfPayment>,
        data_map_chunk: &Chunk,
    ) -> Result<(), PutError> {
        let map_xor_name = data_map_chunk.name();

        if let Some(proof) = payment_proofs.get(map_xor_name) {
            debug!("Uploading data map chunk: {map_xor_name:?}");
            self.chunk_upload_with_payment(data_map_chunk.clone(), proof.clone())
                .await
                .inspect_err(|err| error!("Error uploading data map chunk: {err:?}"))
        } else {
            Ok(())
        }
    }

    async fn upload_chunks(
        &self,
        chunks: &[Chunk],
        payment_proofs: &HashMap<XorName, ProofOfPayment>,
    ) -> Result<(), PutError> {
        debug!("Uploading {} chunks", chunks.len());
        for chunk in chunks {
            if let Some(proof) = payment_proofs.get(chunk.name()) {
                let address = *chunk.address();
                self.chunk_upload_with_payment(chunk.clone(), proof.clone())
                    .await
                    .inspect_err(|err| error!("Error uploading chunk {address:?} :{err:?}"))?;
            }
        }
        Ok(())
    }
}

/// Encrypts data as chunks.
///
/// Returns the data map chunk, file chunks and a list of all content addresses including the data map.
fn encrypt_data(data: Bytes) -> Result<(Chunk, Vec<Chunk>, Vec<XorName>), PutError> {
    let now = sn_networking::target_arch::Instant::now();
    let result = encrypt(data)?;

    debug!("Encryption took: {:.2?}", now.elapsed());

    let map_xor_name = *result.0.address().xorname();
    let mut xor_names = vec![map_xor_name];

    for chunk in &result.1 {
        xor_names.push(*chunk.name());
    }

    Ok((result.0, result.1, xor_names))
}
