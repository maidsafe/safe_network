// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    DerivationIndex, DerivedSecretKey, Hash, MainPubkey, MainSecretKey, NanoTokens, SignedSpend,
    Transaction, UniquePubkey,
};

use crate::{Result, TransferError};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tiny_keccak::{Hasher, Sha3};

/// Represents a CashNote (CashNote).
///
/// A CashNote is like a check. Only the recipient can spend it.
///
/// A CashNote has a MainPubkey representing the recipient of the CashNote.
///
/// An MainPubkey consists of a PublicKey.
/// The user who receives payments to this MainPubkey, will be holding
/// a MainSecretKey - a secret key, which corresponds to the MainPubkey.
///
/// The MainPubkey can be given out to multiple parties and
/// multiple CashNotes can share the same MainPubkey.
///
/// The spentbook nodes never sees the MainPubkey. Instead, when a
/// transaction output cashnote is created for a given MainPubkey, a random
/// derivation index is generated and used to derive a UniquePubkey, which will be
/// used for this new cashnote.
///
/// The UniquePubkey is a unique identifier of a CashNote.
/// So there can only ever be one CashNote with that id, previously, now and forever.
/// The UniquePubkey consists of a PublicKey. To unlock the tokens of the CashNote,
/// the corresponding DerivedSecretKey (consists of a SecretKey) must be used.
/// It is derived from the MainSecretKey, in the same way as the UniquePubkey was derived
/// from the MainPubkey to get the UniquePubkey.
///
/// So, there are two important pairs to conceptually be aware of.
/// The MainSecretKey and MainPubkey is a unique pair of a user, where the MainSecretKey
/// is held secret, and the MainPubkey is given to all and anyone who wishes to send tokens to you.
/// A sender of tokens will derive the UniquePubkey from the MainPubkey, which will identify the CashNote that
/// holds the tokens going to the recipient. The sender does this using a derivation index.
/// The recipient of the tokens, will use the same derivation index, to derive the DerivedSecretKey
/// from the MainSecretKey. The DerivedSecretKey and UniquePubkey pair is the second important pair.
/// For an outsider, there is no way to associate either the DerivedSecretKey or the UniquePubkey to the MainPubkey
/// (or for that matter to the MainSecretKey, if they were ever to see it, which they shouldn't of course).
/// Only by having the derivation index, which is only known to sender and recipient, can such a connection be made.
///
/// To spend or work with a CashNote, wallet software must obtain the corresponding
/// MainSecretKey from the user, and then call an API function that accepts a MainSecretKey,
/// eg: `cashnote.derivation_index(&main_key)`
#[derive(custom_debug::Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct CashNote {
    /// The unique public key of this CashNote. It is unique, and there can never
    /// be another CashNote with the same public key. It used in SignedSpends.
    pub unique_pubkey: UniquePubkey,
    /// The transaction where this CashNote was created.
    #[debug(skip)]
    pub parent_tx: Transaction,
    /// The transaction's input's SignedSpends
    pub parent_spends: BTreeSet<SignedSpend>,
    /// This is the MainPubkey of the owner of this CashNote
    pub main_pubkey: MainPubkey,
    /// The derivation index used to derive the UniquePubkey and DerivedSecretKey from the MainPubkey and MainSecretKey respectively.
    /// It is to be kept secret to preserve the privacy of the owner.
    /// Without it, it is very hard to link the MainPubkey (original owner) and the UniquePubkey (derived unique identity of the CashNote)
    pub derivation_index: DerivationIndex,
}

impl CashNote {
    /// Return the unique pubkey of this CashNote.
    pub fn unique_pubkey(&self) -> UniquePubkey {
        self.unique_pubkey
    }

    // Return MainPubkey from which UniquePubkey is derived.
    pub fn main_pubkey(&self) -> &MainPubkey {
        &self.main_pubkey
    }

    /// Return DerivedSecretKey using MainSecretKey supplied by caller.
    /// Will return an error if the supplied MainSecretKey does not match the
    /// CashNote MainPubkey.
    pub fn derived_key(&self, main_key: &MainSecretKey) -> Result<DerivedSecretKey> {
        if &main_key.main_pubkey() != self.main_pubkey() {
            return Err(TransferError::MainSecretKeyDoesNotMatchMainPubkey);
        }
        Ok(main_key.derive_key(&self.derivation_index()))
    }

    /// Return UniquePubkey using MainPubkey supplied by caller.
    /// Will return an error if the supplied MainPubkey does not match the
    /// CashNote MainPubkey.
    pub fn derived_pubkey(&self, main_pubkey: &MainPubkey) -> Result<UniquePubkey> {
        if main_pubkey != self.main_pubkey() {
            return Err(TransferError::MainPubkeyMismatch);
        }
        Ok(main_pubkey.new_unique_pubkey(&self.derivation_index()))
    }

    /// Return the derivation index that was used to derive UniquePubkey and corresponding DerivedSecretKey of a CashNote.
    pub fn derivation_index(&self) -> DerivationIndex {
        self.derivation_index
    }

    /// Return the reason why this CashNote was spent.
    /// Will be the default Hash (empty) if reason is none.
    pub fn reason(&self) -> Hash {
        self.parent_spends
            .iter()
            .next()
            .map(|c| c.reason())
            .unwrap_or_default()
    }

    /// Return the value in NanoTokens for this CashNote.
    pub fn value(&self) -> Result<NanoTokens> {
        Ok(self
            .parent_tx
            .outputs
            .iter()
            .find(|o| &self.unique_pubkey() == o.unique_pubkey())
            .ok_or(TransferError::OutputNotFound)?
            .amount)
    }

    /// Generate the hash of this CashNote
    pub fn hash(&self) -> Hash {
        let mut sha3 = Sha3::v256();
        sha3.update(self.parent_tx.hash().as_ref());
        sha3.update(&self.main_pubkey.to_bytes());
        sha3.update(&self.derivation_index.0);

        for sp in self.parent_spends.iter() {
            sha3.update(&sp.to_bytes());
        }

        sha3.update(self.reason().as_ref());
        let mut hash = [0u8; 32];
        sha3.finalize(&mut hash);
        Hash::from(hash)
    }

    /// Verifies that this CashNote is valid.
    ///
    /// A CashNote recipient should call this immediately upon receipt.
    ///
    /// important: this will verify there is a matching transaction provided
    /// for each SignedSpend, although this does not check if the CashNote has been spent.
    /// For that, one must query the spentbook nodes.
    ///
    /// Note that the spentbook nodes cannot perform this check.  Only the CashNote
    /// recipient (private key holder) can.
    ///
    /// see TransactionVerifier::verify() for a description of
    /// verifier requirements.
    pub fn verify(&self, main_key: &MainSecretKey) -> Result<(), TransferError> {
        self.parent_tx
            .verify_against_inputs_spent(self.parent_spends.iter())?;

        let unique_pubkey = self.derived_key(main_key)?.unique_pubkey();
        if !self
            .parent_tx
            .outputs
            .iter()
            .any(|o| unique_pubkey.eq(o.unique_pubkey()))
        {
            return Err(TransferError::CashNoteCiphersNotPresentInTransactionOutput);
        }

        // verify that all signed_spend reasons are equal
        let reason = self.reason();
        let reasons_are_equal = |s: &SignedSpend| reason == s.reason();
        if !self.parent_spends.iter().all(reasons_are_equal) {
            return Err(TransferError::SignedSpendReasonMismatch(unique_pubkey));
        }
        Ok(())
    }

    /// Deserializes a `CashNote` represented as a hex string to a `CashNote`.
    pub fn from_hex(hex: &str) -> Result<Self, TransferError> {
        let mut bytes =
            hex::decode(hex).map_err(|e| TransferError::HexDeserializationFailed(e.to_string()))?;
        bytes.reverse();
        let cashnote: CashNote = rmp_serde::from_slice(&bytes)
            .map_err(|e| TransferError::HexDeserializationFailed(e.to_string()))?;
        Ok(cashnote)
    }

    /// Serialize this `CashNote` instance to a hex string.
    pub fn to_hex(&self) -> Result<String, TransferError> {
        let mut serialized = rmp_serde::to_vec(&self)
            .map_err(|e| TransferError::HexSerializationFailed(e.to_string()))?;
        serialized.reverse();
        Ok(hex::encode(serialized))
    }
}
