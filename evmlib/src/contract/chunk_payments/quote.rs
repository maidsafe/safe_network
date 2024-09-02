use alloy::primitives::{Address, FixedBytes, U256};

#[derive(Clone)]
pub struct Signature {
    pub r: FixedBytes<32>,
    pub s: FixedBytes<32>,
    /// Recovery id
    pub v: u8,
}

#[derive(Clone)]
pub struct Quote {
    /// Keccak256 hash of the chunk address
    pub chunk_address_hash: FixedBytes<32>,
    /// Price for the chunk
    pub cost: U256,
    /// Expiration timestamp as seconds since UNIX epoch
    pub expiration_timestamp: U256,
    /// Wallet address receiving payment
    pub payment_address: Address,
}

#[derive(Clone)]
pub struct SignedQuote {
    pub quote: Quote,
    /// Signature by the Node of this quote
    pub signature: Signature,
}
