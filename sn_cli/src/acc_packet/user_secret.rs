// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::SecretKey;
use color_eyre::{eyre::eyre, Result};
use curv::elliptic::curves::ECScalar;
use rand::RngCore;
use sn_client::transfers::MainSecretKey;
use std::path::Path;
use xor_name::XorName;

const MNEMONIC_FILENAME: &str = "account_secret";

const ACCOUNT_ROOT_XORNAME_DERIVATION: &str = "m/1/0";

const ACCOUNT_WALLET_DERIVATION: &str = "m/2/0";

pub fn random_eip2333_mnemonic() -> Result<bip39::Mnemonic> {
    let mut entropy = [1u8; 32];
    let rng = &mut rand::rngs::OsRng;
    rng.fill_bytes(&mut entropy);
    let mnemonic = bip39::Mnemonic::from_entropy(&entropy)?;
    Ok(mnemonic)
}

/// Derive a wallet secret key from the mnemonic for the account.
pub fn account_wallet_secret_key(
    mnemonic: bip39::Mnemonic,
    passphrase: &str,
) -> Result<MainSecretKey> {
    let seed = mnemonic.to_seed(passphrase);

    let root_sk = eip2333::derive_master_sk(&seed)
        .map_err(|err| eyre!("Invalid seed from mnemonic: {err}"))?;
    let derived_key = eip2333::derive_child_sk(root_sk, ACCOUNT_WALLET_DERIVATION);
    let key_bytes = derived_key.serialize();
    let sk =
        SecretKey::from_bytes(key_bytes.into()).map_err(|err| eyre!("Invalid key bytes: {err}"))?;
    Ok(MainSecretKey::new(sk))
}

#[allow(dead_code)] // as yet unused, will be used soon
/// Derive an xorname from the mnemonic for the account to store data.
pub(crate) fn account_root_xorname(mnemonic: bip39::Mnemonic, passphrase: &str) -> Result<XorName> {
    let seed = mnemonic.to_seed(passphrase);

    let root_sk = eip2333::derive_master_sk(&seed)
        .map_err(|err| eyre!("Invalid seed from mnemonic: {err}"))?;
    let derived_key = eip2333::derive_child_sk(root_sk, ACCOUNT_ROOT_XORNAME_DERIVATION);
    let derived_key_bytes = derived_key.serialize();
    Ok(XorName::from_content(&derived_key_bytes))
}

pub fn write_mnemonic_to_disk(files_dir: &Path, mnemonic: &bip39::Mnemonic) -> Result<()> {
    let filename = files_dir.join(MNEMONIC_FILENAME);
    let content = mnemonic.to_string();
    std::fs::write(filename, content)?;
    Ok(())
}

#[allow(dead_code)] // as yet unused, will be used soon
pub(super) fn read_mnemonic_from_disk(files_dir: &Path) -> Result<bip39::Mnemonic> {
    let filename = files_dir.join(MNEMONIC_FILENAME);
    let content = std::fs::read_to_string(filename)
        .map_err(|err| eyre!("Error reading account secret: {err}"))?;
    let mnemonic = bip39::Mnemonic::parse_normalized(&content)
        .map_err(|err| eyre!("Error parsing account secret: {err}"))?;
    Ok(mnemonic)
}
