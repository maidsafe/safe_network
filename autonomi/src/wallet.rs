use sn_evm::wallet::{get_random_private_key,create_a_evm_wallet,create_file_with_keys};


pub fn create_evm_wallet() {
    let wallet_private_key = get_random_private_key();
    let wallet_public_key = create_a_evm_wallet(&wallet_private_key);
    let file_path = create_file_with_keys(wallet_private_key, wallet_public_key);
    println!("A file is created with the path: {}", file_path);
}