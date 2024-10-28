use crate::wallet::encryption::{decrypt_private_key, encrypt_private_key};
use crate::wallet::error::Error;
use crate::wallet::input::{get_password_input, get_wallet_selection_input};
use crate::wallet::DUMMY_NETWORK;
use autonomi::Wallet;
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;

const ENCRYPTED_PRIVATE_KEY_EXT: &str = ".encrypted";

/// Creates the wallets folder if it is missing and returns the folder path.
pub(crate) fn get_client_wallet_dir_path() -> Result<PathBuf, Error> {
    let mut home_dirs = dirs_next::data_dir().ok_or(Error::WalletsFolderNotFound)?;
    home_dirs.push("safe");
    home_dirs.push("autonomi");
    home_dirs.push("wallets");

    std::fs::create_dir_all(home_dirs.as_path()).map_err(|_| Error::FailedToCreateWalletsFolder)?;

    Ok(home_dirs)
}

/// Writes the private key (hex-encoded) to disk.
///
/// When a password is set, the private key file will be encrypted.
pub(crate) fn store_private_key(
    private_key: &str,
    encryption_password: Option<String>,
) -> Result<OsString, Error> {
    let wallet = Wallet::new_from_private_key(DUMMY_NETWORK, private_key)
        .map_err(|_| Error::InvalidPrivateKey)?;

    // Wallet address
    let wallet_address = wallet.address().to_string();
    let wallets_folder = get_client_wallet_dir_path()?;

    // If `encryption_password` is provided, the private key will be encrypted with the password.
    // Else it will be saved as plain text.
    if let Some(password) = encryption_password.as_ref() {
        let encrypted_key = encrypt_private_key(private_key, password)?;
        let file_name = format!("{wallet_address}{ENCRYPTED_PRIVATE_KEY_EXT}");
        let file_path = wallets_folder.join(file_name);
        std::fs::write(file_path.clone(), encrypted_key)
            .map_err(|err| Error::FailedToStorePrivateKey(err.to_string()))?;
        Ok(file_path.into_os_string())
    } else {
        let file_path = wallets_folder.join(wallet_address);
        std::fs::write(file_path.clone(), private_key)
            .map_err(|err| Error::FailedToStorePrivateKey(err.to_string()))?;
        Ok(file_path.into_os_string())
    }
}

/// Loads the private key (hex-encoded) from disk.
///
/// If the private key file is encrypted, the function will prompt for the decryption password in the CLI.
pub(crate) fn load_private_key(wallet_address: &str) -> Result<String, Error> {
    let wallets_folder = get_client_wallet_dir_path()?;

    let mut file_name = wallet_address.to_string();

    // Check if a file with the encrypted extension exists
    let encrypted_file_path =
        wallets_folder.join(format!("{wallet_address}{ENCRYPTED_PRIVATE_KEY_EXT}"));

    let is_encrypted = encrypted_file_path.exists();

    if is_encrypted {
        file_name.push_str(ENCRYPTED_PRIVATE_KEY_EXT);
    }

    let file_path = wallets_folder.join(file_name);

    let mut file = std::fs::File::open(&file_path).map_err(|_| Error::PrivateKeyFileNotFound)?;

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .map_err(|_| Error::InvalidPrivateKey)?;

    // If the file is encrypted, prompt for the password and decrypt the key.
    if is_encrypted {
        let password = get_password_input("Enter password to decrypt wallet:");

        decrypt_private_key(&buffer, &password)
    } else {
        Ok(buffer)
    }
}

/// Lists all wallet files together with an index and let the user select one.
/// If only one wallet exists, auto select it.
pub(crate) fn select_wallet() -> Result<String, Error> {
    let wallets_folder = get_client_wallet_dir_path()?;
    let wallet_files = get_wallet_files(&wallets_folder)?;

    match wallet_files.len() {
        0 => Err(Error::NoWalletsFound),
        1 => Ok(wallet_files[0].to_string_lossy().into_owned()),
        _ => {
            println!("Select a wallet:");

            for (index, wallet_file) in wallet_files.iter().enumerate() {
                println!("{}: {}", index + 1, wallet_file.display());
            }

            let selected_index = get_wallet_selection_input()
                .parse::<usize>()
                .map_err(|_| Error::InvalidSelection)?;

            if selected_index >= 1 && selected_index <= wallet_files.len() {
                Ok(wallet_files[selected_index - 1]
                    .to_string_lossy()
                    .into_owned())
            } else {
                Err(Error::InvalidSelection)
            }
        }
    }
}

fn get_wallet_files(wallets_folder: &PathBuf) -> Result<Vec<PathBuf>, Error> {
    let wallet_files = std::fs::read_dir(wallets_folder)
        .map_err(|_| Error::WalletsFolderNotFound)?
        .filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

    Ok(wallet_files)
}
