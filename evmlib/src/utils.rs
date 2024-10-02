use crate::common::{Address, Hash};
use crate::{CustomNetwork, Network};
use rand::Rng;
use std::env;
use std::env::VarError;

const ENV_RPC_URL: &str = "RPC_URL";
const ENV_PAYMENT_TOKEN_ADDRESS: &str = "PAYMENT_TOKEN_ADDRESS";
const ENV_CHUNK_PAYMENTS_ADDRESS: &str = "CHUNK_PAYMENTS_ADDRESS";

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
    const EVM_VARS: [&str; 3] = [
        ENV_RPC_URL,
        ENV_PAYMENT_TOKEN_ADDRESS,
        ENV_CHUNK_PAYMENTS_ADDRESS,
    ];
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

/// Get the EVM `Network` using env variables at compile time.
pub fn evm_network_from_env_compile_time() -> Network {
    let rpc_url = option_env!("RPC_URL").expect("RPC_URL not set");
    let payment_token_address =
        option_env!("PAYMENT_TOKEN_ADDRESS").expect("PAYMENT_TOKEN_ADDRESS not set");
    let chunk_payments_address =
        option_env!("CHUNK_PAYMENTS_ADDRESS").expect("CHUNK_PAYMENTS_ADDRESS not set");

    Network::Custom(CustomNetwork::new(
        rpc_url,
        payment_token_address,
        chunk_payments_address,
    ))
}
