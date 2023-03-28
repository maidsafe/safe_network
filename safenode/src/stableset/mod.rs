mod join;
mod stable_set;
mod stableset_msg;

use stable_set::{Elders, StableSet};
pub use stableset_msg::StableSetMsg;

use crate::{
    comms::{
        send_on_stream, Comm, CommEvent, MsgId, MsgReceived, NetworkMsg, NetworkNode,
        ResponseStream,
    },
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
                let (response_stream, stableset_msg, sender) = msg_received_parts(msg)?;

                let members_to_sync = update_set_and_get_nodes_to_sync(
                    &mut stable_set,
                    stableset_msg,
                    myself,
                    sender,
                    &comm,
                )
                .await;

                // Finally we send out our current state of affairs to all nodes
                sync_nodes(
                    stable_set.clone(),
                    members_to_sync,
                    myself,
                    sender,
                    response_stream,
                    &comm,
                )
                .await?
            }
            CommEvent::Error { node_id: _, error } => info!("Comm Event Error: {error:?}"),
        }
    }

    Ok(())
}

/// Sync all nodes
/// Will sync the sender on their ReponseStream
/// OTher nodes are passed through the normal comms flow
async fn sync_nodes(
    stable_set: StableSet,
    members_to_sync: BTreeSet<NetworkNode>,

    myself: NetworkNode,
    sender: NetworkNode,
    response_stream: Option<ResponseStream>,
    comm: &Comm,
) -> Result<()> {
    let current_stable_set = stable_set.clone();
    let sync_msg = StableSetMsg::Sync(current_stable_set);

    let msg = NetworkMsg::<StableSetMsg> {
        id: MsgId::new(),
        payload: sync_msg,
    };

    let msg_bytes = msg.to_bytes()?;

    let mut sender_synced = false;
    if members_to_sync.contains(&sender) {
        if let Some(stream) = response_stream {
            debug!("About to sync sender: {sender:?}");
            sender_synced = true;
            send_on_stream(msg.msg_id(), msg_bytes.clone(), stream).await;
        }
    }

    for member in members_to_sync {
        // TODO: I think we can negate self-send when properly contact all elders
        // if member == myself {
        //     warn!("There should be no need to self-sync");
        //     continue;
        // }

        if member == sender && sender_synced {
            continue;
        }

        debug!("Syncing {member:?}");
        comm.send_msg(member, msg.id, msg_bytes.clone()).await;
    }
    Ok(())
}

/// Updates our stableset based on a given msg
/// sets comms targets if we've changed
async fn update_set_and_get_nodes_to_sync(
    stable_set: &mut StableSet,
    stableset_msg: StableSetMsg,
    myself: NetworkNode,
    sender: NetworkNode,
    comm: &Comm,
) -> BTreeSet<NetworkNode> {
    let elders = &stable_set.elders();
    let mut members_to_sync =
        stable_set.on_msg_return_nodes_to_sync(elders, myself, sender, stableset_msg);

    // process everything we learnt about here...
    members_to_sync.extend(stable_set.process_pending_actions(sender));

    // update comms
    let valid_section_targets: BTreeSet<_> = stable_set.members().iter().map(|n| n.id).collect();
    comm.set_comm_targets(valid_section_targets.clone()).await;

    debug!("These members should get synced now: {members_to_sync:?}");

    members_to_sync
}

/// Pulls our relevant parts of a comms MsgReceived event
fn msg_received_parts(
    msg_event: MsgReceived<StableSetMsg>,
) -> Result<(Option<ResponseStream>, StableSetMsg, NetworkNode)> {
    // If we get a response over a stream... there is no response stream
    let stream = msg_event.response_stream;
    let stableset_msg = msg_event.wire_msg.payload;
    let sender = NetworkNode {
        addr: msg_event.sender,
    };

    info!("Received {stableset_msg:?} from {sender:?}");

    Ok((stream, stableset_msg, sender))
}
