// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg(test)]
mod tests {
    use crate::{
        mock, random_derivation_index,
        tests::{TinyInt, TinyVec},
        CashNote, DerivedSecretKey, Error, Hash, MainSecretKey, Nano, Result, SignedSpend, Spend,
        TransactionBuilder,
    };
    use bls::SecretKey;
    use quickcheck_macros::quickcheck;
    use std::collections::{BTreeMap, BTreeSet};
    use std::iter::FromIterator;

    #[test]
    fn issue_genesis() -> Result<(), Error> {
        let (_spentbook_node, genesis_cashnote, genesis, _token) =
            mock::GenesisBuilder::init_genesis_single()?;

        let verified = genesis_cashnote.verify(&genesis.main_key);
        assert!(verified.is_ok());

        Ok(())
    }

    #[quickcheck]
    fn prop_splitting_the_genesis_cashnote(output_amounts: TinyVec<TinyInt>) -> Result<(), Error> {
        let mut rng = crate::rng::from_seed([0u8; 32]);

        let mut output_amounts =
            Vec::from_iter(output_amounts.into_iter().map(TinyInt::coerce::<u64>));
        output_amounts
            .push(mock::GenesisMaterial::GENESIS_AMOUNT - output_amounts.iter().sum::<u64>());

        let n_outputs = output_amounts.len();
        let output_amount: u64 = output_amounts.iter().sum();

        let (mut spentbook_node, genesis_cashnote, genesis, _token) =
            mock::GenesisBuilder::init_genesis_single()?;

        let first_output_key_map: BTreeMap<_, _> = output_amounts
            .iter()
            .map(|amount| {
                let main_key = MainSecretKey::random_from_rng(&mut rng);
                let derivation_index = random_derivation_index(&mut rng);
                let unique_pubkey = main_key.main_pubkey().new_unique_pubkey(&derivation_index);
                (
                    unique_pubkey,
                    (main_key, derivation_index, Nano::from_nano(*amount)),
                )
            })
            .collect();

        let derived_key = genesis_cashnote.derived_key(&genesis.main_key).unwrap();
        let cashnote_builder = TransactionBuilder::default()
            .add_input_cashnote(&genesis_cashnote, &derived_key)?
            .add_outputs(first_output_key_map.values().map(
                |(main_key, derivation_index, amount)| {
                    (*amount, main_key.main_pubkey(), *derivation_index)
                },
            ))
            .build(Hash::default())?;

        // We make this a closure to keep the spentbook loop readable.
        let check_error = |error: Error| -> Result<()> {
            match error {
                Error::InconsistentTransaction => {
                    // Verify that no outputs were present and we got correct verification error.
                    assert_eq!(n_outputs, 0);
                    Ok(())
                }
                _ => Err(error),
            }
        };

        let tx = &cashnote_builder.spent_tx;
        for signed_spend in cashnote_builder.signed_spends() {
            match spentbook_node.log_spent(tx, signed_spend) {
                Ok(s) => s,
                Err(e) => return check_error(e),
            };
        }
        let output_cashnotes = cashnote_builder.build()?;

        for (cashnote, output_token) in output_cashnotes.iter() {
            let (main_key, _, token) = first_output_key_map.get(&cashnote.unique_pubkey()).unwrap();
            let cashnote_token = cashnote.token()?;
            assert_eq!(token, &cashnote_token);
            assert_eq!(cashnote_token, *output_token);
            assert!(cashnote.verify(main_key).is_ok());
        }

        assert_eq!(
            {
                let mut sum: u64 = 0;
                for (cashnote, _) in output_cashnotes.iter() {
                    // note: we could just use the amount provided by CashNoteBuilder::build()
                    // but we go further to verify the correct value is encrypted in the CashNote.
                    sum += cashnote.token()?.as_nano()
                }
                sum
            },
            output_amount
        );

        Ok(())
    }

    #[quickcheck]
    fn prop_cashnote_transaction_many_to_many(
        // the amount of each input transaction
        input_amounts: TinyVec<TinyInt>,
        // The amount for each transaction output
        output_amounts: TinyVec<TinyInt>,
        // Include an invalid SignedSpends for the following inputs
        invalid_signed_spends: TinyVec<TinyInt>,
    ) -> Result<(), Error> {
        let mut rng = crate::rng::from_seed([0u8; 32]);

        let mut first_input_amounts =
            Vec::from_iter(input_amounts.into_iter().map(TinyInt::coerce::<u64>));
        first_input_amounts
            .push(mock::GenesisMaterial::GENESIS_AMOUNT - first_input_amounts.iter().sum::<u64>());

        let mut first_output_amounts =
            Vec::from_iter(output_amounts.into_iter().map(TinyInt::coerce::<u64>));
        first_output_amounts
            .push(mock::GenesisMaterial::GENESIS_AMOUNT - first_output_amounts.iter().sum::<u64>());

        let invalid_signed_spends = BTreeSet::from_iter(
            invalid_signed_spends
                .into_iter()
                .map(TinyInt::coerce::<usize>),
        );

        let (mut spentbook_node, genesis_cashnote, genesis_material, _token) =
            mock::GenesisBuilder::init_genesis_single()?;

        let mut first_output_key_map: BTreeMap<_, _> = first_input_amounts
            .iter()
            .map(|amount| {
                let main_key = MainSecretKey::random_from_rng(&mut rng);
                let derivation_index = random_derivation_index(&mut rng);
                let unique_pubkey = main_key.main_pubkey().new_unique_pubkey(&derivation_index);
                (
                    unique_pubkey,
                    (main_key, derivation_index, Nano::from_nano(*amount)),
                )
            })
            .collect();

        let derived_key = genesis_cashnote
            .derived_key(&genesis_material.main_key)
            .unwrap();
        let cashnote_builder = TransactionBuilder::default()
            .add_input_cashnote(&genesis_cashnote, &derived_key)?
            .add_outputs(first_output_key_map.values().map(
                |(main_key, derivation_index, token)| {
                    (*token, main_key.main_pubkey(), *derivation_index)
                },
            ))
            .build(Hash::default())?;

        // note: we make this a closure to keep the spentbook loop readable.
        let check_tx_error = |error: Error| -> Result<()> {
            match error {
                Error::InconsistentTransaction => {
                    // Verify that no inputs were present and we got correct verification error.
                    assert!(first_input_amounts.is_empty());
                    Ok(())
                }
                _ => Err(error),
            }
        };

        let tx1 = cashnote_builder.spent_tx.clone();
        for signed_spend in cashnote_builder.signed_spends() {
            // normally spentbook verifies the tx, but here we skip it in order check reissue results.
            match spentbook_node.log_spent_and_skip_tx_verification(&tx1, signed_spend) {
                Ok(s) => s,
                Err(e) => return check_tx_error(e),
            };
        }

        let first_output_cashnotes = cashnote_builder.build()?;

        // The outputs become inputs for next tx.
        let second_inputs_cashnotes: Vec<(CashNote, DerivedSecretKey)> = first_output_cashnotes
            .into_iter()
            .map(|(cashnote, _)| {
                let (main_key, _, _) = first_output_key_map
                    .remove(&cashnote.unique_pubkey())
                    .unwrap();
                let derived_key = cashnote.derived_key(&main_key).unwrap();
                (cashnote, derived_key)
            })
            .collect();

        let second_inputs_cashnotes_len = second_inputs_cashnotes.len();

        let second_output_key_map: BTreeMap<_, _> = first_output_amounts
            .iter()
            .map(|amount| {
                let main_key = MainSecretKey::random_from_rng(&mut rng);
                let derivation_index = random_derivation_index(&mut rng);
                let unique_pubkey = main_key.main_pubkey().new_unique_pubkey(&derivation_index);
                (
                    unique_pubkey,
                    (main_key, derivation_index, Nano::from_nano(*amount)),
                )
            })
            .collect();

        let cashnote_builder = TransactionBuilder::default()
            .add_input_cashnotes(&second_inputs_cashnotes)?
            .add_outputs(second_output_key_map.values().map(
                |(main_key, derivation_index, token)| {
                    (*token, main_key.main_pubkey(), *derivation_index)
                },
            ))
            .build(Hash::default())?;

        let cashnote_output_amounts = first_output_amounts.clone();
        let output_total_amount: u64 = cashnote_output_amounts.iter().sum();

        assert_eq!(
            second_inputs_cashnotes_len,
            cashnote_builder.spent_tx.inputs.len()
        );
        assert_eq!(
            second_inputs_cashnotes_len,
            cashnote_builder.signed_spends().len()
        );

        let tx2 = cashnote_builder.spent_tx.clone();

        // note: we make this a closure because the logic is needed in
        // a couple places.
        let check_error = |error: Error| -> Result<()> {
            match error {
                Error::SignedSpendInputLenMismatch { expected, .. } => {
                    assert!(!invalid_signed_spends.is_empty());
                    assert_eq!(second_inputs_cashnotes_len, expected);
                }
                Error::SignedSpendInputIdMismatch => {
                    assert!(!invalid_signed_spends.is_empty());
                }
                Error::InconsistentTransaction => {
                    if mock::GenesisMaterial::GENESIS_AMOUNT == output_total_amount {
                        // This can correctly occur if there are 0 outputs and inputs sum to zero.
                        //
                        // The error occurs because there is no output
                        // to match against the input amount, and also no way to
                        // know that the input amount is zero.
                        assert!(first_output_amounts.is_empty());
                        assert_eq!(first_input_amounts.iter().sum::<u64>(), 0);
                        assert!(!first_input_amounts.is_empty());
                    }
                }
                Error::MissingTxInputs => {
                    assert_eq!(first_input_amounts.len(), 0);
                }
                Error::InvalidSpendSignature(unique_pubkey) => {
                    let idx = tx2
                        .inputs
                        .iter()
                        .position(|i| i.unique_pubkey() == unique_pubkey)
                        .unwrap();
                    assert!(invalid_signed_spends.contains(&idx));
                }
                _ => panic!("Unexpected err {:#?}", error),
            }
            Ok(())
        };

        let tx = &cashnote_builder.spent_tx;
        for (i, signed_spend) in cashnote_builder.signed_spends().into_iter().enumerate() {
            let is_invalid_signed_spend = invalid_signed_spends.contains(&i);

            let _signed_spend = match i % 2 {
                0 if is_invalid_signed_spend => {
                    // drop this signed spend
                    continue;
                }
                1 if is_invalid_signed_spend => {
                    // spentbook verifies the tx.  If an error, we need to check it
                    match spentbook_node.log_spent(tx, signed_spend) {
                        Ok(s) => s,
                        Err(e) => return check_error(e),
                    };
                    SignedSpend {
                        spend: Spend {
                            unique_pubkey: *signed_spend.unique_pubkey(),
                            spent_tx: signed_spend.spend.spent_tx.clone(),
                            reason: Hash::default(),
                            token: *signed_spend.token(),
                            cashnote_creation_tx: tx1.clone(),
                        },
                        derived_key_sig: SecretKey::random().sign([0u8; 32]),
                    }
                }
                _ => {
                    // spentbook verifies the tx.
                    match spentbook_node.log_spent(tx, signed_spend) {
                        Ok(()) => signed_spend.clone(),
                        Err(e) => return check_error(e),
                    }
                }
            };
        }

        let many_to_many_result = cashnote_builder.build();

        match many_to_many_result {
            Ok(second_output_cashnotes) => {
                assert_eq!(mock::GenesisMaterial::GENESIS_AMOUNT, output_total_amount);
                // assert!(invalid_signed_spends.iter().all(|i| i >= &tx2.inputs.len()));

                // The output amounts (from params) should correspond to the actual output_amounts
                assert_eq!(
                    BTreeSet::from_iter(cashnote_output_amounts.clone()),
                    BTreeSet::from_iter(first_output_amounts)
                );

                for (cashnote, _) in second_output_cashnotes.iter() {
                    let (main_key, _, _) = second_output_key_map
                        .get(&cashnote.unique_pubkey())
                        .unwrap();
                    let cashnote_confirm_result = cashnote.verify(main_key);
                    assert!(cashnote_confirm_result.is_ok());
                }

                assert_eq!(
                    second_output_cashnotes
                        .iter()
                        .enumerate()
                        .map(|(idx, _cashnote)| { cashnote_output_amounts[idx] })
                        .sum::<u64>(),
                    output_total_amount
                );
                Ok(())
            }
            Err(err) => check_error(err),
        }
    }
}
