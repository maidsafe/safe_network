use crate::common::{Address, Hash};
use crate::{CustomNetwork, Network};
use dirs_next::data_dir;
use rand::Rng;
use std::env;

pub const EVM_TESTNET_CSV_FILENAME: &str = "evm_testnet_data.csv";

/// environment variable to connect to a custom EVM network
pub const RPC_URL: &str = "RPC_URL";
const RPC_URL_BUILD_TIME_VAL: Option<&str> = option_env!("RPC_URL");
pub const PAYMENT_TOKEN_ADDRESS: &str = "PAYMENT_TOKEN_ADDRESS";
const PAYMENT_TOKEN_ADDRESS_BUILD_TIME_VAL: Option<&str> = option_env!("PAYMENT_TOKEN_ADDRESS");
pub const DATA_PAYMENTS_ADDRESS: &str = "DATA_PAYMENTS_ADDRESS";
const DATA_PAYMENTS_ADDRESS_BUILD_TIME_VAL: Option<&str> = option_env!("DATA_PAYMENTS_ADDRESS");

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to get EVM network")]
    FailedToGetEvmNetwork,
}

/// Generate a random Address.
pub fn dummy_address() -> Address {
    Address::new(rand::rngs::OsRng.gen())
}

/// Generate a random Hash.
pub fn dummy_hash() -> Hash {
    Hash::new(rand::rngs::OsRng.gen())
}

/// Get the `Network` from environment variables
pub fn evm_network_from_env() -> Result<Network, Error> {
    let evm_vars = [
        env::var(RPC_URL)
            .ok()
            .or_else(|| RPC_URL_BUILD_TIME_VAL.map(|s| s.to_string())),
        env::var(PAYMENT_TOKEN_ADDRESS)
            .ok()
            .or_else(|| PAYMENT_TOKEN_ADDRESS_BUILD_TIME_VAL.map(|s| s.to_string())),
        env::var(PAYMENT_TOKEN_ADDRESS)
            .ok()
            .or_else(|| DATA_PAYMENTS_ADDRESS_BUILD_TIME_VAL.map(|s| s.to_string())),
    ]
    .into_iter()
    .map(|var| var.ok_or(Error::FailedToGetEvmNetwork))
    .collect::<Result<Vec<String>, Error>>();

    let use_local_evm = std::env::var("EVM_NETWORK")
        .map(|v| v == "local")
        .unwrap_or(false);
    let use_arbitrum_one = std::env::var("EVM_NETWORK")
        .map(|v| v == "arbitrum-one")
        .unwrap_or(false);

    if use_arbitrum_one {
        Ok(Network::ArbitrumOne)
    } else if use_local_evm {
        local_evm_network_from_csv()
    } else if let Ok(evm_vars) = evm_vars {
        Ok(Network::Custom(CustomNetwork::new(
            &evm_vars[0],
            &evm_vars[1],
            &evm_vars[2],
        )))
    } else {
        Ok(Network::ArbitrumOne)
    }
}

/// Get the `Network::Custom` from the local EVM testnet CSV file
pub fn local_evm_network_from_csv() -> Result<Network, Error> {
    // load the csv
    let csv_path = data_dir()
        .ok_or(Error::FailedToGetEvmNetwork)
        .inspect_err(|_| error!("Failed to get data dir when fetching evm testnet CSV file"))?
        .join("safe")
        .join(EVM_TESTNET_CSV_FILENAME);

    if !csv_path.exists() {
        error!("evm data csv path does not exist {:?}", csv_path);
        return Err(Error::FailedToGetEvmNetwork)
            .inspect_err(|_| error!("Missing evm testnet CSV file"))?;
    }

    let csv = std::fs::read_to_string(&csv_path)
        .map_err(|_| Error::FailedToGetEvmNetwork)
        .inspect_err(|_| error!("Failed to read evm testnet CSV file"))?;
    let parts: Vec<&str> = csv.split(',').collect();
    match parts.as_slice() {
        [rpc_url, payment_token_address, chunk_payments_address] => Ok(Network::Custom(
            CustomNetwork::new(rpc_url, payment_token_address, chunk_payments_address),
        )),
        _ => {
            error!("Invalid data in evm testnet CSV file");
            Err(Error::FailedToGetEvmNetwork)
        }
    }
}