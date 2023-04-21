// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    domain::storage::{
        DataAddress, {ChunkStorage, RegisterStorage},
    },
    network::Network,
    protocol::messages::{
        DataRequest, DataResponse, Event, Query, QueryResponse, RegisterQuery, Request, Response,
    },
};
use libp2p::PeerId;
use std::{collections::HashSet, time::Duration};
use tokio::spawn;

pub(crate) async fn send_out_data_addrs(
    network: Network,
    chunk_storage: ChunkStorage,
    _register_storage: RegisterStorage,
) {
    tokio::time::sleep(Duration::from_secs(10)).await;
    let mut our_closest_peers = HashSet::new();
    if let Ok(closest) = network
        .get_closest_local_peers(network.peer_id.to_bytes(), false)
        .await
    {
        our_closest_peers = closest.into_iter().collect();
    }
    loop {
        let new_peers = check_for_group_change(&network, &mut our_closest_peers).await;
        info!("Churn detected, the new_peers in our closest_group is: {new_peers:?}");
        // send out our data address to them
        // todo: should do the same for reg storage
        let data_addresses: Vec<DataAddress> = chunk_storage
            .addrs()
            .into_iter()
            .map(DataAddress::Chunk)
            .collect();
        for peer in new_peers {
            let network_clone = network.clone();
            let data_addresses = data_addresses.clone();
            // spawn task to send event to each peer
            let _handle = spawn(async move {
                let _ = network_clone
                    .send_request(
                        Request::Event(Event::ReplicateData(data_addresses.clone())),
                        peer,
                    )
                    .await;
            });
        }
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
}

pub(crate) async fn ask_peers_for_data(
    network: Network,
    chunk_storage: ChunkStorage,
    register_storage: RegisterStorage,
    data_addresses: Vec<DataAddress>,
) {
    // for each addr, spawn a task to request for the data corresponding to the addr from its
    // closest peers and store them.
    for addr in data_addresses {
        let req = match addr {
            DataAddress::Chunk(addr) => {
                if chunk_storage.get(&addr).await.is_ok() {
                    continue;
                }
                Request::Data(DataRequest::Query(Query::GetChunk(addr)))
            }
            DataAddress::Register(addr) => {
                if register_storage.addrs().await.contains(&addr) {
                    continue;
                }
                Request::Data(DataRequest::Query(Query::Register(RegisterQuery::Get(
                    addr,
                ))))
            }
            DataAddress::Spend(_) => todo!(),
        };
        let network = network.clone();
        let chunk_storage = chunk_storage.clone();
        let _register_storage = register_storage.clone();
        let _handle = spawn(async move {
            if let Ok(responses) = network.node_send_to_closest(&req).await {
                for resp in responses.iter().flatten() {
                    if let Response::Data(DataResponse::Query(QueryResponse::GetChunk(Ok(chunk)))) =
                        resp
                    {
                        if chunk_storage.store(chunk).await.is_ok() {
                            break;
                        }
                    } else if let Response::Data(DataResponse::Query(QueryResponse::GetRegister(
                        Ok(_register),
                    ))) = resp
                    {
                        // todo
                        // register_storage.update(register).await.ok();
                    };
                }
            }
        });
    }
}

async fn check_for_group_change(
    network: &Network,
    our_closest_peers: &mut HashSet<PeerId>,
) -> HashSet<PeerId> {
    // Create an interval stream that yields every x seconds
    let mut local_scan_interval = tokio::time::interval(Duration::from_secs(10));
    let mut network_scan_interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        // The first interval to elapsed will be executed. The select macro will be fair among the
        // two intervals, and the last executed one will not be executed again if both are ready.
        let new_closest_peers = tokio::select! {
            _ = local_scan_interval.tick() => {
                network
                    .get_closest_local_peers(network.peer_id.to_bytes(), false)
                    .await.ok()
            }
            _ = network_scan_interval.tick() => {
                network
                    .get_closest_peers(network.peer_id.to_bytes(), false)
                    .await.ok()
            }
        };

        if let Some(new_closest_peers) = new_closest_peers {
            let new_closest_peers = new_closest_peers.into_iter().collect::<HashSet<_>>();
            debug!(
                "our_closest_peers of len {}: {our_closest_peers:?}\n
                new_closest_peers of len {}: {new_closest_peers:?}",
                our_closest_peers.len(),
                new_closest_peers.len()
            );
            // difference is not symmetric; we only care about the new peers that have joined
            let new_peers = new_closest_peers
                .difference(our_closest_peers)
                .cloned()
                .collect::<HashSet<_>>();
            *our_closest_peers = new_closest_peers;
            if !new_peers.is_empty() {
                return new_peers;
            }
        }
    }
}
