use crate::common::{Address, Hash, U256};
use alloy::primitives::{b256, FixedBytes};
use alloy::rpc::types::Log;

// Should be updated when the smart contract changes!
pub(crate) const CHUNK_PAYMENT_EVENT_SIGNATURE: FixedBytes<32> =
    b256!("a6df5ca64d2adbcdd26949b97238efc4e97dc7e5d23012ea53f92a24f005f958"); // DevSkim: ignore DS173237

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Topics amount is unexpected. Was expecting 4")]
    TopicsAmountUnexpected,
    #[error("Event signature is missing")]
    EventSignatureMissing,
    #[error("Event signature does not match")]
    EventSignatureDoesNotMatch,
}

/// Struct for the ChunkPaymentEvent emitted by the ChunkPayments smart contract.
#[derive(Debug)]
pub(crate) struct ChunkPaymentEvent {
    pub reward_address: Address,
    pub amount: U256,
    pub quote_hash: Hash,
}

impl TryFrom<Log> for ChunkPaymentEvent {
    type Error = Error;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        // Verify the amount of topics
        if log.topics().len() != 4 {
            return Err(Error::TopicsAmountUnexpected);
        }

        let topic0 = log.topics().first().ok_or(Error::EventSignatureMissing)?;

        // Verify the event signature
        if topic0 != &CHUNK_PAYMENT_EVENT_SIGNATURE {
            return Err(Error::EventSignatureDoesNotMatch);
        }

        // Extract the data
        let reward_address = Address::from_slice(&log.topics()[1][12..]);
        let amount = U256::from_be_slice(&log.topics()[2][12..]);
        let quote_hash = Hash::from_slice(log.topics()[3].as_slice());

        Ok(Self {
            reward_address,
            amount,
            quote_hash,
        })
    }
}
