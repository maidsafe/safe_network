// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod audit;
pub(crate) mod helpers;
pub(crate) mod hot_wallet;
pub(crate) mod wo_wallet;

use sn_client::transfers::{CashNote, HotWallet, MainPubkey, NanoTokens, WatchOnlyWallet};

use color_eyre::Result;
use std::{collections::BTreeSet, io::Read, path::Path};

// TODO: convert this into a Trait part of the wallet APIs.
enum WalletApiHelper {
    WatchOnlyWallet(WatchOnlyWallet),
    HotWallet(HotWallet),
}

impl WalletApiHelper {
    pub fn watch_only_from_pk(main_pk: MainPubkey, root_dir: &Path) -> Result<Self> {
        let wallet = watch_only_wallet_from_pk(main_pk, root_dir)?;
        Ok(Self::WatchOnlyWallet(wallet))
    }

    pub fn load_from(root_dir: &Path) -> Result<Self> {
        let wallet = HotWallet::load_from(root_dir)?;
        Ok(Self::HotWallet(wallet))
    }

    pub fn address(&self) -> MainPubkey {
        match self {
            Self::WatchOnlyWallet(w) => w.address(),
            Self::HotWallet(w) => w.address(),
        }
    }

    pub fn balance(&self) -> NanoTokens {
        match self {
            Self::WatchOnlyWallet(w) => w.balance(),
            Self::HotWallet(w) => w.balance(),
        }
    }

    pub fn read_cash_note_from_stdin(&mut self) -> Result<()> {
        println!("Please paste your CashNote below:");
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        self.deposit_from_cash_note_hex(&input)
    }

    pub fn deposit_from_cash_note_hex(&mut self, input: &str) -> Result<()> {
        let cash_note = CashNote::from_hex(input.trim())?;

        let old_balance = self.balance();
        let cash_notes = vec![cash_note.clone()];

        let spent_unique_pubkeys: BTreeSet<_> = cash_note
            .parent_tx
            .inputs
            .iter()
            .map(|input| input.unique_pubkey())
            .collect();

        match self {
            Self::WatchOnlyWallet(w) => {
                w.mark_notes_as_spent(spent_unique_pubkeys);
                w.deposit_and_store_to_disk(&cash_notes)?
            }
            Self::HotWallet(w) => {
                w.mark_notes_as_spent(spent_unique_pubkeys);
                w.deposit_and_store_to_disk(&cash_notes)?
            }
        }
        let new_balance = self.balance();
        println!("Successfully stored cash_note to wallet dir. \nOld balance: {old_balance}\nNew balance: {new_balance}");

        Ok(())
    }

    pub fn deposit(&mut self, read_from_stdin: bool, cash_note: Option<&str>) -> Result<()> {
        if read_from_stdin {
            return self.read_cash_note_from_stdin();
        }

        if let Some(cash_note_hex) = cash_note {
            return self.deposit_from_cash_note_hex(cash_note_hex);
        }

        let previous_balance = self.balance();

        self.try_load_cash_notes()?;

        let deposited = NanoTokens::from(self.balance().as_nano() - previous_balance.as_nano());
        if deposited.is_zero() {
            println!("Nothing deposited.");
        } else if let Err(err) = self.deposit_and_store_to_disk(&vec![]) {
            println!("Failed to store deposited ({deposited}) amount: {err:?}");
        } else {
            println!("Deposited {deposited}.");
        }

        Ok(())
    }

    fn deposit_and_store_to_disk(&mut self, cash_notes: &Vec<CashNote>) -> Result<()> {
        match self {
            Self::WatchOnlyWallet(w) => w.deposit_and_store_to_disk(cash_notes)?,
            Self::HotWallet(w) => w.deposit_and_store_to_disk(cash_notes)?,
        }
        Ok(())
    }

    fn try_load_cash_notes(&mut self) -> Result<()> {
        match self {
            Self::WatchOnlyWallet(w) => w.try_load_cash_notes()?,
            Self::HotWallet(w) => w.try_load_cash_notes()?,
        }
        Ok(())
    }
}

fn watch_only_wallet_from_pk(main_pk: MainPubkey, root_dir: &Path) -> Result<WatchOnlyWallet> {
    let pk_hex = main_pk.to_hex();
    let folder_name = format!("pk_{}_{}", &pk_hex[..6], &pk_hex[pk_hex.len() - 6..]);
    let wallet_dir = root_dir.join(folder_name);
    println!(
        "Loading watch-only local wallet from: {}",
        wallet_dir.display()
    );
    let wallet = WatchOnlyWallet::load_from(&wallet_dir, main_pk)?;
    Ok(wallet)
}
