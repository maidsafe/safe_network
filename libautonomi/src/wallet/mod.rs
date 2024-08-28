pub mod error;

use crate::wallet::error::WalletError;
use sn_client::transfers::{HotWallet, MainSecretKey};
use sn_transfers::{
    CashNote, CashNoteRedemption, DerivationIndex, DerivedSecretKey, MainPubkey, NanoTokens,
    OfflineTransfer, SignedSpend, SpendReason, Transfer, UniquePubkey,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct MemWallet {
    hot_wallet: HotWallet,
    available_cash_notes: BTreeMap<UniquePubkey, CashNote>,
}

impl MemWallet {
    /// Create an empty wallet from a main secret key.
    fn from_main_secret_key(main_secret_key: MainSecretKey) -> Self {
        Self {
            hot_wallet: HotWallet::new(main_secret_key, PathBuf::default()),
            available_cash_notes: Default::default(),
        }
    }

    // TODO: as WASM can not save a wallet state to disk or load from disk -- we need to provide a wallet state manually.
    /// Initialise a wallet from wallet state bytes containing all payments, (un)confirmed spends, cash notes and the secret key.
    fn from_state_bytes<T: AsRef<[u8]>>(_data: T) -> Self {
        todo!()
    }

    /// Returns the entire wallet state as bytes. That includes all payments (un)confirmed spends, cash notes and the secret key.
    /// A wallet can be fully initialised again from these state bytes.
    fn to_state_bytes(&self) -> Vec<u8> {
        todo!()
    }

    /// Returns the wallet address (main public key).
    pub fn address(&self) -> MainPubkey {
        self.hot_wallet.address()
    }

    /// Returns the balance of a wallet in Nanos.
    pub fn balance(&self) -> NanoTokens {
        self.hot_wallet.balance()
    }

    pub(super) fn unwrap_transfer(
        &self,
        transfer: &Transfer,
    ) -> Result<Vec<CashNoteRedemption>, WalletError> {
        self.hot_wallet
            .unwrap_transfer(transfer)
            .map_err(|_| WalletError::FailedToDecryptTransfer)
    }

    /// Returns all available `CashNotes` together with their secret key to spend them.
    pub(super) fn cash_notes_with_secret_keys(
        &mut self,
    ) -> Vec<(CashNote, Option<DerivedSecretKey>)> {
        self.available_cash_notes
            .values()
            .map(|cn| (cn.clone(), cn.derived_key(self.hot_wallet.key()).ok()))
            .collect()
    }

    pub(super) fn create_offline_transfer(
        &mut self,
        outputs: Vec<(NanoTokens, MainPubkey)>,
        reason: Option<SpendReason>,
    ) -> Result<OfflineTransfer, WalletError> {
        for output in &outputs {
            if output.0.is_zero() {
                return Err(WalletError::TransferAmountZero);
            }
        }

        let mut rng = &mut rand::rngs::OsRng;

        // create a unique key for each output
        let to_unique_keys: Vec<_> = outputs
            .into_iter()
            .map(|(amount, address)| (amount, address, DerivationIndex::random(&mut rng)))
            .collect();

        let cash_notes_with_keys = self.cash_notes_with_secret_keys();

        let reason = reason.unwrap_or_default();

        let transfer =
            OfflineTransfer::new(cash_notes_with_keys, to_unique_keys, self.address(), reason)?;

        Ok(transfer)
    }

    fn mark_cash_notes_as_spent<'a, T: IntoIterator<Item = &'a UniquePubkey>>(
        &mut self,
        unique_pubkeys: T,
    ) {
        let unique_pubkeys: Vec<&'a UniquePubkey> = unique_pubkeys.into_iter().collect();

        for unique_pubkey in &unique_pubkeys {
            let _ = self.available_cash_notes.remove(unique_pubkey);
        }

        self.hot_wallet
            .wo_wallet_mut()
            .mark_notes_as_spent(unique_pubkeys);
    }

    pub(super) fn deposit_cash_note(&mut self, cash_note: CashNote) -> Result<(), WalletError> {
        if cash_note
            .derived_pubkey(&self.hot_wallet.key().main_pubkey())
            .is_err()
        {
            return Err(WalletError::CashNoteNotOwned);
        }

        if cash_note.value().is_err() {
            return Err(WalletError::CashNoteOutputNotFound);
        }

        self.available_cash_notes
            .insert(cash_note.unique_pubkey, cash_note.clone());

        // DevNote: the deposit fn already does the checks above,
        // but I have added them here just in case we get rid
        // of the composited hotwallet and its deposit checks
        self.hot_wallet
            .wo_wallet_mut()
            .deposit(&[cash_note])
            .map_err(|_| WalletError::CashNoteOutputNotFound)?;

        Ok(())
    }

    pub(super) fn add_pending_spend(&mut self, spend: SignedSpend) {
        self.hot_wallet
            .unconfirmed_spend_requests_mut()
            .insert(spend);
    }

    // TODO: should we verify if the transfer is valid and destined for this wallet?
    pub(super) fn process_offline_transfer(&mut self, transfer: OfflineTransfer) {
        let spent_unique_pubkeys: BTreeSet<_> = transfer
            .tx
            .inputs
            .iter()
            .map(|input| input.unique_pubkey())
            .collect();

        self.mark_cash_notes_as_spent(spent_unique_pubkeys);

        if let Some(cash_note) = transfer.change_cash_note {
            let _ = self.deposit_cash_note(cash_note);
        }
    }
}
