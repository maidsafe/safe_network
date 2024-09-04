use alloy::primitives::keccak256;

/// Hash data using Keccak256.
pub fn hash<T: AsRef<[u8]>>(data: T) -> [u8; 32] {
    keccak256(data.as_ref()).0
}
