// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::{Ciphertext, PublicKey, SecretKey};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use xor_name::XorName;

use crate::{DerivationIndex, Hash, Result, TransferError};

const CUSTOM_SPEND_REASON_SIZE: usize = 64;

/// The attached metadata or reason for which a Spend was spent
#[derive(Default, Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum SpendReason {
    #[default]
    None,
    /// Reference to network data
    NetworkData(XorName),
    /// Custom field for any application data
    Custom(#[serde(with = "serde_bytes")] [u8; CUSTOM_SPEND_REASON_SIZE]),

    /// Beta only feature to track rewards
    /// Discord username encrypted to the Foundation's pubkey with a random nonce
    BetaRewardTracking(DiscordNameCipher),
}

impl SpendReason {
    pub fn hash(&self) -> Hash {
        match self {
            Self::None => Hash::default(),
            Self::NetworkData(xor_name) => Hash::hash(xor_name),
            Self::Custom(bytes) => Hash::hash(bytes),
            Self::BetaRewardTracking(cypher) => Hash::hash(&cypher.cipher),
        }
    }

    pub fn create_reward_tracking_reason(input_str: &str) -> Result<Self> {
        let input_pk = crate::NETWORK_ROYALTIES_PK.public_key();
        Ok(Self::BetaRewardTracking(DiscordNameCipher::create(
            input_str, input_pk,
        )?))
    }
}

lazy_static! {
    pub static ref FOUNDATION_PK: PublicKey = crate::NETWORK_ROYALTIES_PK.public_key();
}
const MAX_CIPHER_SIZE: usize = std::u8::MAX as usize;
const DERIVATION_INDEX_SIZE: usize = 32;
const HASH_SIZE: usize = 32;
const LIMIT_SIZE: usize = HASH_SIZE + DERIVATION_INDEX_SIZE;

/// Discord username encrypted to the Foundation's pubkey with a random nonce
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct DiscordNameCipher {
    /// Length of the cipher, hard limited to MAX_U8
    len: u8,
    /// Encrypted Discord username
    #[serde(with = "serde_bytes")]
    cipher: [u8; MAX_CIPHER_SIZE],
}

/// Discord username hash and nonce
/// u256 hash + u256 nonce might be overkill (very big)
struct DiscordName {
    hash: Hash,
    nonce: DerivationIndex,
}

impl DiscordName {
    fn new(user_name: &str) -> Self {
        let rng = &mut rand::thread_rng();
        DiscordName {
            hash: Hash::hash(user_name.as_bytes()),
            nonce: DerivationIndex::random(rng),
        }
    }

    fn to_sized_bytes(&self) -> [u8; LIMIT_SIZE] {
        let mut bytes: [u8; LIMIT_SIZE] = [0; LIMIT_SIZE];
        bytes[0..HASH_SIZE].copy_from_slice(self.hash.slice());
        bytes[HASH_SIZE..LIMIT_SIZE].copy_from_slice(&self.nonce.0);
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash_bytes = [0; HASH_SIZE];
        hash_bytes.copy_from_slice(&bytes[0..HASH_SIZE]);
        let hash = Hash::from(hash_bytes.to_owned());
        let mut nonce_bytes = [0; DERIVATION_INDEX_SIZE];
        nonce_bytes.copy_from_slice(&bytes[HASH_SIZE..LIMIT_SIZE]);
        let nonce = DerivationIndex(nonce_bytes.to_owned());
        Self { hash, nonce }
    }
}

impl DiscordNameCipher {
    /// Create a new DiscordNameCipher from a Discord username
    /// it is encrypted to the given pubkey
    pub fn create(user_name: &str, foundation_pk: PublicKey) -> Result<Self> {
        let discord_name = DiscordName::new(user_name);
        let cipher = foundation_pk.encrypt(discord_name.to_sized_bytes());
        let bytes = cipher.to_bytes();
        if bytes.len() > MAX_CIPHER_SIZE {
            return Err(TransferError::DiscordNameCipherTooBig);
        }
        let mut sized = [0; MAX_CIPHER_SIZE];
        sized[0..bytes.len()].copy_from_slice(&bytes);
        Ok(Self {
            len: bytes.len() as u8,
            cipher: sized,
        })
    }

    /// Recover a Discord username hash using the secret key it was encrypted to
    pub fn decrypt_to_username_hash(&self, sk: &SecretKey) -> Result<Hash> {
        let cipher = Ciphertext::from_bytes(&self.cipher[0..self.len as usize])?;
        let decrypted = sk
            .decrypt(&cipher)
            .ok_or(TransferError::UserNameDecryptFailed)?;
        let discord_name = DiscordName::from_bytes(&decrypted);
        Ok(discord_name.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_name_cyphering() {
        let foundation_sk = SecretKey::random();
        let foundation_pk = foundation_sk.public_key();

        let user_name = "JohnDoe#1234";
        let user_name_hash = Hash::hash(user_name.as_bytes());
        let cypher =
            DiscordNameCipher::create(user_name, foundation_pk).expect("cypher creation failed");
        let recovered_hash = cypher
            .decrypt_to_username_hash(&foundation_sk)
            .expect("decryption failed");
        assert_eq!(user_name_hash, recovered_hash);

        let user_name2 = "JackMa#5678";
        let user_name_hash2 = Hash::hash(user_name2.as_bytes());
        let cypher =
            DiscordNameCipher::create(user_name2, foundation_pk).expect("cypher creation failed");
        let recovered_hash = cypher
            .decrypt_to_username_hash(&foundation_sk)
            .expect("decryption failed");
        assert_eq!(user_name_hash2, recovered_hash);

        assert_ne!(user_name_hash, user_name_hash2);
    }
}
