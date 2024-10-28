use autonomi::EvmNetwork;

pub(crate) mod encryption;
pub(crate) mod error;
pub(crate) mod fs;
pub(crate) mod input;

pub const DUMMY_NETWORK: EvmNetwork = EvmNetwork::ArbitrumSepolia;
