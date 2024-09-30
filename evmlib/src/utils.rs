use crate::common::{Address, Hash};
use rand::Rng;

/// Generate a random Address.
pub fn dummy_address() -> Address {
    Address::new(rand::rngs::OsRng.gen())
}

/// generate a random Hash.
pub fn dummy_hash() -> Hash {
    Hash::new(rand::rngs::OsRng.gen())
}
