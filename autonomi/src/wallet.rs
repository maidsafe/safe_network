use std::path::Path;

    // wallet_encryption_storage(&file_path, encrypted_private_key)
use sn_evm::wallet::{self, create_a_evm_wallet, create_file_with_keys, get_random_private_key, wallet_encryption_status, wallet_encryption_storage};

use sn_evm::encryption::{decrypt_secret_key,encrypt_secret_key};

pub fn import_evm_wallet(wallet_private_key: String) -> String {
    let wallet_public_key = create_a_evm_wallet(&wallet_private_key);
    let file_path = create_file_with_keys(wallet_private_key, wallet_public_key);
    file_path
}


pub fn create_evm_wallet() -> String {
    let wallet_private_key = get_random_private_key();
    println!("private key length is {}", wallet_private_key.len());
    let wallet_public_key = create_a_evm_wallet(&wallet_private_key);
    let file_path = create_file_with_keys(wallet_private_key, wallet_public_key);
    // println!("A file is created with the path: {}", file_path);
    file_path
}

pub fn encrypt_evm_wallet(file_path: String, password: String) -> String {
    if wallet_encryption_status(Path::new(&file_path)) {
        return String::from("Not exists"); //replace with error condition later. 
    }

    let private_key = std::fs::read_to_string(&file_path).expect("not able to read the contents");

    let encrypted_private_key = encrypt_secret_key(&private_key, &password);
    println!("private key is {}", private_key);
    println!("encrypted Private key is {}", encrypted_private_key);

    let decrypted_private_key = decrypt_secret_key(&encrypted_private_key, &password);
    println!("decrypted private key is {} ", decrypted_private_key);
    println!("Generated Private keys are equal: {}", private_key == decrypted_private_key);
    println!("Private key length is {} and decrypted is {}", private_key.len(), decrypted_private_key.len());
        //make the wallet a directory.

    if Path::new(&file_path).is_file() {
        std::fs::remove_file(&file_path).expect("not able to remove the file");
        std::fs::create_dir(&file_path).expect("not able to create the directory"); 
    }

    wallet_encryption_storage(&file_path, &encrypted_private_key)
}