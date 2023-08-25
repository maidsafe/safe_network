// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod api;
mod chunks;
mod error;
mod event;
mod faucet;
mod file_apis;
mod register;
mod wallet;

pub(crate) use error::Result;

pub use self::{
    error::Error,
    event::{ClientEvent, ClientEventsReceiver},
    faucet::{get_tokens_from_faucet, load_faucet_wallet_from_genesis_wallet},
    file_apis::{Files, MAX_CONCURRENT_CHUNK_UPLOAD},
    register::ClientRegister,
    wallet::{send, WalletClient},
};

use self::event::ClientEventsChannel;
use indicatif::ProgressBar;
use sn_networking::Network;

/// Client API implementation to store and get data.
#[derive(Clone)]
pub struct Client {
    network: Network,
    events_channel: ClientEventsChannel,
    signer: bls::SecretKey,
    peers_added: usize,
    progress: Option<ProgressBar>,
}
