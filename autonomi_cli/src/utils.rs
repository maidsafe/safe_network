// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::Multiaddr;
use color_eyre::eyre::eyre;
use color_eyre::eyre::Context;
use color_eyre::Result;
use color_eyre::Section;
use sn_peers_acquisition::PeersArgs;
use std::env;
use std::fs;
use std::path::PathBuf;

use sn_peers_acquisition::SAFE_PEERS_ENV;

// NB TODO: use those as return values for the functions below
// use autonomi::register::RegisterKey;
// use autonomi::wallet::WalletKey;

const SECRET_KEY: &str = "SECRET_KEY";
const REGISTER_SIGNING_KEY: &str = "REGISTER_SIGNING_KEY";

const SECRET_KEY_FILE: &str = "secret_key";
const REGISTER_SIGNING_KEY_FILE: &str = "register_signing_key";

pub fn get_secret_key() -> Result<String> {
    // try env var first
    let why_env_failed = match env::var(SECRET_KEY) {
        Ok(key) => return Ok(key),
        Err(e) => e,
    };

    // try from data dir
    let dir = get_client_data_dir_path()
        .wrap_err(format!("Failed to obtain secret key from env var: {why_env_failed}, reading from disk also failed as couldn't access data dir"))
        .with_suggestion(|| format!("make sure you've provided the {SECRET_KEY} env var"))?;

    // load the key from file
    let key_path = dir.join(SECRET_KEY_FILE);
    fs::read_to_string(&key_path)
        .wrap_err("Failed to read secret key from file".to_string())
        .with_suggestion(|| format!("make sure you've provided the {SECRET_KEY} env var or have the key in a file at {key_path:?}"))
}

pub fn get_register_signing_key() -> Result<String> {
    // try env var first
    let why_env_failed = match env::var(REGISTER_SIGNING_KEY) {
        Ok(key) => return Ok(key),
        Err(e) => e,
    };

    // try from data dir
    let dir = get_client_data_dir_path()
        .wrap_err(format!("Failed to obtain register signing key from env var: {why_env_failed}, reading from disk also failed as couldn't access data dir"))
        .with_suggestion(|| format!("make sure you've provided the {REGISTER_SIGNING_KEY} env var"))?;

    // load the key from file
    let key_path = dir.join(REGISTER_SIGNING_KEY_FILE);
    fs::read_to_string(&key_path)
        .wrap_err("Failed to read secret key from file".to_string())
        .with_suggestion(|| format!("make sure you've provided the {REGISTER_SIGNING_KEY} env var or have the key in a file at {key_path:?}"))
}

pub fn get_client_data_dir_path() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::data_dir()
        .ok_or_else(|| eyre!("Failed to obtain data dir, your OS might not be supported."))?;
    home_dirs.push("safe");
    home_dirs.push("client");
    std::fs::create_dir_all(home_dirs.as_path())
        .wrap_err("Failed to create data dir".to_string())?;
    Ok(home_dirs)
}

pub fn get_peers(peers: PeersArgs) -> Result<Vec<Multiaddr>> {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime to spawn peers acquisition thread");
    rt.block_on(peers.get_peers())
        .wrap_err(format!("Please provide valid Network peers to connect to"))
        .with_suggestion(|| format!("make sure you've provided network peers using the --peers option or the {SAFE_PEERS_ENV} env var"))
        .with_suggestion(|| format!("a peer address looks like this: /ip4/42.42.42.42/udp/4242/quic-v1/p2p/B64nodePeerIDvdjb3FAJF4ks3moreBase64CharsHere"))
}
