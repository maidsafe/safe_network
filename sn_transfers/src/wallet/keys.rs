// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::error::{Error, Result};
use crate::wallet::encryption::{
    encrypt_secret_key, EncryptedSecretKey, ENCRYPTED_MAIN_SECRET_KEY_FILENAME,
};
use crate::{MainPubkey, MainSecretKey};
use hex::{decode, encode};
use std::path::Path;

/// Filename for storing the node's reward (BLS hex-encoded) main secret key.
const MAIN_SECRET_KEY_FILENAME: &str = "main_secret_key";
/// Filename for storing the node's reward (BLS hex-encoded) public key.
const MAIN_PUBKEY_FILENAME: &str = "main_pubkey";

/// Writes the public address and main key (hex-encoded) to different locations at disk.
pub(crate) fn store_new_keypair(
    wallet_dir: &Path,
    main_key: &MainSecretKey,
    password: Option<String>,
) -> Result<()> {
    store_new_pubkey(wallet_dir, &main_key.main_pubkey())?;
    store_main_secret_key(wallet_dir, main_key, password)?;

    Ok(())
}

/// Returns sn_transfers::MainSecretKey or None if file doesn't exist. It assumes it's hex-encoded.
pub(super) fn get_main_key_from_disk(
    wallet_dir: &Path,
    password: Option<String>,
) -> Result<MainSecretKey> {
    // If a valid `main_secret_key.encrypted` file is found, use it
    if EncryptedSecretKey::file_exists(wallet_dir) {
        let encrypted_secret_key = EncryptedSecretKey::from_file(wallet_dir)?;
        let password = password.ok_or(Error::EncryptedMainSecretKeyRequiresPassword)?;

        encrypted_secret_key.decrypt(&password)
    } else {
        // Else try a `main_secret_key` file
        let path = wallet_dir.join(MAIN_SECRET_KEY_FILENAME);

        if !path.is_file() {
            return Err(Error::MainSecretKeyNotFound(path));
        }

        let secret_hex_bytes = std::fs::read(&path)?;
        let secret = bls_secret_from_hex(secret_hex_bytes)?;

        Ok(MainSecretKey::new(secret))
    }
}

/// Writes the main secret key (hex-encoded) to disk.
///
/// When a password is set, the secret key file will be encrypted.
pub(crate) fn store_main_secret_key(
    wallet_dir: &Path,
    main_secret_key: &MainSecretKey,
    password: Option<String>,
) -> Result<()> {
    // If encryption_password is provided, the secret key will be encrypted with the password
    if let Some(password) = password.as_ref() {
        let encrypted_key = encrypt_secret_key(main_secret_key, password)?;
        // Save the encrypted secret key in `main_secret_key.encrypted` file
        encrypted_key.save_to_file(wallet_dir)?;
    } else {
        // Save secret key as plain hex text in `main_secret_key` file
        let secret_key_path = wallet_dir.join(MAIN_SECRET_KEY_FILENAME);
        std::fs::write(secret_key_path, encode(main_secret_key.to_bytes()))?;
    }

    Ok(())
}

/// Writes the public address (hex-encoded) to disk.
pub(crate) fn store_new_pubkey(wallet_dir: &Path, main_pubkey: &MainPubkey) -> Result<()> {
    let public_key_path = wallet_dir.join(MAIN_PUBKEY_FILENAME);
    std::fs::write(public_key_path, encode(main_pubkey.to_bytes()))
        .map_err(|e| Error::FailedToHexEncodeKey(e.to_string()))?;
    Ok(())
}

/// Returns Some(sn_transfers::MainPubkey) or None if file doesn't exist. It assumes it's hex-encoded.
pub(super) fn get_main_pubkey(wallet_dir: &Path) -> Result<Option<MainPubkey>> {
    let path = wallet_dir.join(MAIN_PUBKEY_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }

    let pk_hex_bytes = std::fs::read(&path)?;
    let main_pk = MainPubkey::from_hex(pk_hex_bytes)?;

    Ok(Some(main_pk))
}

/// Delete the file containing the secret key `main_secret_key`.
/// WARNING: Only call this if you know what you're doing!
pub(crate) fn delete_unencrypted_main_secret_key(wallet_dir: &Path) -> Result<()> {
    let path = wallet_dir.join(MAIN_SECRET_KEY_FILENAME);
    std::fs::remove_file(path)?;
    Ok(())
}

/// Delete the file containing the secret key `main_secret_key.encrypted`.
/// WARNING: Only call this if you know what you're doing!
pub(crate) fn delete_encrypted_main_secret_key(wallet_dir: &Path) -> Result<()> {
    let path = wallet_dir.join(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);
    std::fs::remove_file(path)?;
    Ok(())
}

/// Construct a BLS secret key from a hex-encoded string.
pub fn bls_secret_from_hex<T: AsRef<[u8]>>(hex: T) -> Result<bls::SecretKey> {
    let bytes = decode(hex).map_err(|_| Error::FailedToDecodeHexToKey)?;
    let bytes_fixed_len: [u8; bls::SK_SIZE] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| Error::FailedToParseBlsKey)?;
    let sk = bls::SecretKey::from_bytes(bytes_fixed_len)?;
    Ok(sk)
}

#[cfg(test)]
mod test {
    use super::{get_main_key_from_disk, store_new_keypair, MainSecretKey};
    use assert_fs::TempDir;
    use eyre::Result;

    #[test]
    fn reward_key_to_and_from_file() -> Result<()> {
        let main_key = MainSecretKey::random();
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();
        store_new_keypair(&root_dir, &main_key, None)?;
        let secret_result = get_main_key_from_disk(&root_dir, None)?;
        assert_eq!(secret_result.main_pubkey(), main_key.main_pubkey());
        Ok(())
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Should be able to create a temp dir.")
    }
}
