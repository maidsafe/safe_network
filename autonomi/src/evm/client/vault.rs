use crate::client::data::PutError;
use crate::client::vault::Vault;
use crate::client::ClientWrapper;
use crate::evm::client::EvmClient;
use bytes::Bytes;
use evmlib::wallet::Wallet;
use libp2p::kad::{Quorum, Record};
use sn_networking::{GetRecordCfg, PutRecordCfg, VerificationKind};
use sn_protocol::storage::{try_serialize_record, RecordKind, RetryStrategy, Scratchpad};
use std::collections::HashSet;
use tracing::info;

impl Vault for EvmClient {}

impl EvmClient {
    /// Put data into the client's VaultPacket
    ///
    /// Returns Ok(None) early if no vault packet is defined.
    ///
    /// Pays for a new VaultPacket if none yet created for the client. Returns the current version
    /// of the data on success.
    pub async fn write_bytes_to_vault_if_defined(
        &mut self,
        data: Bytes,
        wallet: &mut Wallet,
    ) -> Result<Option<u64>, PutError> {
        // Exit early if no vault packet defined
        let Some(client_sk) = self.client().vault_secret_key.as_ref() else {
            return Ok(None);
        };

        let client_pk = client_sk.public_key();

        let pad_res = self.get_vault_from_network().await;
        let mut is_new = true;

        let mut scratch = if let Ok(existing_data) = pad_res {
            tracing::info!("Scratchpad already exists, returning existing data");

            info!(
                "scratch already exists, is version {:?}",
                existing_data.count()
            );

            is_new = false;
            existing_data
        } else {
            tracing::trace!("new scratchpad creation");
            Scratchpad::new(client_pk)
        };

        let next_count = scratch.update_and_sign(data, client_sk);
        let scratch_address = scratch.network_address();
        let scratch_key = scratch_address.to_record_key();

        let record = if is_new {
            self.pay(
                [&scratch_address].iter().filter_map(|f| f.as_xorname()),
                wallet,
            )
            .await?;

            let scratch_xor = scratch_address.as_xorname().ok_or(PutError::VaultXorName)?;
            let (payment_proofs, _) = self.pay(std::iter::once(scratch_xor), wallet).await?;
            // Should always be there, else it would have failed on the payment step.
            let proof = payment_proofs.get(&scratch_xor).expect("Missing proof");

            Record {
                key: scratch_key,
                value: try_serialize_record(&(proof, scratch), RecordKind::ScratchpadWithPayment)
                    .map_err(|_| PutError::Serialization)?
                    .to_vec(),
                publisher: None,
                expires: None,
            }
        } else {
            Record {
                key: scratch_key,
                value: try_serialize_record(&scratch, RecordKind::Scratchpad)
                    .map_err(|_| PutError::Serialization)?
                    .to_vec(),
                publisher: None,
                expires: None,
            }
        };

        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::Majority,
            retry_strategy: Some(RetryStrategy::Balanced),
            use_put_record_to: None,
            verification: Some((
                VerificationKind::Network,
                GetRecordCfg {
                    get_quorum: Quorum::Majority,
                    retry_strategy: None,
                    target_record: None,
                    expected_holders: HashSet::new(),
                    is_register: false,
                },
            )),
        };

        self.network().put_record(record, &put_cfg).await?;

        Ok(Some(next_count))
    }
}
