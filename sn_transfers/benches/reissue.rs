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
    rand::{CryptoRng, RngCore},
    random_derivation_index, rng, CashNote, FeeOutput, Hash, MainSecretKey, NanoTokens, Output,
    Result, SignedSpend, Transaction, UniquePubkey,
};
use std::collections::{BTreeMap, BTreeSet};

const N_OUTPUTS: u64 = 100;

fn bench_reissue_1_to_100(c: &mut Criterion) {
    let mut rng = rng::from_seed([0u8; 32]);

    let (starting_cashnote, starting_main_key) =
        generate_cashnote_of_value(NanoTokens::from(N_OUTPUTS), &mut rng).unwrap();

    let derived_key = starting_cashnote.derived_key(&starting_main_key).unwrap();
    let cashnote_builder = sn_transfers::TransactionBuilder::default()
        .add_input_cashnote(&starting_cashnote, &derived_key)
        .unwrap()
        .add_outputs((0..N_OUTPUTS).map(|_| {
            let main_key = MainSecretKey::random_from_rng(&mut rng);
            (
                NanoTokens::from(1),
                main_key.main_pubkey(),
                random_derivation_index(&mut rng),
            )
        }))
        .build(Hash::default())
        .unwrap();

    let spent_tx = &cashnote_builder.spent_tx;
    let mut spentbook_node = BTreeMap::new();
    for signed_spend in cashnote_builder.signed_spends() {
        if spentbook_node
            .insert(signed_spend.unique_pubkey(), signed_spend)
            .is_some()
        {
            panic!("cashnote double spend");
        };
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

    let (starting_cashnote, starting_main_key) =
        generate_cashnote_of_value(NanoTokens::from(N_OUTPUTS), &mut rng).unwrap();

    let outputs: BTreeMap<_, _> = (0..N_OUTPUTS)
        .map(|_| {
            let main_key = MainSecretKey::random_from_rng(&mut rng);
            let derivation_index = random_derivation_index(&mut rng);
            let unique_pubkey = main_key.derive_key(&derivation_index).unique_pubkey();
            (
                unique_pubkey,
                (main_key, derivation_index, NanoTokens::from(1)),
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
                .map(|(_, (main_key, derivation_index, value))| {
                    (*value, main_key.main_pubkey(), *derivation_index)
                }),
        )
        .build(Hash::default())
        .unwrap();

    let mut spentbook_node: BTreeMap<UniquePubkey, SignedSpend> = BTreeMap::new();
    let signed_spends: Vec<SignedSpend> = cashnote_builder
        .signed_spends()
        .into_iter()
        .cloned()
        .collect();
    for signed_spend in signed_spends {
        if spentbook_node
            .insert(*signed_spend.unique_pubkey(), signed_spend)
            .is_some()
        {
            panic!("cashnote double spend");
        };
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
            NanoTokens::from(N_OUTPUTS),
            main_key.main_pubkey(),
            derivation_index,
        )
        .build(Hash::default())
        .unwrap();

    let merge_spent_tx = merge_cashnote_builder.spent_tx.clone();
    for signed_spend in merge_cashnote_builder.signed_spends() {
        if spentbook_node
            .insert(*signed_spend.unique_pubkey(), signed_spend.to_owned())
            .is_some()
        {
            panic!("cashnote double spend");
        };
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
    amount: NanoTokens,
    mut rng: &mut (impl RngCore + CryptoRng),
) -> Result<(CashNote, MainSecretKey)> {
    let main_key = MainSecretKey::random_from_rng(&mut rng);
    let derivation_index = random_derivation_index(&mut rng);
    let derived_key = main_key.derive_key(&derivation_index);

    let tx = Transaction {
        inputs: vec![],
        outputs: vec![Output {
            unique_pubkey: derived_key.unique_pubkey(),
            amount,
        }],
        fee: FeeOutput::default(),
    };

    let cashnote = CashNote {
        id: derived_key.unique_pubkey(),
        src_tx: tx,
        signed_spends: Default::default(),
        main_pubkey: main_key.main_pubkey(),
        derivation_index,
    };

    Ok((cashnote, main_key))
}

criterion_group! {
    name = reissue;
    config = Criterion::default().sample_size(10);
    targets = bench_reissue_1_to_100, bench_reissue_100_to_1
}

criterion_main!(reissue);
