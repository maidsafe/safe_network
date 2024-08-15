use sn_client::acc_packet::user_secret::account_wallet_secret_key;
use sn_client::transfers::MainSecretKey;

pub fn generate_mnemonic() -> eyre::Result<bip39::Mnemonic> {
    sn_client::acc_packet::user_secret::random_eip2333_mnemonic().into()
}

pub fn main_sk_from_mnemonic(
    mnemonic: bip39::Mnemonic,
    derivation_passphrase: &str,
) -> eyre::Result<MainSecretKey> {
    account_wallet_secret_key(mnemonic, derivation_passphrase).into()
}
