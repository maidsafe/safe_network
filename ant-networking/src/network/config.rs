use libp2p::{
    kad::{Quorum, Record},
    PeerId,
};
use ant_protocol::{
    messages::{ChunkProof, Nonce},
    storage::RetryStrategy,
};
use std::collections::HashSet;
use ant_registers::SignedRegister;
use ant_protocol::storage::try_deserialize_record;
use tracing::error;

/// The various settings to apply to when fetching a record from network
#[derive(Clone)]
pub struct GetRecordCfg {
    /// The query will result in an error if we get records less than the provided Quorum
    pub get_quorum: Quorum,
    /// If enabled, the provided `RetryStrategy` is used to retry if a GET attempt fails.
    pub retry_strategy: Option<RetryStrategy>,
    /// Only return if we fetch the provided record.
    pub target_record: Option<Record>,
    /// Logs if the record was not fetched from the provided set of peers.
    pub expected_holders: HashSet<PeerId>,
    /// For register record, only root value shall be checked, not the entire content.
    pub is_register: bool,
}

impl GetRecordCfg {
    pub fn does_target_match(&self, record: &Record) -> bool {
        if let Some(ref target_record) = self.target_record {
            if self.is_register {
                let pretty_key = format!("{:?}", &target_record.key);

                let fetched_register = match try_deserialize_record::<SignedRegister>(record) {
                    Ok(fetched_register) => fetched_register,
                    Err(err) => {
                        error!("When try to deserialize register from fetched record {pretty_key:?}, have error {err:?}");
                        return false;
                    }
                };
                let target_register = match try_deserialize_record::<SignedRegister>(target_record) {
                    Ok(target_register) => target_register,
                    Err(err) => {
                        error!("When try to deserialize register from target record {pretty_key:?}, have error {err:?}");
                        return false;
                    }
                };

                target_register.base_register() == fetched_register.base_register()
                    && target_register.ops() == fetched_register.ops()
            } else {
                target_record == record
            }
        } else {
            // Not have target_record to check with
            true
        }
    }
}

impl std::fmt::Debug for GetRecordCfg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("GetRecordCfg");
        f.field("get_quorum", &self.get_quorum)
            .field("retry_strategy", &self.retry_strategy);

        match &self.target_record {
            Some(record) => {
                let pretty_key = format!("{:?}", &record.key);
                f.field("target_record", &pretty_key);
            }
            None => {
                f.field("target_record", &"None");
            }
        };

        f.field("expected_holders", &self.expected_holders).finish()
    }
}

/// The various settings related to writing a record to the network.
#[derive(Debug, Clone)]
pub struct PutRecordCfg {
    /// The quorum used by KAD PUT. KAD still sends out the request to all the peers set by the `replication_factor`, it
    /// just makes sure that we get at least `n` successful responses defined by the Quorum.
    /// Our nodes currently send `Ok()` response for every KAD PUT. Thus this field does not do anything atm.
    pub put_quorum: Quorum,
    /// If enabled, the provided `RetryStrategy` is used to retry if a PUT attempt fails.
    pub retry_strategy: Option<RetryStrategy>,
    /// Use the `kad::put_record_to` to PUT the record only to the specified peers. If this option is set to None, we
    /// will be using `kad::put_record` which would PUT the record to all the closest members of the record.
    pub use_put_record_to: Option<Vec<PeerId>>,
    /// Enables verification after writing. The VerificationKind is used to determine the method to use.
    pub verification: Option<(VerificationKind, GetRecordCfg)>,
}

/// The methods in which verification on a PUT can be carried out.
#[derive(Debug, Clone)]
pub enum VerificationKind {
    /// Uses the default KAD GET to perform verification.
    Network,
    /// Uses the default KAD GET to perform verification, but don't error out on split records
    Crdt,
    /// Uses the hash based verification for chunks.
    ChunkProof {
        expected_proof: ChunkProof,
        nonce: Nonce,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::kad::Record;

    #[test]
    fn test_get_record_cfg_target_match() {
        let record = Record {
            key: vec![1, 2, 3],
            value: vec![4, 5, 6],
            publisher: None,
            expires: None,
        };

        // Test with no target record
        let cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: None,
            target_record: None,
            expected_holders: HashSet::new(),
            is_register: false,
        };
        assert!(cfg.does_target_match(&record));

        // Test with matching target record
        let cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: None,
            target_record: Some(record.clone()),
            expected_holders: HashSet::new(),
            is_register: false,
        };
        assert!(cfg.does_target_match(&record));

        // Test with non-matching target record
        let different_record = Record {
            key: vec![1, 2, 3],
            value: vec![7, 8, 9],
            publisher: None,
            expires: None,
        };
        let cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: None,
            target_record: Some(different_record),
            expected_holders: HashSet::new(),
            is_register: false,
        };
        assert!(!cfg.does_target_match(&record));
    }
}
