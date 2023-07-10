// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::path;

use crate::{claim_genesis, send_tokens};
use eyre::{eyre, Result};
use sn_client::Client;

use tiny_http::{Response, Server};

/// Run the faucet server.
///
/// This will listen on port 8000 and send tokens to any request.
///
/// # Example
///
/// ```bash
/// # run faucet server
/// cargo run  --features="local-discovery" --bin faucet --release -- server
///
/// # query faucet server for DBC at `get local wallet address`
/// curl "localhost:8000/`cargo run  --features="local-discovery"  --bin safe --release  wallet address | tail -n 1`" > dbc_hex
///
/// # feed DBC to local wallet
/// cat dbc_hex | cargo run  --features="local-discovery"  --bin safe --release  wallet deposit --stdin
///
/// # balance should be updated
/// ```

pub async fn run_faucet_server(client: &Client) -> Result<()> {
    let server =
        Server::http("0.0.0.0:8000").map_err(|e| eyre!("Failed to start server: {}", e))?;
    claim_genesis(client).await;

    println!("Starting http server listening on port 8000...");
    for request in server.incoming_requests() {
        println!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );
        let key = request.url().trim_matches(path::is_separator);

        match send_tokens(client, "10", key).await {
            Ok(dbc) => {
                println!("Sent tokens to {}", key);
                let response = Response::from_string(dbc);
                let _ = request
                    .respond(response)
                    .map_err(|e| eprintln!("Failed to send response: {}", e));
            }
            Err(e) => {
                eprintln!("Failed to send tokens to {}: {}", key, e);
                let response = Response::from_string(format!("Failed to send tokens: {}", e));
                let _ = request
                    .respond(response.with_status_code(500))
                    .map_err(|e| eprintln!("Failed to send response: {}", e));
            }
        }
    }
    Ok(())
}
