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

    assert_eq!(dag.verify(&genesis), Ok(BTreeSet::new()));
    Ok(())
}

#[test]
fn test_spend_dag_double_spend_detection() -> Result<()> {
    let mut net = MockNetwork::genesis()?;
    let genesis = net.genesis_spend;

    let owner1 = net.new_pk_with_balance(100)?;
    let owner2a = net.new_pk_with_balance(0)?;
    let owner2b = net.new_pk_with_balance(0)?;

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

    let mut dag = SpendDag::new(genesis);
    for spend in net.spends {
        dag.insert(spend.address(), spend.clone());
    }
    dag.record_faults(&genesis)?;

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
    Ok(())
}
