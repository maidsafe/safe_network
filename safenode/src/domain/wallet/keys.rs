// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::error::{Error, Result};

use sn_dbc::{MainKey, PublicAddress};

use hex::{decode, encode};
use std::path::Path;
use tokio::fs;

/// Filename for storing the node's reward (BLS hex-encoded) main key.
const MAIN_KEY_FILENAME: &str = "main_key";
/// Filename for storing the node's reward (BLS hex-encoded) public address.
const PUBLIC_ADDRESS_FILENAME: &str = "public_address";

/// Parse a public address from a hex-encoded string.
pub fn parse_public_address<T: AsRef<[u8]>>(hex: T) -> Result<PublicAddress> {
    let public_key = bls_public_from_hex(hex)?;
    Ok(PublicAddress::new(public_key))
}

/// Writes the public address and main key (hex-encoded) to different locations at disk.
pub(super) async fn store_new_keypair(wallet_dir: &Path, main_key: &MainKey) -> Result<()> {
    let secret_key_path = wallet_dir.join(MAIN_KEY_FILENAME);
    let public_key_path = wallet_dir.join(PUBLIC_ADDRESS_FILENAME);
    fs::write(secret_key_path, encode(main_key.to_bytes())).await?;
    fs::write(
        public_key_path,
        encode(main_key.public_address().to_bytes()),
    )
    .await
    .map_err(|e| Error::FailedToHexEncodeKey(e.to_string()))?;
    Ok(())
}

/// Returns Some(sn_dbc::MainKey) or None if file doesn't exist. It assumes it's hex-encoded.
pub(super) async fn get_main_key(wallet_dir: &Path) -> Result<Option<MainKey>> {
    let path = wallet_dir.join(MAIN_KEY_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }

    let secret_hex_bytes = fs::read(&path).await?;
    let secret = bls_secret_from_hex(secret_hex_bytes)?;

    Ok(Some(MainKey::new(secret)))
}

/// Construct a BLS secret key from a hex-encoded string.
fn bls_secret_from_hex<T: AsRef<[u8]>>(hex: T) -> Result<bls::SecretKey> {
    let bytes = decode(hex).map_err(|_| Error::FailedToDecodeHexToKey)?;
    let bytes_fixed_len: [u8; bls::SK_SIZE] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| Error::FailedToParseBlsKey)?;
    let sk = bls::SecretKey::from_bytes(bytes_fixed_len)?;
    Ok(sk)
}

/// Construct a BLS public key from a hex-encoded string.
fn bls_public_from_hex<T: AsRef<[u8]>>(hex: T) -> Result<bls::PublicKey> {
    let bytes = decode(hex).map_err(|_| Error::FailedToDecodeHexToKey)?;
    let bytes_fixed_len: [u8; bls::PK_SIZE] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| Error::FailedToParseBlsKey)?;
    let pk = bls::PublicKey::from_bytes(bytes_fixed_len)?;
    Ok(pk)
}

#[cfg(test)]
mod test {
    use super::{get_main_key, store_new_keypair, MainKey};

    use assert_fs::TempDir;
    use eyre::Result;

    #[tokio::test]
    async fn reward_key_to_and_from_file() -> Result<()> {
        let main_key = MainKey::random();
        let dir = create_temp_dir();
        let root_dir = dir.path().to_path_buf();
        store_new_keypair(&root_dir, &main_key).await?;
        let secret_result = get_main_key(&root_dir)
            .await?
            .expect("There to be a key on disk.");
        assert_eq!(secret_result.public_address(), main_key.public_address());
        Ok(())
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new().expect("Should be able to create a temp dir.")
    }
}
