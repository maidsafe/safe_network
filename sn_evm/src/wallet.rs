use evmlib::{wallet::{get_random_private_key_for_wallet, Wallet}, Network};
use evmlib::utils::get_evm_network_from_env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn get_random_private_key() -> String {
    get_random_private_key_for_wallet()
}

pub fn create_a_evm_wallet(private_key: &String) -> String {
    let network = get_evm_network_from_env()
                        .expect("Failed to get EVM network from environment variables");
    let wallet = Wallet::new_from_private_key(network, &private_key)
                                                .expect("Could not init deployer wallet");
    hex::encode(wallet.address())
}

pub fn create_file_with_keys(private_key: String, public_key: String) -> String {
    let mut file = File::create(public_key.clone()).expect("could not create file");
    file.write_all(private_key.as_bytes()).expect("Not able to write into file");
    let file_path = Path::new(&public_key).canonicalize().expect("Not able to find the absolute path for the file");
    file_path.to_string_lossy().to_string()
}