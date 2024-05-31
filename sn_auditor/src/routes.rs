// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::dag_db::{self, SpendDagDb};
use color_eyre::eyre::{eyre, Result};
use sn_client::transfers::SpendAddress;
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{Cursor, Write},
    str::FromStr,
};
use tiny_http::{Request, Response};

pub(crate) fn spend_dag_svg(_dag: &SpendDagDb) -> Result<Response<Cursor<Vec<u8>>>> {
    #[cfg(not(feature = "svg-dag"))]
    return Ok(Response::from_string(
        "SVG DAG not enabled on this server (the host should enable it with the 'svg-dag' feature flag)",
    )
    .with_status_code(200));

    #[cfg(feature = "svg-dag")]
    {
        let svg = _dag
            .load_svg()
            .map_err(|e| eyre!("Failed to get SVG: {e}"))?;
        let response = Response::from_data(svg);
        Ok(response)
    }
}

pub(crate) async fn spend(
    dag: &SpendDagDb,
    request: &Request,
) -> Result<Response<Cursor<Vec<u8>>>> {
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
        .await
        .map_err(|e| eyre!("Failed to get spend JSON: {e}"))?;
    let response = Response::from_data(json);
    Ok(response)
}

pub(crate) fn not_found() -> Result<Response<Cursor<Vec<u8>>>> {
    let response = Response::from_string("404: Try /").with_status_code(404);
    Ok(response)
}

pub(crate) async fn beta_rewards(dag: &SpendDagDb) -> Result<Response<Cursor<Vec<u8>>>> {
    let json = dag
        .beta_program_json()
        .await
        .map_err(|e| eyre!("Failed to get beta rewards JSON: {e}"))?;
    let response = Response::from_data(json);
    Ok(response)
}

pub(crate) async fn add_participant(
    dag: &SpendDagDb,
    request: &Request,
) -> Result<Response<Cursor<Vec<u8>>>> {
    let discord_id = match request.url().split('/').last() {
        Some(discord_id) => {
            // TODO: When we simply accept POST we can remove this decoding
            // For now we need it to decode #fragments in urls
            let discord_id = urlencoding::decode(discord_id)?;
            discord_id.to_string()
        }
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
    } else if discord_id.chars().count() == 0 {
        return Ok(Response::from_string("discord_id cannot be empty").with_status_code(400));
    }

    if let Err(err) = track_new_participant(dag, discord_id.to_owned()).await {
        return Ok(
            Response::from_string(format!("Failed to track new participant: {err}"))
                .with_status_code(400),
        );
    }

    Ok(Response::from_string("Successfully added participant "))
}

async fn track_new_participant(dag: &SpendDagDb, discord_id: String) -> Result<()> {
    dag.track_new_beta_participants(BTreeSet::from_iter([discord_id.to_owned()]))
        .await?;

    // only append new ids
    if dag.is_participant_tracked(&discord_id).await? {
        return Ok(());
    }

    let local_participants_file = dag.path.join(dag_db::BETA_PARTICIPANTS_FILENAME);

    if local_participants_file.exists() {
        let mut file = OpenOptions::new()
            .append(true)
            .open(local_participants_file)
            .map_err(|e| eyre!("Failed to open file: {e}"))?;
        writeln!(file, "{discord_id}")?;
    } else {
        let mut file = File::create(local_participants_file)
            .map_err(|e| eyre!("Failed to create file: {e}"))?;
        writeln!(file, "{discord_id}")?;
    }

    Ok(())
}
