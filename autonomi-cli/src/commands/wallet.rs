// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::Multiaddr;
use autonomi::wallet::*;
use color_eyre::{
    eyre::{bail, Context},
    Result,
};
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
                            println!("Proceeding without password...");
                            return None;
                        }
                    };
                    Some(pwd)
                }
            }
        }
        _ => None,
    }
}

fn filter_and_lowercase(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_alphabetic()) // Keep only alphabetic characters
        .flat_map(|c| c.to_lowercase()) // Convert each char to lowercase
        .collect() // Collect into a String
}

pub fn filter_data_input(input: Option<String>, check_string: &str) ->  Option<String> {
    let input_arg_data;
    match input {
        Some(input) => {
            input_arg_data = input;
        }
        None => {
            return None;
        }
    }

    if let Some((key, value)) = input_arg_data.split_once('=') {

        if (filter_and_lowercase(key) == check_string.to_string()) {
            Some(value.to_string())
        } else {
            None
        }
        // (Some(key.to_string()), Some(value.to_string()))
    } else {
    None
    }
}

fn find_matching_sentence(sentences: &Vec<String>, pattern: &str) -> Option<String> {
    sentences.iter().find(|sentence| sentence.contains(pattern)).map(|sentence| sentence.to_string())
}


pub fn process_parameters(
    encryption: Option<String>,
    password: Option<String>,
    private_key: Option<String>
) -> (Option<String>, Option<String>, Option<String>) {

    // let mut vars = std::collections::HashMap::new();
    let mut vec_strings = Vec::new();
    
    if let Some(data) = encryption {
        vec_strings.push(data);
    }

    if let Some(data) = password {
        vec_strings.push(data);
    }

    if let Some(data) = private_key {
        vec_strings.push(data);
    }

    let encryption = find_matching_sentence(&vec_strings, "encrypt");
    let password = find_matching_sentence(&vec_strings, "password");
    let private_key = find_matching_sentence(&vec_strings, "private_key");
    
    // seperate the values from its keys from the user
    
    let encryption = filter_data_input(encryption, "encrypt");
    let password = filter_data_input(password, "password");
    let private_key = filter_data_input(private_key, "privatekey");
    
    (encryption, password, private_key)
}


pub fn initiate_wallet_creation(encryption: Option<String>, password: Option<String>, private_key: Option<String>) -> Result<()>{
    //ensure parameters are proper and ordered. 
    let (encryption, password, private_key) = process_parameters(encryption, password, private_key);
    
    println!("encryption: {:?}", encryption);
    println!("password: {:?}", password);
    println!("private_key: {:?}", private_key);
    // return Ok(()); 
    
    let pass = process_password(encryption, password);
    println!("pass is {:?}", pass);
    match private_key {
        Some(priv_key) => {
            println!("priv_key is {}", priv_key);
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

pub fn balance(peers: Vec<Multiaddr>) -> Result<()> {
    Ok(())
}