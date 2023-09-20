// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    transaction::Transaction, unique_keys::MainPubkey, DerivationIndex, DerivedSecretKey, Error,
    FeeOutput, Hash, MainSecretKey, NanoTokens, Result, SignedSpend, UniquePubkey,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tiny_keccak::{Hasher, Sha3};

/// Represents a Digital Bearer Certificate (CashNote).
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
#[derive(custom_debug::Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CashNote {
    /// The unique pulbic key of this CashNote. It is unique, and there can never
    /// be another CashNote with the same pulbic key. It used in SignedSpends.
    pub id: UniquePubkey,
    /// The transaction where this CashNote was created.
    #[debug(skip)]
    pub src_tx: Transaction,
    /// The transaction's input's SignedSpends
    pub signed_spends: BTreeSet<SignedSpend>,
    /// This is the MainPubkey of the recipient of this CashNote.
    pub main_pubkey: MainPubkey,
    /// This indicates which index to use when deriving the UniquePubkey of the
    /// CashNote, from the MainPubkey.
    pub derivation_index: DerivationIndex,
}

impl CashNote {
    /// Return the id of this CashNote.
    pub fn unique_pubkey(&self) -> UniquePubkey {
        self.id
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
            return Err(Error::MainSecretKeyDoesNotMatchMainPubkey);
        }
        Ok(main_key.derive_key(&self.derivation_index()))
    }

    /// Return the derivation index that was used to derive UniquePubkey and corresponding DerivedSecretKey of a CashNote.
    pub fn derivation_index(&self) -> DerivationIndex {
        self.derivation_index
    }

    /// Return the fee output used in the source transaction
    pub fn fee_output(&self) -> &FeeOutput {
        &self.src_tx.fee
    }

    /// Return the reason why this CashNote was spent.
    /// Will be the default Hash (empty) if reason is none.
    pub fn reason(&self) -> Hash {
        self.signed_spends
            .iter()
            .next()
            .map(|c| c.reason())
            .unwrap_or_default()
    }

    /// Return the Nanos for this CashNote.
    pub fn token(&self) -> Result<NanoTokens> {
        Ok(self
            .src_tx
            .outputs
            .iter()
            .find(|o| &self.unique_pubkey() == o.unique_pubkey())
            .ok_or(Error::OutputNotFound)?
            .amount)
    }

    /// Generate the hash of this CashNote
    pub fn hash(&self) -> Hash {
        let mut sha3 = Sha3::v256();
        sha3.update(self.src_tx.hash().as_ref());
        sha3.update(&self.main_pubkey.to_bytes());
        sha3.update(&self.derivation_index);

        for sp in self.signed_spends.iter() {
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
    pub fn verify(&self, main_key: &MainSecretKey) -> Result<(), Error> {
        self.src_tx
            .verify_against_inputs_spent(&self.signed_spends)?;

        let unique_pubkey = self.derived_key(main_key)?.unique_pubkey();
        if !self
            .src_tx
            .outputs
            .iter()
            .any(|o| unique_pubkey.eq(o.unique_pubkey()))
        {
            return Err(Error::CashNoteCiphersNotPresentInTransactionOutput);
        }

        // verify that all signed_spend reasons are equal
        let reason = self.reason();
        let reasons_are_equal = |s: &SignedSpend| reason == s.reason();
        if !self.signed_spends.iter().all(reasons_are_equal) {
            return Err(Error::SignedSpendReasonMismatch(unique_pubkey));
        }
        Ok(())
    }

    /// Deserializes a `CashNote` represented as a hex string to a `CashNote`.
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let mut bytes =
            hex::decode(hex).map_err(|e| Error::HexDeserializationFailed(e.to_string()))?;
        bytes.reverse();
        let cashnote: CashNote = bincode::deserialize(&bytes)
            .map_err(|e| Error::HexDeserializationFailed(e.to_string()))?;
        Ok(cashnote)
    }

    /// Serialize this `CashNote` instance to a hex string.
    pub fn to_hex(&self) -> Result<String, Error> {
        let mut serialized =
            bincode::serialize(&self).map_err(|e| Error::HexSerializationFailed(e.to_string()))?;
        serialized.reverse();
        Ok(hex::encode(serialized))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use crate::{
        mock,
        rand::{CryptoRng, RngCore},
        transaction::Output,
        unique_keys::random_derivation_index,
        FeeOutput, Hash, NanoTokens,
    };
    use bls::{PublicKey, SecretKey};
    use std::convert::TryInto;

    #[test]
    fn from_hex_should_deserialize_a_hex_encoded_string_to_a_cashnote() -> Result<(), Error> {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let amount = 1_530_000_000;
        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = random_derivation_index(&mut rng);
        let derived_key = main_key.derive_key(&derivation_index);
        let tx = Transaction {
            inputs: vec![],
            outputs: vec![Output::new(derived_key.unique_pubkey(), amount)],
            fee: FeeOutput::new(Hash::default(), 3_500, Hash::default()),
        };
        let cashnote = CashNote {
            id: derived_key.unique_pubkey(),
            src_tx: tx,
            signed_spends: Default::default(),
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        let hex = cashnote.to_hex()?;

        let cashnote = CashNote::from_hex(&hex)?;
        assert_eq!(cashnote.token()?.as_nano(), 1_530_000_000);

        let fee_amount = cashnote.fee_output().token;
        assert_eq!(fee_amount, NanoTokens::from(3_500));

        Ok(())
    }

    #[test]
    fn to_hex_should_serialize_a_cashnote_to_a_hex_encoded_string() -> Result<(), Error> {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let amount = 100;
        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = random_derivation_index(&mut rng);
        let derived_key = main_key.derive_key(&derivation_index);
        let tx = Transaction {
            inputs: vec![],
            outputs: vec![Output::new(derived_key.unique_pubkey(), amount)],
            fee: FeeOutput::new(Hash::default(), 2_500, Hash::default()),
        };
        let cashnote = CashNote {
            id: derived_key.unique_pubkey(),
            src_tx: tx,
            signed_spends: Default::default(),
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        let hex = cashnote.to_hex()?;
        let cashnote_from_hex = CashNote::from_hex(&hex)?;

        assert_eq!(cashnote.token()?, cashnote_from_hex.token()?);

        let fee_amount = cashnote.fee_output().token;
        assert_eq!(fee_amount, NanoTokens::from(2_500));

        Ok(())
    }

    #[test]
    fn input_should_error_if_unique_pubkey_is_not_derived_from_main_key() -> Result<(), Error> {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let (_, _, (cashnote, _)) = generate_cashnote_of_value_from_pk_hex(
            100,
            "a14a1887c61f95d5bdf6d674da3032dad77f2168fe6bf5e282aa02394bd45f41f0\
            fe722b61fa94764da42a9b628701db",
            &mut rng,
        )?;
        let sk = get_secret_key_from_hex(
            "d823b03be25ad306ce2c2ef8f67d8a49322ed2a8636de5dbf01f6cc3467dc91e",
        )?;
        let main_key = MainSecretKey::new(sk);
        let result = cashnote.derived_key(&main_key);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Main key does not match public address."
        );
        Ok(())
    }

    #[test]
    fn test_cashnote_without_inputs_fails_verification() -> Result<(), Error> {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let amount = 100;

        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = random_derivation_index(&mut rng);
        let derived_key = main_key.derive_key(&derivation_index);

        let tx = Transaction {
            inputs: vec![],
            outputs: vec![Output::new(derived_key.unique_pubkey(), amount)],
            fee: FeeOutput::default(),
        };

        let cashnote = CashNote {
            id: derived_key.unique_pubkey(),
            src_tx: tx,
            signed_spends: Default::default(),
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        assert!(matches!(
            cashnote.verify(&main_key),
            Err(Error::MissingTxInputs)
        ));

        Ok(())
    }

    pub(crate) fn generate_cashnote_of_value_from_pk_hex(
        amount: u64,
        pk_hex: &str,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(mock::SpentbookNode, CashNote, (CashNote, CashNote))> {
        let pk_bytes =
            hex::decode(pk_hex).map_err(|e| Error::HexDeserializationFailed(e.to_string()))?;
        let pk_bytes: [u8; bls::PK_SIZE] = pk_bytes.try_into().unwrap_or_else(|v: Vec<u8>| {
            panic!(
                "Expected vec of length {} but received vec of length {}",
                bls::PK_SIZE,
                v.len()
            )
        });
        let pk = PublicKey::from_bytes(pk_bytes)?;
        let main_pubkey = MainPubkey::new(pk);
        generate_cashnote_of_value(amount, main_pubkey, rng)
    }

    fn generate_cashnote_of_value(
        amount: u64,
        recipient: MainPubkey,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(mock::SpentbookNode, CashNote, (CashNote, CashNote))> {
        let (mut spentbook_node, genesis_cashnote, genesis_material, _) =
            mock::GenesisBuilder::init_genesis_single()?;

        let output_tokens = vec![
            NanoTokens::from(amount),
            NanoTokens::from(mock::GenesisMaterial::GENESIS_AMOUNT - amount),
        ];

        let derived_key = genesis_cashnote.derived_key(&genesis_material.main_key)?;
        let cashnote_builder = crate::TransactionBuilder::default()
            .add_input_cashnote(&genesis_cashnote, &derived_key)
            .unwrap()
            .add_outputs(
                output_tokens
                    .into_iter()
                    .map(|token| (token, recipient, random_derivation_index(rng))),
            )
            .build(Hash::default())?;

        let tx = &cashnote_builder.spent_tx;
        for signed_spend in cashnote_builder.signed_spends() {
            spentbook_node.log_spent(tx, signed_spend)?
        }

        let mut iter = cashnote_builder.build()?.into_iter();
        let (starting_cashnote, _) = iter.next().unwrap();
        let (change_cashnote, _) = iter.next().unwrap();

        Ok((
            spentbook_node,
            genesis_cashnote,
            (starting_cashnote, change_cashnote),
        ))
    }

    fn get_secret_key_from_hex(sk_hex: &str) -> Result<SecretKey, Error> {
        let sk_bytes =
            hex::decode(sk_hex).map_err(|e| Error::HexDeserializationFailed(e.to_string()))?;
        let mut sk_bytes: [u8; bls::SK_SIZE] = sk_bytes.try_into().unwrap_or_else(|v: Vec<u8>| {
            panic!(
                "Expected vec of length {} but received vec of length {}",
                bls::SK_SIZE,
                v.len()
            )
        });
        sk_bytes.reverse();
        Ok(SecretKey::from_bytes(sk_bytes)?)
    }
}
