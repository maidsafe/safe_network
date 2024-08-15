use crate::Client;
use sn_client::transfers::{HotWallet, MainSecretKey};
use sn_transfers::{MainPubkey, Transfer};
use std::path::PathBuf;

struct MemWallet {
    hot_wallet: HotWallet,
}

impl MemWallet {
    /// Create an empty wallet from a main secret key.
    fn from_main_secret_key(main_secret_key: MainSecretKey) -> Self {
        Self {
            hot_wallet: HotWallet::new(main_secret_key, PathBuf::new()),
        }
    }

    /// Initialise a wallet from a wallet folder containing all payments, (un)confirmed spends, cash notes and the secret key.
    fn from_wallet_folder() -> Self {
        todo!()
    }

    /// Returns the wallet address (main public key).
    pub fn address(&self) -> MainPubkey {
        self.hot_wallet.address()
    }

    /// Deposits all valid `CashNotes` into the wallet from a transfer.
    pub async fn receive(&mut self, transfer_hex: &str, client: &Client) -> eyre::Result<()> {
        let transfer = Transfer::from_hex(&transfer_hex)?;
        let cash_note_redemptions = self.hot_wallet.unwrap_transfer(&transfer)?;

        let cash_notes = client
            .network
            .verify_cash_notes_redemptions(self.address(), &cash_note_redemptions)
            .await?;

        let mut valid_cash_notes = Vec::new();

        for cash_note in cash_notes {
            match client.verify_if_cash_note_is_valid(&cash_note).await {
                Ok(_) => valid_cash_notes.push(cash_note),
                Err(e) => {
                    tracing::warn!("Error verifying CashNote: {}", e);
                }
            }
        }

        self.hot_wallet.deposit(&valid_cash_notes)?;

        Ok(())
    }
}
