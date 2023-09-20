// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{CashNote, DerivationIndex, MainPubkey, SpendAddress};

use bls::{Ciphertext, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::error::{Error, Result};

/// Transfer sent to a recipient
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, custom_debug::Debug)]
pub struct Transfer {
    /// List of encrypted CashNoteRedemptions from which a recipient can verify and get money
    /// Only the recipient can decrypt these CashNoteRedemptions
    encrypted_cashnote_redemptions: Vec<Ciphertext>,
}

impl Transfer {
    /// This function is used to create Transfer from CashNotes, can be done offline, and sent to the recipient.
    /// Creates Transfers from the given cash_notes
    /// Grouping CashNotes by recipient into different transfers
    /// This Transfer can be sent safely to the recipients as all data in it is encrypted
    /// The recipients can then decrypt the data and use it to verify and reconstruct the CashNotes
    pub fn transfers_from_cash_notes(cash_notes: Vec<CashNote>) -> Result<Vec<Transfer>> {
        let mut cashnote_redemptions_map: BTreeMap<MainPubkey, Vec<CashNoteRedemption>> =
            BTreeMap::new();
        for cash_note in cash_notes {
            let recipient = cash_note.main_pubkey;
            let derivation_index = cash_note.derivation_index();
            let parent_spend_addr = match cash_note.signed_spends.iter().next() {
                Some(s) => SpendAddress::from_unique_pubkey(s.unique_pubkey()),
                None => {
                    warn!(
                        "Skipping CashNote {cash_note:?} while creating Transfer as it has no parent spends."
                    );
                    continue;
                }
            };

            let u = CashNoteRedemption::new(derivation_index, parent_spend_addr);
            cashnote_redemptions_map
                .entry(recipient)
                .or_insert_with(Vec::new)
                .push(u);
        }

        let mut transfers = Vec::new();
        for (recipient, cashnote_redemptions) in cashnote_redemptions_map {
            let t = Transfer::create(cashnote_redemptions, recipient.0)
                .map_err(|_| Error::CashNoteRedemptionEncryptionFailed)?;
            transfers.push(t)
        }
        Ok(transfers)
    }

    /// Create a new transfer
    /// cashnote_redemptions: List of CashNoteRedemptions to be used for payment
    /// recipient: main Public key (donation key) of the recipient,
    ///     not to be confused with the derived keys
    pub fn create(
        cashnote_redemptions: Vec<CashNoteRedemption>,
        recipient: PublicKey,
    ) -> Result<Self> {
        let encrypted_cashnote_redemptions = cashnote_redemptions
            .into_iter()
            .map(|cashnote_redemption| cashnote_redemption.encrypt(recipient))
            .collect::<Result<Vec<Ciphertext>>>()?;
        Ok(Transfer {
            encrypted_cashnote_redemptions,
        })
    }

    /// Get the CashNoteRedemptions from the Payment
    /// This is used by the recipient of a payment to decrypt the cashnote_redemptions in a payment
    pub fn cashnote_redemptions(&self, sk: &SecretKey) -> Result<Vec<CashNoteRedemption>> {
        let mut cashnote_redemptions = Vec::new();
        for cypher in &self.encrypted_cashnote_redemptions {
            let cashnote_redemption = CashNoteRedemption::decrypt(cypher, sk)?;
            cashnote_redemptions.push(cashnote_redemption);
        }
        Ok(cashnote_redemptions)
    }

    /// Deserializes a `Transfer` represented as a hex string to a `Transfer`.
    pub fn from_hex(hex: &str) -> Result<Self> {
        let mut bytes = hex::decode(hex).map_err(|_| Error::TransferDeserializationFailed)?;
        bytes.reverse();
        let transfer: Transfer =
            bincode::deserialize(&bytes).map_err(|_| Error::TransferDeserializationFailed)?;
        Ok(transfer)
    }

    /// Serialize this `Transfer` instance to a readable hex string that a human can copy paste
    pub fn to_hex(&self) -> Result<String> {
        let mut serialized =
            bincode::serialize(&self).map_err(|_| Error::TransferSerializationFailed)?;
        serialized.reverse();
        Ok(hex::encode(serialized))
    }
}

/// Unspent Transaction (Tx) Output
/// Information can be used by the Tx recipient of this output
/// to check that they recieved money and to spend it
///
/// This struct contains sensitive information that should be kept secret
/// so it should be encrypted to the recipient's public key (public address)
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, custom_debug::Debug)]
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

    /// Serialize the CashNoteRedemption to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(self).map_err(|_| Error::CashNoteRedemptionSerialisationFailed)
    }

    /// Deserialize the CashNoteRedemption from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes).map_err(|_| Error::CashNoteRedemptionSerialisationFailed)
    }

    /// Encrypt the CashNoteRedemption to a public key
    pub fn encrypt(&self, pk: PublicKey) -> Result<Ciphertext> {
        let bytes = self.to_bytes()?;
        Ok(pk.encrypt(bytes))
    }

    /// Decrypt the CashNoteRedemption with a secret key
    pub fn decrypt(cypher: &Ciphertext, sk: &SecretKey) -> Result<Self> {
        let bytes = sk
            .decrypt(cypher)
            .ok_or(Error::CashNoteRedemptionDecryptionFailed)?;
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
        let cashnote_redemption =
            CashNoteRedemption::new([42; 32], SpendAddress::new(XorName::random(rng)));
        let sk = SecretKey::random();
        let pk = sk.public_key();

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
        let cashnote_redemption =
            CashNoteRedemption::new([42; 32], SpendAddress::new(XorName::random(rng)));
        let sk = SecretKey::random();
        let pk = sk.public_key();

        let payment = Transfer::create(vec![cashnote_redemption.clone()], pk).unwrap();
        let cashnote_redemptions = payment.cashnote_redemptions(&sk).unwrap();

        assert_eq!(cashnote_redemptions, vec![cashnote_redemption]);
    }
}
