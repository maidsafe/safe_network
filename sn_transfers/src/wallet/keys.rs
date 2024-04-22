// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::error::{Error, Result};
use crate::{MainPubkey, MainSecretKey};
use hex::{decode, encode};
use std::path::Path;

/// Filename for storing the node's reward (BLS hex-encoded) main secret key.
const MAIN_SECRET_KEY_FILENAME: &str = "main_secret_key";
/// Filename for storing the node's reward (BLS hex-encoded) public key.
const MAIN_PUBKEY_FILENAME: &str = "main_pubkey";

/// Writes the public address and main key (hex-encoded) to different locations at disk.
pub(crate) fn store_new_keypair(wallet_dir: &Path, main_key: &MainSecretKey) -> Result<()> {
    let secret_key_path = wallet_dir.join(MAIN_SECRET_KEY_FILENAME);
    let public_key_path = wallet_dir.join(MAIN_PUBKEY_FILENAME);
    std::fs::write(secret_key_path, encode(main_key.to_bytes()))?;
    std::fs::write(public_key_path, encode(main_key.main_pubkey().to_bytes()))
        .map_err(|e| Error::FailedToHexEncodeKey(e.to_string()))?;
    Ok(())
}

/// Returns sn_transfers::MainSecretKey or None if file doesn't exist. It assumes it's hex-encoded.
pub(super) fn get_main_key_from_disk(wallet_dir: &Path) -> Result<MainSecretKey> {
    let path = wallet_dir.join(MAIN_SECRET_KEY_FILENAME);
    if !path.is_file() {
        return Err(Error::MainSecretKeyNotFound(path));
    }

    let secret_hex_bytes = std::fs::read(&path)?;
    let secret = bls_secret_from_hex(secret_hex_bytes)?;

    Ok(MainSecretKey::new(secret))
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
        store_new_keypair(&root_dir, &main_key)?;
        let secret_result = get_main_key_from_disk(&root_dir)?.expect("There to be a key on disk.");
        assert_eq!(secret_result.main_pubkey(), main_key.main_pubkey());
        Ok(())
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Should be able to create a temp dir.")
    }
}
