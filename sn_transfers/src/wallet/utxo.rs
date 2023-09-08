// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::{Ciphertext, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use sn_dbc::DerivationIndex;
use sn_protocol::storage::DbcAddress;

use super::error::{Error, Result};

/// Transfer sent to a recipient
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, custom_debug::Debug)]
pub struct Transfer {
    /// List of encrypted UTXOs from which a recipient can verify and get money
    /// Only the recipient can decrypt these UTXOs
    encrypted_utxos: Vec<Ciphertext>,
}

impl Transfer {
    /// Create a new transfer
    /// utxos: List of UTXOs to be used for payment
    /// recipient: main Public key (donation key) of the recipient,
    ///     not to be confused with the derived keys
    pub fn create(utxos: Vec<Utxo>, recipient: PublicKey) -> Result<Self> {
        let encrypted_utxos = utxos
            .into_iter()
            .map(|utxo| utxo.encrypt(recipient))
            .collect::<Result<Vec<Ciphertext>>>()?;
        Ok(Transfer { encrypted_utxos })
    }

    /// Get the UTXOs from the Payment
    /// This is used by the recipient of a payment to decrypt the utxos in a payment
    pub fn utxos(&self, sk: &SecretKey) -> Result<Vec<Utxo>> {
        let mut utxos = Vec::new();
        for cypher in &self.encrypted_utxos {
            let utxo = Utxo::decrypt(cypher, sk)?;
            utxos.push(utxo);
        }
        Ok(utxos)
    }
}

/// Unspent Transaction (Tx) Output
/// Information can be used by the Tx recipient of this output
/// to check that they recieved money and to spend it
///
/// This struct contains sensitive information that should be kept secret
/// so it should be encrypted to the recipient's public key (public address)
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, custom_debug::Debug)]
pub struct Utxo {
    /// derivation index of the UTXO
    /// with this derivation index the owner can derive
    /// the secret key from their main key needed to spend this UTXO
    pub derivation_index: DerivationIndex,
    /// spentbook entry of one of one of the inputs (parent spends)
    /// using data found at this address the owner can check that the output is valid money
    pub parent_spend: DbcAddress,
}

impl Utxo {
    /// Create a new Utxo
    pub fn new(derivation_index: DerivationIndex, parent_spend: DbcAddress) -> Self {
        Self {
            derivation_index,
            parent_spend,
        }
    }

    /// Serialize the Utxo to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(self).map_err(|_| Error::UtxoSerialisationFailed)
    }

    /// Deserialize the Utxo from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes).map_err(|_| Error::UtxoSerialisationFailed)
    }

    /// Encrypt the Utxo to a public key
    pub fn encrypt(&self, pk: PublicKey) -> Result<Ciphertext> {
        let bytes = self.to_bytes()?;
        Ok(pk.encrypt(bytes))
    }

    /// Decrypt the Utxo with a secret key
    pub fn decrypt(cypher: &Ciphertext, sk: &SecretKey) -> Result<Self> {
        let bytes = sk.decrypt(cypher).ok_or(Error::UtxoDecryptionFailed)?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use xor_name::XorName;

    use super::*;

    #[test]
    fn test_utxo_conversions() {
        let rng = &mut bls::rand::thread_rng();
        let utxo = Utxo::new([42; 32], DbcAddress::new(XorName::random(rng)));
        let sk = SecretKey::random();
        let pk = sk.public_key();

        let bytes = utxo.to_bytes().unwrap();
        let cipher = utxo.encrypt(pk).unwrap();

        let utxo2 = Utxo::from_bytes(&bytes).unwrap();
        let utxo3 = Utxo::decrypt(&cipher, &sk).unwrap();

        assert_eq!(utxo, utxo2);
        assert_eq!(utxo, utxo3);
    }

    #[test]
    fn test_utxo_transfer() {
        let rng = &mut bls::rand::thread_rng();
        let utxo = Utxo::new([42; 32], DbcAddress::new(XorName::random(rng)));
        let sk = SecretKey::random();
        let pk = sk.public_key();

        let payment = Transfer::create(vec![utxo.clone()], pk).unwrap();
        let utxos = payment.utxos(&sk).unwrap();

        assert_eq!(utxos, vec![utxo]);
    }
}
