mod join;
mod stable_set;
mod stableset_msg;

use stable_set::{Elders, StableSet};
pub use stableset_msg::StableSetMsg;

use crate::{
    comms::{Comm, CommEvent, MsgId, NetworkMsg, NetworkNode},
    error::Result,
};

use std::{collections::BTreeSet, mem};

pub type Rx = tokio::sync::mpsc::Receiver<CommEvent<StableSetMsg>>;

/// start stable set and no_return unless fatal error
pub async fn run_stable_set(
    comm: Comm,
    mut receiver: Rx,
    myself: NetworkNode,
    peers: BTreeSet<NetworkNode>,
) -> Result<()> {
    comm.set_comm_targets(peers.clone()).await;

    // if we're not the first node
    if !peers.is_empty() {
        // Join the network
        join::send_join_msg_to_peers(&comm, myself, &peers).await?;
    }

    // start membership with hardcoded peers
    let hardcoded_network_nodes = peers.into_iter().chain([myself]).collect();
    let mut stable_set = StableSet::new(&hardcoded_network_nodes);

    // infinite stableset loop
    while let Some(comm_event) = receiver.recv().await {
        match comm_event {
            CommEvent::Msg(msg) => {
                let response_stream = &msg.send_stream;

                // TODO: move this check into comms, we would respond here for AE etc
                let Some(stream) = response_stream else {
                    warn!("No response stream provided. Dropping msg: {:?}", msg);
                    continue;
                };

                let stableset_msg = msg.wire_msg.payload;
                let sender = NetworkNode { addr: msg.sender };

                info!("Received {stableset_msg:?} from {sender:?}");

                let elders = &stable_set.elders();
                let mut members_to_sync =
                    stable_set.on_msg_return_nodes_to_sync(elders, myself, sender, stableset_msg);

                // process everything we learnt about here...
                members_to_sync.extend(stable_set.process_pending_actions(myself));
                
                
                let valid_section_targets: BTreeSet<_> = stable_set
                    .members()
                    .iter()
                    .map(|n| n.id)
                    .collect();
                comm.set_comm_targets(valid_section_targets.clone()).await;

                debug!("These members should get synced now: {members_to_sync:?}");
    
                // Finally we send out our current state of affairs to all nodes
                // who need it
                let mut current_stable_set = stable_set.clone();
                let sync_msg = StableSetMsg::Sync(current_stable_set);

                let msg = NetworkMsg::<StableSetMsg> {
                    id: MsgId::new(),
                    payload: sync_msg,
                };

                for member in members_to_sync {
                    // TODO: if we have repsonse stream, use that..?
                    debug!("Syncing {member:?}");
                    comm.send_msg(member, msg.id, msg.to_bytes()?).await;
                }
            }
            CommEvent::Error { node_id: _, error } => info!("Comm Event Error: {error:?}"),
        }
    }

    Ok(())
}
