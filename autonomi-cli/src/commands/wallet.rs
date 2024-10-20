// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.


use autonomi::wallet::*;
use color_eyre::Result;
use rpassword::read_password;

pub fn process_password(encryption: Option<String>, password: Option<String>) -> Option<String> {
    match encryption {
        Some(value) => {
            if !(value == "Y" || value == "y" || value == "Yes" || value == "YES" || value == "yes") {
                println!("value: {}", value);
                return None;
            }
            match password {
                Some(passw) => {
                    return Some(passw);
                } 
                None => {
                    //prompt for the password
                    println!("Please enter the Password");
                    let input_password = read_password();
                    let pwd = match input_password {
                        Ok(pwd) => pwd,
                        Err(e) => {
                            eprintln!("Failed to read password: {}",e);
                            // return Ok(())
                            println!("Try again...");
                            panic!("issue with password");
                        }
                    };
                    Some(pwd)
                }
            }
        }
        _ => None,
    }
}



pub fn initiate_wallet_creation(encryption: Option<String>, password: Option<String>, private_key: Option<String>) -> Result<()>{
    let pass = process_password(encryption, password);

    match private_key {
        Some(priv_key) => {
            import_new_wallet(priv_key, pass)
        },
        None => create_new_wallet(pass),
    }
}

pub fn import_new_wallet(private_key: String, encryption: Option<String>) -> Result<()> {
    let mut file_path = import_evm_wallet(private_key);

    if let Some(passw) = encryption {
        file_path = encrypt_evm_wallet(file_path, passw);
    }

    println!("The wallet is imported here: {}", file_path);

    Ok(())
}
pub fn create_new_wallet(encryption: Option<String>) -> Result<()> {
    let mut file_path = create_evm_wallet();

    if let Some(passw) = encryption {
        file_path = encrypt_evm_wallet(file_path, passw);
    }

    println!("The wallet is created here: {}", file_path);
    Ok(())
}

pub fn balance() -> Result<()> {
    // list_available_public_wallets
    // Call the function to get numbered file names as a HashMap
    let get_client_data_dir_path = get_wallet_directory();

    let files = get_numbered_files(get_client_data_dir_path.to_str().expect("error"))?;
    let mut sorted_files: Vec<(&u32, &(String,String))> = files.iter().collect();
    sorted_files.sort_by_key(|&(key, _)| key);
    // Print the HashMap
    for (key, value) in sorted_files {
        println!("{}: - {} - {}", key, value.0, value.1);
    }

    let key = prompt_for_key();

    if let Some(private_key) = get_private_key_from_wallet(key, files){
        get_wallet_information(private_key);
    }
    Ok(())
}