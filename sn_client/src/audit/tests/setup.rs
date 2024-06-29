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
    OfflineTransfer, SignedSpend, SpendAddress, SpendReason, GENESIS_CASHNOTE,
    GENESIS_OUTPUT_DERIVATION_INDEX,
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

            let genesis_sk_main_pubkey = get_genesis_sk().main_pubkey();

            println!("Sending {balance} from genesis {genesis_pk:?} to {owner_pk:?}");
            self.send(&genesis_sk_main_pubkey, &owner_pk, balance, false)
                .map_err(|e| eyre!("failed to get money from genesis: {e}"))?;
        }
        Ok(owner_pk)
    }

    pub fn send(
        &mut self,
        from: &MainPubkey,
        to: &MainPubkey,
        amount: u64,
        is_genesis: bool,
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
        let cash_notes_with_keys = from_wallet
            .cn
            .clone()
            .into_iter()
            .map(|cn| Ok((cn.clone(), Some(cn.derived_key(&from_wallet.sk)?))))
            .collect::<Result<_>>()
            .map_err(|e| eyre!("could not get cashnotes for transfer: {e}"))?;

        let derivation_index = if is_genesis {
            GENESIS_OUTPUT_DERIVATION_INDEX
        } else {
            DerivationIndex::random(&mut rng)
        };

        let recipient = vec![(
            NanoTokens::from(amount),
            to_wallet.sk.main_pubkey(),
            derivation_index,
        )];
        let transfer = OfflineTransfer::new(
            cash_notes_with_keys,
            recipient,
            from_wallet.sk.main_pubkey(),
            SpendReason::default(),
        )
        .map_err(|e| eyre!("failed to create transfer: {}", e))?;
        let spends = transfer.all_spend_requests;

        // update wallets
        let mut updated_from_wallet_cns = from_wallet.cn.clone();
        updated_from_wallet_cns.retain(|cn| {
            !spends
                .iter()
                .any(|s| s.unique_pubkey() == &cn.unique_pubkey())
        });
        if let Some(ref change_cn) = transfer.change_cash_note {
            if !updated_from_wallet_cns
                .iter()
                .any(|cn| cn.unique_pubkey() == change_cn.unique_pubkey())
            {
                updated_from_wallet_cns.extend(transfer.change_cash_note);
            }
        }

        self.wallets
            .entry(*from)
            .and_modify(|w| w.cn = updated_from_wallet_cns);
        self.wallets
            .entry(*to)
            .and_modify(|w| w.cn.extend(transfer.cash_notes_for_recipient));

        // update network spends
        let spent_addrs = spends.iter().map(|s| s.address()).collect();
        self.spends.extend(spends);
        Ok(spent_addrs)
    }
}
