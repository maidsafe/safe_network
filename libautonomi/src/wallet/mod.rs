use sn_client::transfers::{HotWallet, MainSecretKey};
use std::path::PathBuf;

struct MemWallet {
    hot_wallet: HotWallet,
}

impl MemWallet {
    fn from_main_secret_key(main_secret_key: MainSecretKey) -> Self {
        Self {
            hot_wallet: HotWallet::new(main_secret_key, PathBuf::new()),
        }
    }

    /// Initialise a wallet from a wallet folder containing all payments, (un)confirmed spends, cash notes and the secret key.
    fn from_wallet_folder() -> Self {
        todo!()
    }
}
