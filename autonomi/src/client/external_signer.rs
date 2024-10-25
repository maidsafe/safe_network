use crate::client::data::PutError;
use crate::client::utils::extract_quote_payments;
use crate::self_encryption::encrypt;
use crate::Client;
use bytes::Bytes;
use sn_evm::{PaymentQuote, QuotePayment};
use sn_protocol::storage::Chunk;
use std::collections::HashMap;
use xor_name::XorName;

use crate::utils::cost_map_to_quotes;
#[allow(unused_imports)]
pub use sn_evm::external_signer::*;

impl Client {
    /// Get quotes for data.
    /// Returns a cost map, data payments to be executed and a list of free (already paid for) chunks.
    pub async fn get_quotes_for_data(
        &self,
        data: Bytes,
    ) -> Result<
        (
            HashMap<XorName, PaymentQuote>,
            Vec<QuotePayment>,
            Vec<XorName>,
        ),
        PutError,
    > {
        // Encrypt the data as chunks
        let (_data_map_chunk, _chunks, xor_names) = encrypt_data(data)?;
        let cost_map = self.get_store_quotes(xor_names.into_iter()).await?;
        let (quote_payments, free_chunks) = extract_quote_payments(&cost_map);
        let quotes = cost_map_to_quotes(cost_map);

        Ok((quotes, quote_payments, free_chunks))
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
