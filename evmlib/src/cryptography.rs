use crate::common::Hash;
use alloy::primitives::keccak256;

/// Hash data using Keccak256.
pub fn hash<T: AsRef<[u8]>>(data: T) -> Hash {
    keccak256(data.as_ref())
}
