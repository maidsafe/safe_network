// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::wallet::Error;
use crate::wallet::Result;
use crate::MainSecretKey;
use bls::SecretKey;
use hex::encode;
use rand::Rng;
use ring::aead::{BoundKey, Nonce, NonceSequence};
use ring::error::Unspecified;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::num::NonZeroU32;
use std::path::Path;

/// Number of iterations for pbkdf2.
const ITERATIONS: NonZeroU32 = match NonZeroU32::new(100_000) {
    Some(v) => v,
    None => panic!("`100_000` is not be zero"),
};

/// Filename for the encrypted secret key.
pub const ENCRYPTED_MAIN_SECRET_KEY_FILENAME: &str = "main_secret_key.encrypted";

/// Encrypted secret key for storing on disk and decrypting with password
#[derive(Serialize, Deserialize)]
pub(crate) struct EncryptedSecretKey {
    encrypted_secret_key: String,
    pub salt: String,
    pub nonce: String,
}

impl EncryptedSecretKey {
    /// Save an encrypted secret key to a file inside the wallet directory.
    /// The encrypted secret key will be saved as `main_secret_key.encrypted`.
    pub fn save_to_file(&self, wallet_dir: &Path) -> Result<()> {
        let serialized_data = serde_json::to_string(&self)
            .map_err(|e| Error::FailedToSerializeEncryptedKey(e.to_string()))?;

        let encrypted_secret_key_path = wallet_dir.join(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);

        std::fs::write(encrypted_secret_key_path, serialized_data)?;

        Ok(())
    }

    /// Read an encrypted secret key from file.
    /// The file should be named `main_secret_key.encrypted` and inside the provided wallet directory.
    pub fn from_file(wallet_dir: &Path) -> Result<EncryptedSecretKey> {
        let path = wallet_dir.join(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);

        if !path.is_file() {
            return Err(Error::EncryptedMainSecretKeyNotFound(path));
        }

        let mut file = std::fs::File::open(path).map_err(|_| {
            Error::FailedToDeserializeEncryptedKey(String::from("File open failed."))
        })?;

        let mut buffer = String::new();

        file.read_to_string(&mut buffer).map_err(|_| {
            Error::FailedToDeserializeEncryptedKey(String::from("File read failed."))
        })?;

        let encrypted_secret_key: EncryptedSecretKey =
            serde_json::from_str(&buffer).map_err(|_| {
                Error::FailedToDeserializeEncryptedKey(format!("Deserialization failed: {buffer}"))
            })?;

        Ok(encrypted_secret_key)
    }

    /// Returns whether a `main_secret_key.encrypted` file exists.
    pub fn file_exists(wallet_dir: &Path) -> bool {
        let path = wallet_dir.join(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);
        path.is_file()
    }

    /// Decrypt an encrypted secret key using the password.
    pub fn decrypt(&self, password: &str) -> Result<MainSecretKey> {
        let salt = hex::decode(&self.salt)
            .map_err(|_| Error::FailedToDecryptKey(String::from("Invalid salt encoding.")))?;

        let mut key = [0; 32];

        // Reconstruct the key from salt and password
        ring::pbkdf2::derive(
            ring::pbkdf2::PBKDF2_HMAC_SHA512,
            ITERATIONS,
            &salt,
            password.as_bytes(),
            &mut key,
        );

        // Create an unbound key from the previously reconstructed key
        let unbound_key = ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
            .map_err(|_| {
                Error::FailedToDecryptKey(String::from("Could not create unbound key."))
            })?;

        // Restore original nonce
        let nonce_vec = hex::decode(&self.nonce)
            .map_err(|_| Error::FailedToDecryptKey(String::from("Invalid nonce encoding.")))?;

        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec[0..12]);

        // Create an opening key using the unbound key and original nonce
        let mut opening_key = ring::aead::OpeningKey::new(unbound_key, NonceSeq(nonce));
        let aad = ring::aead::Aad::from(&[]);

        // Convert the hex encoded and encrypted secret key to bytes
        let mut encrypted_secret_key = hex::decode(&self.encrypted_secret_key).map_err(|_| {
            Error::FailedToDecryptKey(String::from("Invalid encrypted secret key encoding."))
        })?;

        // Decrypt the encrypted secret key bytes
        let decrypted_data = opening_key
            .open_in_place(aad, &mut encrypted_secret_key)
            .map_err(|_| Error::FailedToDecryptKey(String::from("Could not open encrypted key")))?;

        let mut secret_key_bytes = [0u8; 32];
        secret_key_bytes.copy_from_slice(&decrypted_data[0..32]);

        // Create secret key from decrypted bytes
        let secret_key = SecretKey::from_bytes(secret_key_bytes)?;

        Ok(MainSecretKey::new(secret_key))
    }
}

/// Nonce sequence for the aead sealing key.
struct NonceSeq([u8; 12]);

impl NonceSequence for NonceSeq {
    fn advance(&mut self) -> std::result::Result<Nonce, Unspecified> {
        Nonce::try_assume_unique_for_key(&self.0)
    }
}

/// Encrypts secret key using pbkdf2 with HMAC<Sha512>.
pub(crate) fn encrypt_secret_key(
    secret_key: &MainSecretKey,
    password: &str,
) -> Result<EncryptedSecretKey> {
    // Generate a random salt
    // Salt is used to ensure unique derived keys even for identical passwords
    let mut salt = [0u8; 8];
    rand::thread_rng().fill(&mut salt);

    // Generate a random nonce
    // Nonce is used to ensure unique encryption outputs even for identical inputs
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill(&mut nonce);

    let mut key = [0; 32];

    // Derive a key from the password using PBKDF2 with HMAC<Sha512>
    // PBKDF2 is used for key derivation to mitigate brute-force attacks by making key derivation computationally expensive
    // HMAC<Sha512> is used as the pseudorandom function for its security properties
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA512,
        ITERATIONS,
        &salt,
        password.as_bytes(),
        &mut key,
    );

    // Create an unbound key using CHACHA20_POLY1305 algorithm
    // CHACHA20_POLY1305 is a fast and secure AEAD (Authenticated Encryption with Associated Data) algorithm
    let unbound_key = ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
        .map_err(|_| Error::FailedToEncryptKey(String::from("Could not create unbound key.")))?;

    // Create a sealing key with the unbound key and nonce
    let mut sealing_key = ring::aead::SealingKey::new(unbound_key, NonceSeq(nonce));
    let aad = ring::aead::Aad::from(&[]);

    // Convert the secret key to bytes
    let secret_key_bytes = secret_key.to_bytes();
    let mut encrypted_secret_key = secret_key_bytes;

    // seal_in_place_append_tag encrypts the data and appends an authentication tag to ensure data integrity
    sealing_key
        .seal_in_place_append_tag(aad, &mut encrypted_secret_key)
        .map_err(|_| Error::FailedToEncryptKey(String::from("Could not seal sealing key.")))?;

    // Return the encrypted secret key along with salt and nonce encoded as hex strings
    Ok(EncryptedSecretKey {
        encrypted_secret_key: encode(encrypted_secret_key),
        salt: encode(salt),
        nonce: encode(nonce),
    })
}

#[cfg(test)]
mod tests {
    use crate::wallet::encryption::{
        encrypt_secret_key, EncryptedSecretKey, ENCRYPTED_MAIN_SECRET_KEY_FILENAME,
    };
    use crate::MainSecretKey;
    use bls::SecretKey;

    /// Helper function to create a random MainSecretKey for testing.
    fn generate_main_secret_key() -> MainSecretKey {
        let secret_key = SecretKey::random();
        MainSecretKey::new(secret_key)
    }

    #[test]
    fn test_encrypt_and_decrypt() {
        let password = "safenetwork";
        let main_secret_key = generate_main_secret_key();

        // Encrypt the secret key
        let encrypted_secret_key =
            encrypt_secret_key(&main_secret_key, password).expect("Failed to encrypt key");

        // Decrypt the secret key
        let decrypted_secret_key = encrypted_secret_key
            .decrypt(password)
            .expect("Failed to decrypt key");

        // Ensure the decrypted key matches the original key
        assert_eq!(main_secret_key.to_bytes(), decrypted_secret_key.to_bytes());
    }

    #[test]
    fn test_decrypt_with_wrong_password() {
        let password = "safenetwork";
        let wrong_password = "unsafenetwork";
        let main_secret_key = generate_main_secret_key();

        // Encrypt the secret key
        let encrypted_secret_key =
            encrypt_secret_key(&main_secret_key, password).expect("Failed to encrypt key");

        // Ensure the decryption succeeds with the correct password
        assert!(encrypted_secret_key.decrypt(password).is_ok());

        // Ensure the decryption fails with the wrong password
        assert!(encrypted_secret_key.decrypt(wrong_password).is_err());
    }

    #[test]
    fn test_save_to_file_and_read_from_file() {
        let password = "safenetwork";
        let main_secret_key = generate_main_secret_key();
        let encrypted_secret_key =
            encrypt_secret_key(&main_secret_key, password).expect("Failed to encrypt key");

        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let wallet_dir = temp_dir.path();

        // Save the encrypted secret key to the file
        encrypted_secret_key
            .save_to_file(wallet_dir)
            .expect("Failed to save encrypted key to file");

        // Check if the file exists
        let encrypted_secret_key_path = wallet_dir.join(ENCRYPTED_MAIN_SECRET_KEY_FILENAME);
        assert!(
            encrypted_secret_key_path.is_file(),
            "Encrypted key file does not exist"
        );

        // Read the file
        let read_encrypted_secret_key = EncryptedSecretKey::from_file(wallet_dir)
            .expect("Failed to read encrypted key from file.");

        // Ensure the read data matches the original encrypted secret key
        assert_eq!(
            read_encrypted_secret_key.encrypted_secret_key,
            encrypted_secret_key.encrypted_secret_key
        );
        assert_eq!(read_encrypted_secret_key.salt, encrypted_secret_key.salt);
        assert_eq!(read_encrypted_secret_key.nonce, encrypted_secret_key.nonce);
    }

    #[test]
    fn test_file_exists() {
        // todo
    }
}
