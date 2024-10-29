use crate::keys::{get_secret_key_from_env, load_evm_wallet_from_env};
use crate::wallet::fs::{select_wallet, select_wallet_private_key};
use autonomi::{EvmNetwork, Wallet};

pub(crate) mod encryption;
pub(crate) mod error;
pub(crate) mod fs;
pub(crate) mod input;

pub const DUMMY_NETWORK: EvmNetwork = EvmNetwork::ArbitrumSepolia;

/// Load wallet from ENV or disk
pub(crate) fn load_wallet() -> color_eyre::Result<Wallet> {
    // First try wallet from ENV
    if let Ok(wallet) = load_evm_wallet_from_env() {
        return Ok(wallet);
    }

    let wallet = select_wallet()?;

    Ok(wallet)
}

/// Load wallet private key from ENV or disk
pub(crate) fn load_wallet_private_key() -> color_eyre::Result<String> {
    // First try wallet private key from ENV
    if let Ok(private_key) = get_secret_key_from_env() {
        return Ok(private_key);
    }

    let private_key = select_wallet_private_key()?;

    Ok(private_key)
}
