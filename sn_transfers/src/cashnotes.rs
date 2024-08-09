// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod address;
mod cashnote;
mod hash;
mod nano;
mod signed_spend;
mod spend_reason;
mod unique_keys;

pub use address::SpendAddress;
pub use cashnote::CashNote;
pub use hash::Hash;
pub use nano::NanoTokens;
pub use signed_spend::{SignedSpend, Spend};
pub use spend_reason::SpendReason;
pub use unique_keys::{DerivationIndex, DerivedSecretKey, MainPubkey, MainSecretKey, UniquePubkey};

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::TransferError;

    use std::collections::{BTreeMap, BTreeSet};

    fn generate_parent_spends(
        derived_sk: DerivedSecretKey,
        amount: u64,
        output: UniquePubkey,
    ) -> BTreeSet<SignedSpend> {
        let mut descendants = BTreeMap::new();
        let _ = descendants.insert(output, NanoTokens::from(amount));
        let spend = Spend {
            unique_pubkey: derived_sk.unique_pubkey(),
            reason: SpendReason::default(),
            ancestors: BTreeSet::new(),
            descendants,
            royalties: vec![],
        };
        let mut parent_spends = BTreeSet::new();
        let derived_key_sig = derived_sk.sign(&spend.to_bytes_for_signing());
        let _ = parent_spends.insert(SignedSpend {
            spend,
            derived_key_sig,
        });
        parent_spends
    }

    #[test]
    fn from_hex_should_deserialize_a_hex_encoded_string_to_a_cashnote() -> Result<(), TransferError>
    {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let amount = 1_530_000_000;
        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = DerivationIndex::random(&mut rng);
        let derived_key = main_key.derive_key(&derivation_index);

        let parent_spends = generate_parent_spends(
            main_key.derive_key(&DerivationIndex::random(&mut rng)),
            amount,
            derived_key.unique_pubkey(),
        );

        let cashnote = CashNote {
            parent_spends,
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        let hex = cashnote.to_hex()?;

        let cashnote = CashNote::from_hex(&hex)?;
        assert_eq!(cashnote.value().as_nano(), 1_530_000_000);

        Ok(())
    }

    #[test]
    fn to_hex_should_serialize_a_cashnote_to_a_hex_encoded_string() -> Result<(), TransferError> {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let amount = 100;
        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = DerivationIndex::random(&mut rng);
        let derived_key = main_key.derive_key(&derivation_index);

        let parent_spends = generate_parent_spends(
            main_key.derive_key(&DerivationIndex::random(&mut rng)),
            amount,
            derived_key.unique_pubkey(),
        );

        let cashnote = CashNote {
            parent_spends,
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        let hex = cashnote.to_hex()?;
        let cashnote_from_hex = CashNote::from_hex(&hex)?;

        assert_eq!(cashnote.value(), cashnote_from_hex.value());

        Ok(())
    }

    #[test]
    fn input_should_error_if_unique_pubkey_is_not_derived_from_main_key(
    ) -> Result<(), TransferError> {
        let mut rng = crate::rng::from_seed([0u8; 32]);
        let amount = 100;

        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = DerivationIndex::random(&mut rng);
        let derived_key = main_key.derive_key(&derivation_index);

        let parent_spends = generate_parent_spends(
            main_key.derive_key(&DerivationIndex::random(&mut rng)),
            amount,
            derived_key.unique_pubkey(),
        );

        let cashnote = CashNote {
            parent_spends,
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        let other_main_key = MainSecretKey::random_from_rng(&mut rng);
        let result = cashnote.derived_key(&other_main_key);
        assert!(matches!(
            result,
            Err(TransferError::MainSecretKeyDoesNotMatchMainPubkey)
        ));
        Ok(())
    }

    #[test]
    fn test_cashnote_without_inputs_fails_verification() -> Result<(), TransferError> {
        let mut rng = crate::rng::from_seed([0u8; 32]);

        let main_key = MainSecretKey::random_from_rng(&mut rng);
        let derivation_index = DerivationIndex::random(&mut rng);

        let cashnote = CashNote {
            parent_spends: Default::default(),
            main_pubkey: main_key.main_pubkey(),
            derivation_index,
        };

        assert!(matches!(
            cashnote.verify(),
            Err(TransferError::CashNoteMissingAncestors)
        ));

        Ok(())
    }
}
