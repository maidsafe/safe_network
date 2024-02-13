// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod api;
mod audit;
mod chunks;
mod error;
mod event;
mod faucet;
mod files;
mod folders;
mod register;
mod wallet;

pub use self::{
    audit::SpendDag,
    error::Error,
    event::{ClientEvent, ClientEventsBroadcaster, ClientEventsReceiver},
    faucet::{get_tokens_from_faucet, load_faucet_wallet_from_genesis_wallet},
    files::{
        download::{FilesDownload, FilesDownloadEvent},
        upload::{FileUploadEvent, FilesUpload},
        FilesApi, BATCH_SIZE,
    },
    folders::{FolderEntry, FoldersApi},
    register::ClientRegister,
    wallet::{broadcast_signed_spends, send, StoragePaymentResult, WalletClient},
};
pub(crate) use error::Result;

use sn_networking::Network;

#[cfg(target_arch = "wasm32")]
use console_error_panic_hook;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::console;

// This is like the `main` function, except for JavaScript.
// this is an arbitrary change to test the release-plz
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn main_js() -> std::result::Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    // #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    console::log_1(&JsValue::from_str("Hello safe world!"));

    // Tracing
    // TODO: dont log _everything_
    // right now it logs all libp2p entirely.
    tracing_wasm::set_as_global_default();

    Ok(())
}

/// A quick client that only takes some peers to connect to
#[wasm_bindgen]
#[cfg(target_arch = "wasm32")]
pub async fn get_data(peer: &str, data_address: &str) -> std::result::Result<(), JsError> {
    let bytes = hex::decode(&data_address).expect("Input address is not a hex string");
    let xor_name = xor_name::XorName(
        bytes
            .try_into()
            .expect("Failed to parse XorName from hex string"),
    );

    use sn_protocol::storage::ChunkAddress;
    console::log_1(&JsValue::from_str(peer));

    let the_peer = sn_peers_acquisition::parse_peer_addr(peer)?;

    console::log_1(&JsValue::from_str(&format!(
        "Provided Peer was {the_peer:?}"
    )));

    // TODO: We need to tidy this up, the client loops forever in the browser, and eventually crashes
    // it does _do things_ but errors surface, and even after getting data, it continues...
    let client = Client::quick_start(Some(vec![the_peer]))
        .await
        .map_err(|e| JsError::new(&format!("Client could not start: {e:?}")))?;

    console::log_1(&JsValue::from_str("Client started {chunk:?}"));

    let chunk = client
        .get_chunk(ChunkAddress::new(xor_name), false, None)
        .await
        .map_err(|e| JsError::new(&format!("Client get data failed: {e:?}")))?;

    console::log_1(&JsValue::from_str(&format!("Data found {chunk:?}")));

    Ok(())
}

/// Client API implementation to store and get data.
#[derive(Clone)]
pub struct Client {
    network: Network,
    events_broadcaster: ClientEventsBroadcaster,
    signer: bls::SecretKey,
}
