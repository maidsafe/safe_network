// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::{BTreeMap, BTreeSet};

use bls::SecretKey;
use eyre::{eyre, Result};
use sn_transfers::{
    get_genesis_sk, CashNote, DerivationIndex, MainPubkey, MainSecretKey, NanoTokens,
    OutputPurpose, SignedSpend, SignedTransaction, SpendAddress, SpendReason, GENESIS_CASHNOTE,
};

pub struct MockWallet {
    pub sk: MainSecretKey,
    pub cn: Vec<CashNote>,
}

pub struct MockNetwork {
    pub genesis_spend: SpendAddress,
    pub spends: BTreeSet<SignedSpend>,
    pub wallets: BTreeMap<MainPubkey, MockWallet>,
}

impl MockNetwork {
    pub fn genesis() -> Result<Self> {
        let mut net = MockNetwork {
            genesis_spend: SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey()),
            spends: BTreeSet::new(),
            wallets: BTreeMap::new(),
        };

        // create genesis wallet
        let genesis_cn = GENESIS_CASHNOTE.clone();
        let genesis_pk = *GENESIS_CASHNOTE.main_pubkey();
        net.wallets.insert(
            genesis_pk,
            MockWallet {
                sk: get_genesis_sk(),
                cn: vec![genesis_cn],
            },
        );

        // spend genesis
        let everything = GENESIS_CASHNOTE.value().as_nano();
        let spent_addrs = net
            .send(&genesis_pk, &genesis_pk, everything)
            .map_err(|e| eyre!("failed to send genesis: {e}"))?;
        net.genesis_spend = match spent_addrs.as_slice() {
            [one] => *one,
            _ => {
                return Err(eyre!(
                    "Expected Genesis spend to be unique but got {spent_addrs:?}"
                ))
            }
        };

        Ok(net)
    }

    pub fn new_pk_with_balance(&mut self, balance: u64) -> Result<MainPubkey> {
        let owner = MainSecretKey::new(SecretKey::random());
        let owner_pk = owner.main_pubkey();
        self.wallets.insert(
            owner_pk,
            MockWallet {
                sk: owner,
                cn: Vec::new(),
            },
        );

        if balance > 0 {
            let genesis_pk = GENESIS_CASHNOTE.main_pubkey();
            println!("Sending {balance} from genesis {genesis_pk:?} to {owner_pk:?}");
            self.send(genesis_pk, &owner_pk, balance)
                .map_err(|e| eyre!("failed to get money from genesis: {e}"))?;
        }
        Ok(owner_pk)
    }

    pub fn send(
        &mut self,
        from: &MainPubkey,
        to: &MainPubkey,
        amount: u64,
    ) -> Result<Vec<SpendAddress>> {
        let mut rng = rand::thread_rng();
        let from_wallet = self
            .wallets
            .get(from)
            .ok_or_else(|| eyre!("from wallet not found: {from:?}"))?;
        let to_wallet = self
            .wallets
            .get(to)
            .ok_or_else(|| eyre!("to wallet not found: {to:?}"))?;

        // perform offline transfer
        let derivation_index = DerivationIndex::random(&mut rng);
        let recipient = vec![(
            NanoTokens::from(amount),
            to_wallet.sk.main_pubkey(),
            derivation_index,
            OutputPurpose::None,
        )];
        let tx = SignedTransaction::new(
            from_wallet.cn.clone(),
            recipient,
            from_wallet.sk.main_pubkey(),
            SpendReason::default(),
            &from_wallet.sk,
        )
        .map_err(|e| eyre!("failed to create transfer: {}", e))?;
        let spends = tx.spends;

        // update wallets
        let mut updated_from_wallet_cns = from_wallet.cn.clone();
        updated_from_wallet_cns.retain(|cn| {
            !spends
                .iter()
                .any(|s| s.unique_pubkey() == &cn.unique_pubkey())
        });
        if let Some(ref change_cn) = tx.change_cashnote {
            if !updated_from_wallet_cns
                .iter()
                .any(|cn| cn.unique_pubkey() == change_cn.unique_pubkey())
            {
                updated_from_wallet_cns.extend(tx.change_cashnote);
            }
        }

        self.wallets
            .entry(*from)
            .and_modify(|w| w.cn = updated_from_wallet_cns);
        self.wallets
            .entry(*to)
            .and_modify(|w| w.cn.extend(tx.output_cashnotes));

        // update network spends
        let spent_addrs = spends.iter().map(|s| s.address()).collect();
        self.spends.extend(spends);
        Ok(spent_addrs)
    }
}
