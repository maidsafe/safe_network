// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{claim_genesis, send_tokens};
use color_eyre::eyre::{eyre, Result};
use sn_client::Client;
use std::path;
use tiny_http::{Response, Server};
use tracing::{debug, error, trace};

/// Run the faucet server.
///
/// This will listen on port 8000 and send a transfer of tokens as response to any GET request.
///
/// # Example
///
/// ```bash
/// # run faucet server
/// cargo run  --features="local-discovery" --bin faucet --release -- server
///
/// # query faucet server for money for our address `get local wallet address`
/// curl "localhost:8000/`cargo run  --features="local-discovery"  --bin safe --release  wallet address | tail -n 1`" > transfer_hex
///
/// # receive transfer with our wallet
/// cargo run  --features="local-discovery" --bin safe --release  wallet receive --file transfer_hex
///
/// # balance should be updated
/// ```
pub async fn run_faucet_server(client: &Client) -> Result<()> {
    let server =
        Server::http("0.0.0.0:8000").map_err(|err| eyre!("Failed to start server: {err}"))?;
    claim_genesis(client).await.map_err(|err| {
        println!("Faucet Server couldn't start as we failed to claim Genesis");
        eprintln!("Faucet Server couldn't start as we failed to claim Genesis");
        error!("Faucet Server couldn't start as we failed to claim Genesis");
        err
    })?;

    // This println is used in sn_testnet to wait for the faucet to start.
    println!("Starting http server listening on port 8000...");
    debug!("Starting http server listening on port 8000...");
    for request in server.incoming_requests() {
        println!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );
        trace!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );
        let key = request.url().trim_matches(path::is_separator);

        match send_tokens(client, "100", key).await {
            Ok(transfer) => {
                println!("Sent tokens to {key}");
                debug!("Sent tokens to {key}");
                let response = Response::from_string(transfer);
                let _ = request.respond(response).map_err(|err| {
                    eprintln!("Failed to send response: {err}");
                    error!("Failed to send response: {err}");
                });
            }
            Err(err) => {
                eprintln!("Failed to send tokens to {key}: {err}");
                error!("Failed to send tokens to {key}: {err}");
                let response = Response::from_string(format!("Failed to send tokens: {err}"));
                let _ = request
                    .respond(response.with_status_code(500))
                    .map_err(|err| eprintln!("Failed to send response: {err}"));
            }
        }
    }
    Ok(())
}
