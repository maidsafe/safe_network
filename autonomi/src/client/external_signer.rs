use crate::client::data::PutError;
use crate::self_encryption::encrypt;
use crate::Client;
use ant_evm::QuotePayment;
use ant_protocol::storage::Chunk;
use bytes::Bytes;
use std::collections::HashMap;
use xor_name::XorName;

#[allow(unused_imports)]
pub use ant_evm::external_signer::*;

use super::quote::QuoteForAddress;

impl Client {
    /// Get quotes for data.
    /// Returns a cost map, data payments to be executed and a list of free (already paid for) chunks.
    pub async fn get_quotes_for_content_addresses(
        &self,
        content_addrs: impl Iterator<Item = XorName> + Clone,
    ) -> Result<
        (
            HashMap<XorName, QuoteForAddress>,
            Vec<QuotePayment>,
            Vec<XorName>,
        ),
        PutError,
    > {
        let quote = self.get_store_quotes(content_addrs.clone()).await?;
        let payments = quote.payments();
        let free_chunks = content_addrs.filter(|addr| !quote.0.contains_key(addr)).collect();
        let quotes_per_addr = quote.0.into_iter().collect();

        debug!(
            "Got the quotes , quote_payments and freechunks from the network {:?}",
            (quotes_per_addr.clone(), payments.clone(), free_chunks.clone())
        );
        Ok((quotes_per_addr, payments, free_chunks))
    }
}

/// Encrypts data as chunks.
///
/// Returns the data map chunk and file chunks.
pub fn encrypt_data(data: Bytes) -> Result<(Chunk, Vec<Chunk>), PutError> {
    let now = ant_networking::target_arch::Instant::now();
    let result = encrypt(data)?;

    debug!("Encryption took: {:.2?}", now.elapsed());

    Ok((result.0, result.1))
}
