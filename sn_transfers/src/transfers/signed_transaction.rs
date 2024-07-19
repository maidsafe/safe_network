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
    CashNote, DerivationIndex, MainPubkey, MainSecretKey, NanoTokens, OutputPurpose, SignedSpend,
    SpendReason, TransferError, UnsignedTransaction,
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
    pub fn new(
        available_cash_notes: Vec<CashNote>,
        recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex, OutputPurpose)>,
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
    pub fn verify(&self, main_key: &MainSecretKey) -> Result<()> {
        for cn in self.output_cashnotes.iter() {
            cn.verify(main_key)?;
        }
        if let Some(ref cn) = self.change_cashnote {
            cn.verify(main_key)?;
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
