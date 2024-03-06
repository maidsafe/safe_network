// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{CashNote, Ciphertext, DerivationIndex, MainPubkey, MainSecretKey, SpendAddress};

use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::error::{Result, TransferError};

/// Transfer sent to a recipient
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum Transfer {
    /// List of encrypted CashNoteRedemptions from which a recipient can verify and get money
    /// Only the recipient can decrypt these CashNoteRedemptions
    Encrypted(Vec<Ciphertext>),
    /// The network requires a payment as network royalties for storage which nodes can validate
    /// and verify, these CashNoteRedemptions need to be sent to storage nodes as payment proof as well.
    NetworkRoyalties(Vec<CashNoteRedemption>),
}

impl std::fmt::Debug for Transfer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkRoyalties(cn_redemptions) => {
                write!(f, "Transfer::NetworkRoyalties: {cn_redemptions:?}")
            }
            Self::Encrypted(transfers) => {
                // Iterate over the transfers and log the hash of each encrypted transfer
                let hashed: Vec<_> = transfers
                    .iter()
                    .map(|transfer| {
                        // Calculate the hash of the transfer
                        let mut hasher = DefaultHasher::new();
                        transfer.hash(&mut hasher);
                        hasher.finish()
                    })
                    .collect();
                // Write the encrypted transfers to the formatter
                write!(f, "Transfer::Encrypted: {hashed:?}")
            }
        }
    }
}

impl Transfer {
    /// This function is used to create a Transfer from a CashNote, can be done offline, and sent to the recipient.
    /// Creates a Transfer from the given cash_note
    /// This Transfer can be sent safely to the recipients as all data in it is encrypted
    /// The recipients can then decrypt the data and use it to verify and reconstruct the CashNote
    pub fn transfer_from_cash_note(cash_note: &CashNote) -> Result<Self> {
        let recipient = cash_note.main_pubkey;
        let u = CashNoteRedemption::from_cash_note(cash_note)?;
        let t = Self::create(vec![u], recipient)
            .map_err(|_| TransferError::CashNoteRedemptionEncryptionFailed)?;
        Ok(t)
    }

    /// This function is used to create a Network Royalties Transfer from a CashNote
    /// can be done offline, and sent to the recipient.
    /// Note that this type of transfer is not encrypted
    pub(crate) fn royalties_transfer_from_cash_note(cash_note: &CashNote) -> Result<Self> {
        let cnr = CashNoteRedemption::from_cash_note(cash_note)?;
        Ok(Self::NetworkRoyalties(vec![cnr]))
    }

    /// Create a new transfer
    /// cashnote_redemptions: List of CashNoteRedemptions to be used for payment
    /// recipient: main Public key (donation key) of the recipient,
    ///     not to be confused with the derived keys
    pub fn create(
        cashnote_redemptions: Vec<CashNoteRedemption>,
        recipient: MainPubkey,
    ) -> Result<Self> {
        let encrypted_cashnote_redemptions = cashnote_redemptions
            .into_iter()
            .map(|cashnote_redemption| cashnote_redemption.encrypt(recipient))
            .collect::<Result<Vec<Ciphertext>>>()?;
        Ok(Self::Encrypted(encrypted_cashnote_redemptions))
    }

    /// Get the CashNoteRedemptions from the Payment
    /// This is used by the recipient of a payment to decrypt the cashnote_redemptions in a payment
    pub fn cashnote_redemptions(&self, sk: &MainSecretKey) -> Result<Vec<CashNoteRedemption>> {
        match self {
            Self::Encrypted(cyphers) => {
                let cashnote_redemptions: Result<Vec<_>> = cyphers
                    .par_iter() // Use Rayon's par_iter for parallel processing
                    .map(|cypher| CashNoteRedemption::decrypt(cypher, sk)) // Decrypt each CashNoteRedemption
                    .collect(); // Collect results into a vector
                let cashnote_redemptions = cashnote_redemptions?; // Propagate error if any
                Ok(cashnote_redemptions)
            }
            Self::NetworkRoyalties(cnr) => Ok(cnr.clone()),
        }
    }

    /// Deserializes a `Transfer` represented as a hex string to a `Transfer`.
    pub fn from_hex(hex: &str) -> Result<Self> {
        let mut bytes =
            hex::decode(hex).map_err(|_| TransferError::TransferDeserializationFailed)?;
        bytes.reverse();
        let transfer: Self = rmp_serde::from_slice(&bytes)
            .map_err(|_| TransferError::TransferDeserializationFailed)?;
        Ok(transfer)
    }

    /// Serialize this `Transfer` instance to a readable hex string that a human can copy paste
    pub fn to_hex(&self) -> Result<String> {
        let mut serialized =
            rmp_serde::to_vec(&self).map_err(|_| TransferError::TransferSerializationFailed)?;
        serialized.reverse();
        Ok(hex::encode(serialized))
    }
}

/// Unspent Transaction (Tx) Output
/// Information can be used by the Tx recipient of this output
/// to check that they received money and to spend it
///
/// This struct contains sensitive information that should be kept secret
/// so it should be encrypted to the recipient's public key (public address)
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Hash)]
pub struct CashNoteRedemption {
    /// derivation index of the CashNoteRedemption
    /// with this derivation index the owner can derive
    /// the secret key from their main key needed to spend this CashNoteRedemption
    pub derivation_index: DerivationIndex,
    /// spentbook entry of one of one of the inputs (parent spends)
    /// using data found at this address the owner can check that the output is valid money
    pub parent_spend: SpendAddress,
}

impl CashNoteRedemption {
    /// Create a new CashNoteRedemption
    pub fn new(derivation_index: DerivationIndex, parent_spend: SpendAddress) -> Self {
        Self {
            derivation_index,
            parent_spend,
        }
    }

    pub fn from_cash_note(cash_note: &CashNote) -> Result<Self> {
        let derivation_index = cash_note.derivation_index();
        let parent_spend = match cash_note.signed_spends.iter().next() {
            Some(s) => SpendAddress::from_unique_pubkey(s.unique_pubkey()),
            None => {
                return Err(TransferError::CashNoteHasNoParentSpends);
            }
        };
        Ok(Self::new(derivation_index, parent_spend))
    }

    /// Serialize the CashNoteRedemption to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(self).map_err(|_| TransferError::CashNoteRedemptionSerialisationFailed)
    }

    /// Deserialize the CashNoteRedemption from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes)
            .map_err(|_| TransferError::CashNoteRedemptionSerialisationFailed)
    }

    /// Encrypt the CashNoteRedemption to a public key
    pub fn encrypt(&self, pk: MainPubkey) -> Result<Ciphertext> {
        let bytes = self.to_bytes()?;
        Ok(pk.0.encrypt(bytes))
    }

    /// Decrypt the CashNoteRedemption with a secret key
    pub fn decrypt(cypher: &Ciphertext, sk: &MainSecretKey) -> Result<Self> {
        let bytes = sk
            .secret_key()
            .decrypt(cypher)
            .ok_or(TransferError::CashNoteRedemptionDecryptionFailed)?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use xor_name::XorName;

    use super::*;

    #[test]
    fn test_cashnote_redemption_conversions() {
        let rng = &mut bls::rand::thread_rng();
        let cashnote_redemption = CashNoteRedemption::new(
            DerivationIndex([42; 32]),
            SpendAddress::new(XorName::random(rng)),
        );
        let sk = MainSecretKey::random();
        let pk = sk.main_pubkey();

        let bytes = cashnote_redemption.to_bytes().unwrap();
        let cipher = cashnote_redemption.encrypt(pk).unwrap();

        let cashnote_redemption2 = CashNoteRedemption::from_bytes(&bytes).unwrap();
        let cashnote_redemption3 = CashNoteRedemption::decrypt(&cipher, &sk).unwrap();

        assert_eq!(cashnote_redemption, cashnote_redemption2);
        assert_eq!(cashnote_redemption, cashnote_redemption3);
    }

    #[test]
    fn test_cashnote_redemption_transfer() {
        let rng = &mut bls::rand::thread_rng();
        let cashnote_redemption = CashNoteRedemption::new(
            DerivationIndex([42; 32]),
            SpendAddress::new(XorName::random(rng)),
        );
        let sk = MainSecretKey::random();
        let pk = sk.main_pubkey();

        let payment = Transfer::create(vec![cashnote_redemption.clone()], pk).unwrap();
        let cashnote_redemptions = payment.cashnote_redemptions(&sk).unwrap();

        assert_eq!(cashnote_redemptions, vec![cashnote_redemption]);
    }
}
