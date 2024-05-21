// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::eyre::{eyre, Result};
use sn_client::transfers::SpendAddress;
use std::{io::Cursor, str::FromStr};
use tiny_http::{Request, Response};

use crate::dag_db::SpendDagDb;

pub(crate) fn spend_dag_svg(dag: &SpendDagDb) -> Result<Response<Cursor<Vec<u8>>>> {
    let svg = dag
        .load_svg()
        .map_err(|e| eyre!("Failed to get SVG: {e}"))?;
    let response = Response::from_data(svg);
    Ok(response)
}

pub(crate) fn spend(dag: &SpendDagDb, request: &Request) -> Result<Response<Cursor<Vec<u8>>>> {
    let addr = match request.url().split('/').last() {
        Some(addr) => addr,
        None => {
            return Ok(Response::from_string(
                "No address provided. Should be /spend/[your_spend_address_here]",
            )
            .with_status_code(400))
        }
    };
    let spend_addr = match SpendAddress::from_str(addr) {
        Ok(addr) => addr,
        Err(e) => {
            return Ok(Response::from_string(format!(
                "Failed to parse address: {e}. Should be /spend/[your_spend_address_here]"
            ))
            .with_status_code(400))
        }
    };
    let json = dag
        .spend_json(spend_addr)
        .map_err(|e| eyre!("Failed to get spend JSON: {e}"))?;
    let response = Response::from_data(json);
    Ok(response)
}

pub(crate) fn not_found() -> Result<Response<Cursor<Vec<u8>>>> {
    let response = Response::from_string("404: Try /").with_status_code(404);
    Ok(response)
}

pub(crate) fn beta_rewards(dag: &SpendDagDb) -> Result<Response<Cursor<Vec<u8>>>> {
    let json = dag
        .beta_program_json()
        .map_err(|e| eyre!("Failed to get beta rewards JSON: {e}"))?;
    let response = Response::from_data(json);
    Ok(response)
}

pub(crate) fn add_participant(
    dag: &SpendDagDb,
    request: &Request,
) -> Result<Response<Cursor<Vec<u8>>>> {
    let discord_id = match request.url().split('/').last() {
        Some(discord_id) => discord_id,
        None => {
            return Ok(Response::from_string(
                "No discord_id provided. Should be /add-participant/[your_discord_id_here]",
            )
            .with_status_code(400))
        }
    };

    if discord_id.chars().count() >= 32 {
        return Ok(
            Response::from_string("discord_id cannot be more than 32 chars").with_status_code(400),
        );
    }

    match dag.track_new_beta_participants(vec![discord_id.to_owned()]) {
        Ok(()) => Ok(Response::from_string("Added participant")),
        Err(e) => Ok(
            Response::from_string(format!("Failed to add participant: {e}")).with_status_code(500),
        ),
    }
}
