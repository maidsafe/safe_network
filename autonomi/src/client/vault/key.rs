// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sha2::{Digest, Sha256};

/// Secret key to decrypt vault content
pub type VaultSecretKey = bls::SecretKey;

#[derive(Debug, thiserror::Error)]
pub enum VaultKeyError {
    #[error("Failed to sign message: {0}")]
    FailedToSignMessage(#[from] sn_evm::cryptography::SignError),
    #[error("Failed to generate vault secret key: {0}")]
    FailedToGenerateVaultSecretKey(String),
}

/// Message used to generate the vault secret key from the EVM secret key
const VAULT_SECRET_KEY_SEED: &[u8] = b"Massive Array of Internet Disks Secure Access For Everyone";

/// Derives the vault secret key from the EVM secret key hex string
/// The EVM secret key is used to sign a message and the signature is hashed to derive the vault secret key
/// Being able to derive the vault secret key from the EVM secret key allows users to only keep track of one key: the EVM secret key
pub fn derive_vault_key(evm_sk_hex: &str) -> Result<VaultSecretKey, VaultKeyError> {
    let signature = sn_evm::cryptography::sign_message(evm_sk_hex, VAULT_SECRET_KEY_SEED)
        .map_err(VaultKeyError::FailedToSignMessage)?;
    let hash = hash_to_32b(&signature);

    // NB TODO: not sure this is safe, we should ask Mav or find a better way to do this!
    let root_sk = bls::SecretKey::default();
    let unique_key = root_sk.derive_child(&hash);
    Ok(unique_key)
}

fn hash_to_32b(msg: &[u8]) -> [u8; 32] {
    let mut sha = Sha256::new();
    sha.update(msg);
    sha.finalize().into()
}
