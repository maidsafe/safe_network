use std::io::Write;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

    // wallet_encryption_storage(&file_path, encrypted_private_key)
use sn_evm::wallet::{get_client_wallet_dir_path,prompt_the_user_for_password,create_a_evm_wallet, create_file_with_keys, get_gas_token_details, get_random_private_key, wallet_encryption_status, wallet_encryption_storage, ENCRYPTED_MAIN_SECRET_KEY_FILENAME};

use sn_evm::encryption::{decrypt_secret_key,encrypt_secret_key};

pub fn import_evm_wallet(wallet_private_key: String) -> String {
    let wallet_public_key = create_a_evm_wallet(&wallet_private_key);
    let file_path = create_file_with_keys(wallet_private_key, wallet_public_key);
    file_path
}

pub fn get_wallet_information(private_key: String) {
    get_gas_token_details(&private_key);
}

pub fn create_evm_wallet() -> String {
    let wallet_private_key = get_random_private_key();
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
       //make the wallet a directory.

    if Path::new(&file_path).is_file() {
        std::fs::remove_file(&file_path).expect("not able to remove the file");
        std::fs::create_dir(&file_path).expect("not able to create the directory"); 
    }

    wallet_encryption_storage(&file_path, &encrypted_private_key)
}



pub fn get_private_key_from_wallet(key: u32, files: HashMap<u32, (String,String)>) -> Option<String> {

    match files.get(&key) {
        Some(value) => {
            let mut wallet_directory = get_wallet_directory();
            wallet_directory.push(value.1.clone());
            if value.0 == "unprotected" {
                let file_contents= std::fs::read(&wallet_directory);
                if let Ok(file_data) = file_contents {
                    let private_key = String::from_utf8(file_data).expect("not able to convert");
                    return Some(private_key);
                }
            }

            if value.0 =="passw-protected" {
                let _ = wallet_directory.push(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);
                println!("encrypted wallet path: {:?}", wallet_directory);
                let encrypted_bytes = std::fs::read(wallet_directory);
                if let Ok(file_data) = encrypted_bytes {
                    let encrypted_private_key = String::from_utf8(file_data).expect("not able to convert");
                    let password = prompt_the_user_for_password()?;
                    let private_key = decrypt_secret_key(&encrypted_private_key, &password);
                    return Some(private_key);
                }
            }
        },
        None => {
            println!("Provided Key doesn't exist try again");
            return None;
        },
    }
    return None;

}

pub fn get_numbered_files(dir: &str) -> std::io::Result<HashMap<u32, (String,String)>> {
    let mut file_map:HashMap<u32, (String,String)> = std::collections::HashMap::new(); // Create a new HashMap to store the files
    let entries = std::fs::read_dir(dir)?; // Get an iterator over directory entries

    // Iterate over the entries and insert them into the HashMap
    for (index, entry) in entries.enumerate() {
        let entry = entry?; // Unwrap the entry from Result<DirEntry, Error>
        let mut path = entry.path(); // Get the path of the entry

            if let Some(name) = path.file_name() {
                let file_name = name.to_string_lossy().into_owned(); // Convert to String
                let mut wallet_details =None;
                if path.is_file() {
                     wallet_details = Some((String::from("unprotected"), file_name));
                } else if path.is_dir() {
                    path.push(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);
                    if path.is_file() {
                        wallet_details =  Some((String::from("passw-protected"), file_name));
                    }
                }
                if let Some(wallet_value) = wallet_details {
                    file_map.insert((index + 1) as u32, wallet_value);
                }
                 // Insert into HashMap with number as key
        }
    }
    Ok(file_map)
}

// Function to prompt the user for a key
pub fn prompt_for_key() -> u32 {
    print!("Enter a key to retrieve the file: ");
    std::io::stdout().flush().expect("Failed to flush stdout");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).expect("Failed to read input");

    input.trim().parse().expect("Invalid input. Please enter a number.")
}

pub fn get_wallet_directory() -> PathBuf {
    get_client_wallet_dir_path().expect("error")
}