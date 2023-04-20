// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, KeyLessWallet};

use std::path::Path;
use tokio::fs;

// Filename for storing a wallet.
const WALLET_FILENAME: &str = "wallet";

/// Writes the `KeyLessWallet` to the specified path.
pub(super) async fn store_wallet(root_dir: &Path, wallet: &KeyLessWallet) -> Result<()> {
    let wallet_path = root_dir.join(WALLET_FILENAME);
    let bytes = bincode::serialize(&wallet)?;
    fs::write(wallet_path, bytes).await?;
    Ok(())
}

/// Returns `Some(KeyLessWallet)` or None if file doesn't exist.
pub(super) async fn get_wallet(root_dir: &Path) -> Result<Option<KeyLessWallet>> {
    let path = root_dir.join(WALLET_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&path).await?;
    let wallet = bincode::deserialize(&bytes)?;

    Ok(Some(wallet))
}
