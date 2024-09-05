use crate::common::{Address, Hash, U256};
use alloy::primitives::{b256, FixedBytes};

// Should be updated when the smart contract changes!
pub(crate) const CHUNK_PAYMENT_EVENT_SIGNATURE: FixedBytes<32> =
    b256!("a6df5ca64d2adbcdd26949b97238efc4e97dc7e5d23012ea53f92a24f005f958");

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Event signature is missing")]
    EventSignatureMissing,
    #[error("Event signature does not match")]
    EventSignatureDoesNotMatch,
}

pub(crate) struct ChunkPaymentEvent {
    reward_address: Address,
    amount: U256,
    quote_hash: Hash,
}

// impl TryFrom<Log> for ChunkPaymentEvent {
//     type Error = Error;
//
//     fn try_from(log: Log) -> Result<Self, Self::Error> {
//         let topic0 = log.topics().get(0).ok_or(Error::EventSignatureMissing)?;
//
//         if topic0 != &CHUNK_PAYMENT_EVENT_SIGNATURE {
//             return Err(Error::EventSignatureDoesNotMatch);
//         }
//
//         // Skip the first topic, and extract the rest
//         let reward_address = Address::from_slice(&log.topics[1][12..]);
//         let amount = U256::from_big_endian(&log.topics[2][12..]);
//         let quote_hash = Hash::from_slice(&log.topics[3]);
//
//         Ok(Self {
//             reward_address,
//             amount,
//             quote_hash,
//         })
//     }
// }
