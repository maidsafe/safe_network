// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod setup;

use std::collections::BTreeSet;

use setup::MockNetwork;

use eyre::Result;
use sn_transfers::SpendAddress;

use crate::{SpendDag, SpendFault};

#[test]
fn test_spend_dag_verify_valid_simple() -> Result<()> {
    let mut net = MockNetwork::genesis()?;
    let genesis = net.genesis_spend;

    let owner1 = net.new_pk_with_balance(100)?;
    let owner2 = net.new_pk_with_balance(0)?;
    let owner3 = net.new_pk_with_balance(0)?;
    let owner4 = net.new_pk_with_balance(0)?;
    let owner5 = net.new_pk_with_balance(0)?;
    let owner6 = net.new_pk_with_balance(0)?;

    net.send(&owner1, &owner2, 100)?;
    net.send(&owner2, &owner3, 100)?;
    net.send(&owner3, &owner4, 100)?;
    net.send(&owner4, &owner5, 100)?;
    net.send(&owner5, &owner6, 100)?;

    let mut dag = SpendDag::new(genesis);
    for spend in net.spends {
        dag.insert(spend.address(), spend.clone());
    }
    assert!(dag.record_faults(&genesis).is_ok());
    // dag.dump_to_file("/tmp/test_spend_dag_verify_valid_simple")?;

    assert_eq!(dag.verify(&genesis), Ok(BTreeSet::new()));
    Ok(())
}

#[test]
fn test_spend_dag_double_spend_poisonning() -> Result<()> {
    let mut net = MockNetwork::genesis()?;
    let genesis = net.genesis_spend;

    let owner1 = net.new_pk_with_balance(100)?;
    let owner2 = net.new_pk_with_balance(0)?;
    let owner3 = net.new_pk_with_balance(0)?;
    let owner4 = net.new_pk_with_balance(0)?;
    let owner5 = net.new_pk_with_balance(0)?;
    let owner6 = net.new_pk_with_balance(0)?;
    let owner_cheat = net.new_pk_with_balance(0)?;

    // spend normaly and save a cashnote to reuse later
    net.send(&owner1, &owner2, 100)?;
    let cn_to_reuse_later = net
        .wallets
        .get(&owner2)
        .expect("owner2 wallet to exist")
        .cn
        .clone();
    let spend1 = net.send(&owner2, &owner3, 100)?;
    let spend_ko3 = net.send(&owner3, &owner4, 100)?;
    let spend_ok4 = net.send(&owner4, &owner5, 100)?;
    let spend_ok5 = net.send(&owner5, &owner6, 100)?;

    // reuse that cashnote to perform a double spend far back in history
    net.wallets
        .get_mut(&owner2)
        .expect("owner2 wallet to still exist")
        .cn = cn_to_reuse_later;
    let spend2 = net.send(&owner2, &owner_cheat, 100)?;

    // create dag
    let mut dag = SpendDag::new(genesis);
    for spend in net.spends {
        dag.insert(spend.address(), spend.clone());
    }
    assert!(dag.record_faults(&genesis).is_ok());
    // dag.dump_to_file("/tmp/test_spend_dag_double_spend_poisonning")?;

    // make sure double spend is detected
    assert_eq!(spend1, spend2, "both spends should be at the same address");
    let double_spent = spend1.first().expect("spend1 to have an element");
    let got = dag.get_spend_faults(double_spent);
    let expected = BTreeSet::from_iter([SpendFault::DoubleSpend(*double_spent)]);
    assert_eq!(got, expected, "DAG should have detected double spend");

    // make sure the double spend's direct descendants are unspendable
    let upk = net
        .wallets
        .get(&owner_cheat)
        .expect("owner_cheat wallet to exist")
        .cn
        .first()
        .expect("owner_cheat wallet to have 1 cashnote")
        .unique_pubkey();
    let utxo = SpendAddress::from_unique_pubkey(&upk);
    let got = dag.get_spend_faults(&utxo);
    let expected = BTreeSet::from_iter([SpendFault::DoubleSpentAncestor {
        addr: utxo,
        ancestor: *double_spent,
    }]);
    assert_eq!(got, expected, "UTXO of double spend should be unspendable");
    let s3 = spend_ko3.first().expect("spend_ko3 to have an element");
    let got = dag.get_spend_faults(s3);
    let expected = BTreeSet::from_iter([SpendFault::DoubleSpentAncestor {
        addr: *s3,
        ancestor: *double_spent,
    }]);
    assert_eq!(got, expected, "spend_ko3 should be unspendable");

    // make sure this didn't affect the rest of the DAG
    let s4 = spend_ok4.first().expect("spend_ok4 to be unique");
    let s5 = spend_ok5.first().expect("spend_ok5 to be unique");

    assert_eq!(dag.get_spend_faults(s4), BTreeSet::new());
    assert_eq!(dag.get_spend_faults(s5), BTreeSet::new());
    Ok(())
}

#[test]
fn test_spend_dag_double_spend_detection() -> Result<()> {
    let mut net = MockNetwork::genesis()?;
    let genesis = net.genesis_spend;

    let owner1 = net.new_pk_with_balance(100)?;
    let owner2a = net.new_pk_with_balance(0)?;
    let owner2b = net.new_pk_with_balance(0)?;

    // perform double spend
    let cn_to_reuse = net
        .wallets
        .get(&owner1)
        .expect("owner1 wallet to exist")
        .cn
        .clone();
    let spend1_addr = net.send(&owner1, &owner2a, 100)?;
    net.wallets
        .get_mut(&owner1)
        .expect("owner1 wallet to still exist")
        .cn = cn_to_reuse;
    let spend2_addr = net.send(&owner1, &owner2b, 100)?;

    // get the UTXOs of the two spends
    let upk_of_2a = net
        .wallets
        .get(&owner2a)
        .expect("owner2a wallet to exist")
        .cn
        .first()
        .expect("owner2a wallet to have 1 cashnote")
        .unique_pubkey();
    let utxo_of_2a = SpendAddress::from_unique_pubkey(&upk_of_2a);
    let upk_of_2b = net
        .wallets
        .get(&owner2b)
        .expect("owner2b wallet to exist")
        .cn
        .first()
        .expect("owner2b wallet to have 1 cashnote")
        .unique_pubkey();
    let utxo_of_2b = SpendAddress::from_unique_pubkey(&upk_of_2b);

    // make DAG
    let mut dag = SpendDag::new(genesis);
    for spend in net.spends {
        dag.insert(spend.address(), spend.clone());
    }
    dag.record_faults(&genesis)?;
    // dag.dump_to_file("/tmp/test_spend_dag_double_spend_detection")?;

    // make sure the double spend is detected
    assert_eq!(
        spend1_addr, spend2_addr,
        "both spends should be at the same address"
    );
    assert_eq!(spend1_addr.len(), 1, "there should only be one spend");
    let double_spent = spend1_addr.first().expect("spend1_addr to have an element");
    let expected = BTreeSet::from_iter([SpendFault::DoubleSpend(*double_spent)]);
    assert_eq!(
        dag.get_spend_faults(double_spent),
        expected,
        "DAG should have detected double spend"
    );

    // make sure the UTXOs of the double spend are unspendable
    let got = dag.get_spend_faults(&utxo_of_2a);
    let expected = BTreeSet::from_iter([SpendFault::DoubleSpentAncestor {
        addr: utxo_of_2a,
        ancestor: *double_spent,
    }]);
    assert_eq!(
        got, expected,
        "UTXO a of double spend should be unspendable"
    );

    let got = dag.get_spend_faults(&utxo_of_2b);
    let expected = BTreeSet::from_iter([SpendFault::DoubleSpentAncestor {
        addr: utxo_of_2b,
        ancestor: *double_spent,
    }]);
    assert_eq!(
        got, expected,
        "UTXO b of double spend should be unspendable"
    );
    Ok(())
}

#[test]
fn test_spend_dag_missing_ancestry() -> Result<()> {
    let mut net = MockNetwork::genesis()?;
    let genesis = net.genesis_spend;

    let owner1 = net.new_pk_with_balance(100)?;
    let owner2 = net.new_pk_with_balance(0)?;
    let owner3 = net.new_pk_with_balance(0)?;
    let owner4 = net.new_pk_with_balance(0)?;
    let owner5 = net.new_pk_with_balance(0)?;
    let owner6 = net.new_pk_with_balance(0)?;

    net.send(&owner1, &owner2, 100)?;
    net.send(&owner2, &owner3, 100)?;
    let spend_missing = net
        .send(&owner3, &owner4, 100)?
        .first()
        .expect("spend_missing should have 1 element")
        .to_owned();
    let spent_after1 = net
        .send(&owner4, &owner5, 100)?
        .first()
        .expect("spent_after1 should have 1 element")
        .to_owned();
    let spent_after2 = net
        .send(&owner5, &owner6, 100)?
        .first()
        .expect("spent_after2 should have 1 element")
        .to_owned();
    let utxo_after3 = net
        .wallets
        .get(&owner6)
        .expect("owner6 wallet to exist")
        .cn
        .first()
        .expect("owner6 wallet to have 1 cashnote")
        .unique_pubkey();
    let utxo_addr = SpendAddress::from_unique_pubkey(&utxo_after3);

    // create dag with one missing spend
    let net_spends = net
        .spends
        .into_iter()
        .filter(|s| spend_missing != s.address());
    let mut dag = SpendDag::new(genesis);
    for spend in net_spends {
        dag.insert(spend.address(), spend.clone());
    }
    dag.record_faults(&genesis)?;
    // dag.dump_to_file("/tmp/test_spend_dag_missing_ancestry")?;

    // make sure the missing spend makes its descendants invalid
    let got = dag.get_spend_faults(&spent_after1);
    let expected = BTreeSet::from_iter([SpendFault::MissingAncestry {
        addr: spent_after1,
        ancestor: spend_missing,
    }]);
    assert_eq!(got, expected, "DAG should have detected missing ancestry");

    let got = dag.get_spend_faults(&spent_after2);
    let expected = BTreeSet::from_iter([SpendFault::PoisonedAncestry(
        spent_after2,
        format!("missing ancestor at: {spend_missing:?}"),
    )]);
    assert_eq!(
        got, expected,
        "DAG should have propagated the error to descendants"
    );

    let got = dag.get_spend_faults(&utxo_addr);
    let expected = BTreeSet::from_iter([SpendFault::PoisonedAncestry(
        utxo_addr,
        format!("missing ancestor at: {spend_missing:?}"),
    )]);
    assert_eq!(
        got, expected,
        "DAG should have propagated the error all the way to descendant utxos"
    );
    Ok(())
}

#[test]
fn test_spend_dag_orphans() -> Result<()> {
    let mut net = MockNetwork::genesis()?;
    let genesis = net.genesis_spend;

    let owner1 = net.new_pk_with_balance(100)?;
    let owner2 = net.new_pk_with_balance(0)?;
    let owner3 = net.new_pk_with_balance(0)?;
    let owner4 = net.new_pk_with_balance(0)?;
    let owner5 = net.new_pk_with_balance(0)?;
    let owner6 = net.new_pk_with_balance(0)?;

    net.send(&owner1, &owner2, 100)?;
    net.send(&owner2, &owner3, 100)?;
    let spend_missing1 = net
        .send(&owner3, &owner4, 100)?
        .first()
        .expect("spend_missing should have 1 element")
        .to_owned();
    let spend_missing2 = net
        .send(&owner4, &owner5, 100)?
        .first()
        .expect("spend_missing2 should have 1 element")
        .to_owned();
    let spent_after1 = net
        .send(&owner5, &owner6, 100)?
        .first()
        .expect("spent_after1 should have 1 element")
        .to_owned();
    let utxo_after2 = net
        .wallets
        .get(&owner6)
        .expect("owner6 wallet to exist")
        .cn
        .first()
        .expect("owner6 wallet to have 1 cashnote")
        .unique_pubkey();
    let utxo_addr = SpendAddress::from_unique_pubkey(&utxo_after2);

    // create dag with two missing spends in the chain
    let net_spends = net
        .spends
        .into_iter()
        .filter(|s| spend_missing1 != s.address() && spend_missing2 != s.address());
    let mut dag = SpendDag::new(genesis);
    for spend in net_spends {
        dag.insert(spend.address(), spend.clone());
    }
    dag.record_faults(&genesis)?;
    // dag.dump_to_file("/tmp/test_spend_dag_orphans")?;

    // make sure the spends after the two missing spends are orphans
    let got = dag.get_spend_faults(&spent_after1);
    let expected = BTreeSet::from_iter([
        SpendFault::OrphanSpend {
            addr: spent_after1,
            src: dag.source(),
        },
        SpendFault::MissingAncestry {
            addr: spent_after1,
            ancestor: spend_missing2,
        },
    ]);
    assert_eq!(got, expected, "DAG should have detected orphan spend");

    let got = dag.get_spend_faults(&utxo_addr);
    let expected = SpendFault::OrphanSpend {
        addr: utxo_addr,
        src: dag.source(),
    };
    assert!(
        got.contains(&expected),
        "Utxo of orphan spend should also be an orphan"
    );
    Ok(())
}
