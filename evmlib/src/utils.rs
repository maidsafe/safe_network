use crate::common::{Address, Amount, Hash};
use rand::Rng;

/// Returns the amount of royalties expected for a certain transfer amount.
pub fn calculate_royalties_from_amount(amount: Amount) -> Amount {
    amount / Amount::from(10)
}

/// Generate a random Address.
pub fn dummy_address() -> Address {
    Address::new(rand::rngs::OsRng.gen())
}

/// generate a random Hash.
pub fn dummy_hash() -> Hash {
    Hash::new(rand::rngs::OsRng.gen())
}
