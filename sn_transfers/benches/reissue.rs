// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(clippy::from_iter_instead_of_collect)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sn_transfers::{
    mock,
    rand::{CryptoRng, RngCore},
    random_derivation_index, rng, CashNote, Hash, MainSecretKey, Nano, Result,
};
use std::collections::{BTreeMap, BTreeSet};

const N_OUTPUTS: u64 = 100;

fn bench_reissue_1_to_100(c: &mut Criterion) {
    let mut rng = rng::from_seed([0u8; 32]);

    let (mut spentbook_node, (starting_cashnote, starting_main_key)) =
        generate_cashnote_of_value(Nano::from_nano(N_OUTPUTS), &mut rng).unwrap();

    let derived_key = starting_cashnote.derived_key(&starting_main_key).unwrap();
    let cashnote_builder = sn_transfers::TransactionBuilder::default()
        .add_input_cashnote(&starting_cashnote, &derived_key)
        .unwrap()
        .add_outputs((0..N_OUTPUTS).map(|_| {
            let main_key = MainSecretKey::random_from_rng(&mut rng);
            (
                Nano::from_nano(1),
                main_key.main_pubkey(),
                random_derivation_index(&mut rng),
            )
        }))
        .build(Hash::default())
        .unwrap();

    let spent_tx = &cashnote_builder.spent_tx;
    for signed_spend in cashnote_builder.signed_spends() {
        spentbook_node.log_spent(spent_tx, signed_spend).unwrap();
    }

    let signed_spends: BTreeSet<_> = cashnote_builder
        .signed_spends()
        .into_iter()
        .cloned()
        .collect();

    c.bench_function(&format!("reissue split 1 to {N_OUTPUTS}"), |b| {
        #[cfg(unix)]
        let guard = pprof::ProfilerGuard::new(100).unwrap();

        b.iter(|| {
            black_box(spent_tx)
                .verify_against_inputs_spent(&signed_spends)
                .unwrap();
        });

        #[cfg(unix)]
        if let Ok(report) = guard.report().build() {
            let file =
                std::fs::File::create(format!("reissue_split_1_to_{N_OUTPUTS}.svg")).unwrap();
            report.flamegraph(file).unwrap();
        };
    });
}

fn bench_reissue_100_to_1(c: &mut Criterion) {
    let mut rng = rng::from_seed([0u8; 32]);

    let (mut spentbook_node, (starting_cashnote, starting_main_key)) =
        generate_cashnote_of_value(Nano::from_nano(N_OUTPUTS), &mut rng).unwrap();

    let outputs: BTreeMap<_, _> = (0..N_OUTPUTS)
        .map(|_| {
            let main_key = MainSecretKey::random_from_rng(&mut rng);
            let derivation_index = random_derivation_index(&mut rng);
            let unique_pubkey = main_key.derive_key(&derivation_index).unique_pubkey();
            (
                unique_pubkey,
                (main_key, derivation_index, Nano::from_nano(1)),
            )
        })
        .collect();

    let derived_key = starting_cashnote.derived_key(&starting_main_key).unwrap();
    let cashnote_builder = sn_transfers::TransactionBuilder::default()
        .add_input_cashnote(&starting_cashnote, &derived_key)
        .unwrap()
        .add_outputs(
            outputs
                .iter()
                .map(|(_, (main_key, derivation_index, token))| {
                    (*token, main_key.main_pubkey(), *derivation_index)
                }),
        )
        .build(Hash::default())
        .unwrap();

    let spent_tx = cashnote_builder.spent_tx.clone();
    for signed_spend in cashnote_builder.signed_spends() {
        spentbook_node.log_spent(&spent_tx, signed_spend).unwrap();
    }
    let cashnotes = cashnote_builder.build().unwrap();

    let main_key = MainSecretKey::random_from_rng(&mut rng);
    let derivation_index = random_derivation_index(&mut rng);

    let mut tx_builder = sn_transfers::TransactionBuilder::default();

    for (cashnote, _) in cashnotes.into_iter() {
        let (main_key, _, _) = outputs.get(&cashnote.unique_pubkey()).unwrap();
        let derived_key = cashnote.derived_key(main_key).unwrap();
        tx_builder = tx_builder
            .add_input_cashnote(&cashnote, &derived_key)
            .unwrap();
    }

    let merge_cashnote_builder = tx_builder
        .add_output(
            Nano::from_nano(N_OUTPUTS),
            main_key.main_pubkey(),
            derivation_index,
        )
        .build(Hash::default())
        .unwrap();

    let merge_spent_tx = merge_cashnote_builder.spent_tx.clone();
    for signed_spend in merge_cashnote_builder.signed_spends() {
        spentbook_node
            .log_spent(&merge_spent_tx, signed_spend)
            .unwrap();
    }

    let signed_spends: BTreeSet<_> = merge_cashnote_builder
        .signed_spends()
        .into_iter()
        .cloned()
        .collect();

    c.bench_function(&format!("reissue merge {N_OUTPUTS} to 1"), |b| {
        #[cfg(unix)]
        let guard = pprof::ProfilerGuard::new(100).unwrap();

        b.iter(|| {
            black_box(&merge_spent_tx)
                .verify_against_inputs_spent(&signed_spends)
                .unwrap();
        });

        #[cfg(unix)]
        if let Ok(report) = guard.report().build() {
            let file =
                std::fs::File::create(format!("reissue_merge_{N_OUTPUTS}_to_1.svg")).unwrap();
            report.flamegraph(file).unwrap();
        };
    });
}

#[allow(clippy::result_large_err)]
fn generate_cashnote_of_value(
    token: Nano,
    rng: &mut (impl RngCore + CryptoRng),
) -> Result<(mock::SpentbookNode, (CashNote, MainSecretKey))> {
    let (mut spentbook_node, genesis_cashnote, genesis_material, __token) =
        mock::GenesisBuilder::init_genesis_single()?;

    let output_tokens = vec![
        token,
        Nano::from_nano(mock::GenesisMaterial::GENESIS_AMOUNT - token.as_nano()),
    ];

    let main_key = MainSecretKey::random_from_rng(rng);

    let derived_key = genesis_cashnote
        .derived_key(&genesis_material.main_key)
        .unwrap();
    let cashnote_builder = sn_transfers::TransactionBuilder::default()
        .add_input_cashnote(&genesis_cashnote, &derived_key)
        .unwrap()
        .add_outputs(
            output_tokens
                .into_iter()
                .map(|token| (token, main_key.main_pubkey(), random_derivation_index(rng))),
        )
        .build(Hash::default())?;

    let tx = cashnote_builder.spent_tx.clone();
    for signed_spend in cashnote_builder.signed_spends() {
        spentbook_node.log_spent(&tx, signed_spend)?;
    }

    let (starting_cashnote, _) = cashnote_builder.build()?.into_iter().next().unwrap();

    Ok((spentbook_node, (starting_cashnote, main_key)))
}

criterion_group! {
    name = reissue;
    config = Criterion::default().sample_size(10);
    targets = bench_reissue_1_to_100, bench_reissue_100_to_1
}

criterion_main!(reissue);
