use crate::common::{Address, Hash};
use crate::{CustomNetwork, Network};
use rand::Rng;
use std::env;
use std::env::VarError;

/// Generate a random Address.
pub fn dummy_address() -> Address {
    Address::new(rand::rngs::OsRng.gen())
}

/// Generate a random Hash.
pub fn dummy_hash() -> Hash {
    Hash::new(rand::rngs::OsRng.gen())
}

/// Get the `Network` from environment variables
pub fn evm_network_from_env() -> Result<Network, VarError> {
    const EVM_VARS: [&str; 3] = ["RPC_URL", "PAYMENT_TOKEN_ADDRESS", "CHUNK_PAYMENTS_ADDRESS"];
    let custom_vars_exist = EVM_VARS.iter().all(|var| env::var(var).is_ok());

    if custom_vars_exist {
        Ok(Network::Custom(CustomNetwork::new(
            &env::var(EVM_VARS[0])?,
            &env::var(EVM_VARS[1])?,
            &env::var(EVM_VARS[2])?,
        )))
    } else {
        Ok(Network::ArbitrumOne)
    }
}
