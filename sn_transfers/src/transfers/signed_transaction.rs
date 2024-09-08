// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::BTreeSet;

use crate::error::Result;
use crate::{
    CashNote, DerivationIndex, MainPubkey, MainSecretKey, NanoTokens, SignedSpend, SpendReason,
    TransferError, UnsignedTransaction,
};
use serde::{Deserialize, Serialize};

/// A local transaction that has been signed and is ready to be executed on the Network
#[derive(custom_debug::Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedTransaction {
    /// Output CashNotes ready to be packaged into a `Transfer`
    #[debug(skip)]
    pub output_cashnotes: Vec<CashNote>,
    /// Change CashNote ready to be added back to our wallet
    #[debug(skip)]
    pub change_cashnote: Option<CashNote>,
    /// All the spends ready to be sent to the Network
    pub spends: BTreeSet<SignedSpend>,
}

impl SignedTransaction {
    /// Create a new `SignedTransaction`
    /// - `available_cash_notes`: provide the available cash notes assumed to be not spent yet
    /// - `recipients`: recipient amounts, mainpubkey, the random derivation index to use, and whether it is royalty fee
    /// - `change_to`: what mainpubkey to give the change to
    /// - `input_reason_hash`: an optional `SpendReason`
    /// - `main_key`: the main secret key that owns the available cash notes, used for signature
    pub fn new(
        available_cash_notes: Vec<CashNote>,
        recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex, bool)>,
        change_to: MainPubkey,
        input_reason_hash: SpendReason,
        main_key: &MainSecretKey,
    ) -> Result<Self> {
        let unsigned_tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )?;
        let signed_tx = unsigned_tx.sign(main_key)?;
        Ok(signed_tx)
    }

    /// Verify the `SignedTransaction`
    pub fn verify(&self) -> Result<()> {
        for cn in self.output_cashnotes.iter() {
            cn.verify()?;
        }
        if let Some(ref cn) = self.change_cashnote {
            cn.verify()?;
        }
        for spend in self.spends.iter() {
            spend.verify()?;
        }
        Ok(())
    }

    /// Create a new `SignedTransaction` from a hex string
    pub fn from_hex(hex: &str) -> Result<Self> {
        let decoded_hex = hex::decode(hex).map_err(|e| {
            TransferError::TransactionSerialization(format!("Hex decode failed: {e}"))
        })?;
        let s = rmp_serde::from_slice(&decoded_hex).map_err(|e| {
            TransferError::TransactionSerialization(format!("Failed to deserialize: {e}"))
        })?;
        Ok(s)
    }

    /// Return the hex representation of the `SignedTransaction`
    pub fn to_hex(&self) -> Result<String> {
        Ok(hex::encode(rmp_serde::to_vec(self).map_err(|e| {
            TransferError::TransactionSerialization(format!("Failed to serialize: {e}"))
        })?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsigned_tx_serialization() -> Result<()> {
        let mut rng = rand::thread_rng();
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 100);

        let available_cash_notes = vec![CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        }];
        let recipients = vec![
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        let signed_tx = tx.sign(&cnr_sk).expect("Sign to succeed");

        let hex = signed_tx.to_hex()?;
        let signed_tx2 = SignedTransaction::from_hex(&hex)?;

        assert_eq!(signed_tx, signed_tx2);
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_verify_simple() -> Result<()> {
        let mut rng = rand::thread_rng();
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 100);

        let available_cash_notes = vec![CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        }];
        let recipients = vec![
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        let signed_tx = tx.sign(&cnr_sk).expect("Sign to succeed");

        let res = signed_tx.verify();
        assert_eq!(res, Ok(()));
        Ok(())
    }
}
