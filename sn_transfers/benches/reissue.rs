// Copyright 2024 MaidSafe.net limited.

// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(clippy::from_iter_instead_of_collect, clippy::unwrap_used)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sn_transfers::{
    create_first_cash_note_from_key, rng, CashNote, DerivationIndex, MainSecretKey, NanoTokens,
    SignedTransaction, SpendReason,
};
use std::collections::BTreeSet;

const N_OUTPUTS: u64 = 100;

fn bench_reissue_1_to_100(c: &mut Criterion) {
    // prepare transfer of genesis cashnote
    let mut rng = rng::from_seed([0u8; 32]);
    let (starting_cashnote, starting_main_key) = generate_cashnote();
    let main_pubkey = starting_main_key.main_pubkey();
    let recipients = (0..N_OUTPUTS)
        .map(|_| {
            (
                NanoTokens::from(1),
                main_pubkey,
                DerivationIndex::random(&mut rng),
                false,
            )
        })
        .collect::<Vec<_>>();

    // transfer to N_OUTPUTS recipients
    let signed_tx = SignedTransaction::new(
        vec![starting_cashnote],
        recipients,
        starting_main_key.main_pubkey(),
        SpendReason::default(),
        &starting_main_key,
    )
    .expect("Transaction creation to succeed");

    // simulate spentbook to check for double spends
    let mut spentbook_node = BTreeSet::new();
    for spend in &signed_tx.spends {
        if !spentbook_node.insert(*spend.unique_pubkey()) {
            panic!("cashnote double spend");
        };
    }

    // bench verification
    c.bench_function(&format!("reissue split 1 to {N_OUTPUTS}"), |b| {
        #[cfg(unix)]
        let guard = pprof::ProfilerGuard::new(100).unwrap();

        b.iter(|| {
            black_box(&signed_tx).verify().unwrap();
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
    // prepare transfer of genesis cashnote to recipient_of_100_mainkey
    let mut rng = rng::from_seed([0u8; 32]);
    let (starting_cashnote, starting_main_key) = generate_cashnote();
    let recipient_of_100_mainkey = MainSecretKey::random_from_rng(&mut rng);
    let recipients = (0..N_OUTPUTS)
        .map(|_| {
            (
                NanoTokens::from(1),
                recipient_of_100_mainkey.main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            )
        })
        .collect::<Vec<_>>();

    // transfer to N_OUTPUTS recipients derived from recipient_of_100_mainkey
    let signed_tx = SignedTransaction::new(
        vec![starting_cashnote],
        recipients,
        starting_main_key.main_pubkey(),
        SpendReason::default(),
        &starting_main_key,
    )
    .expect("Transaction creation to succeed");

    // simulate spentbook to check for double spends
    let mut spentbook_node = BTreeSet::new();
    let signed_spends: BTreeSet<_> = signed_tx.spends.clone().into_iter().collect();
    for spend in signed_spends.into_iter() {
        if !spentbook_node.insert(*spend.unique_pubkey()) {
            panic!("cashnote double spend");
        };
    }

    // prepare to send all of those cashnotes back to our starting_main_key
    let total_amount = signed_tx
        .output_cashnotes
        .iter()
        .map(|cn| cn.value().as_nano())
        .sum();
    let many_cashnotes = signed_tx.output_cashnotes.into_iter().collect();
    let one_single_recipient = vec![(
        NanoTokens::from(total_amount),
        starting_main_key.main_pubkey(),
        DerivationIndex::random(&mut rng),
        false,
    )];

    // create transfer to merge all of the cashnotes into one
    let many_to_one_tx = SignedTransaction::new(
        many_cashnotes,
        one_single_recipient,
        starting_main_key.main_pubkey(),
        SpendReason::default(),
        &recipient_of_100_mainkey,
    )
    .expect("Many to one Transaction creation to succeed");

    // bench verification
    c.bench_function(&format!("reissue merge {N_OUTPUTS} to 1"), |b| {
        #[cfg(unix)]
        let guard = pprof::ProfilerGuard::new(100).unwrap();

        b.iter(|| {
            black_box(&many_to_one_tx).verify().unwrap();
        });

        #[cfg(unix)]
        if let Ok(report) = guard.report().build() {
            let file =
                std::fs::File::create(format!("reissue_merge_{N_OUTPUTS}_to_1.svg")).unwrap();
            report.flamegraph(file).unwrap();
        };
    });
}

fn generate_cashnote() -> (CashNote, MainSecretKey) {
    let key = MainSecretKey::random();
    let genesis = create_first_cash_note_from_key(&key).expect("Genesis creation to succeed.");
    (genesis, key)
}

criterion_group! {
    name = reissue;
    config = Criterion::default().sample_size(10);
    targets = bench_reissue_1_to_100, bench_reissue_100_to_1
}

criterion_main!(reissue);
