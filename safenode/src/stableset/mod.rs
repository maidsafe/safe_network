mod membership;
mod ping;
mod stable_set;
mod stableset_msg;

use membership::Membership;
pub use stableset_msg::StableSetMsg;

use crate::{
    comms::{Comm, CommEvent, NetworkNode},
    error::Result,
};

use std::collections::BTreeSet;

pub type Rx = tokio::sync::mpsc::Receiver<CommEvent<StableSetMsg>>;

/// start stable set and no_return unless fatal error
pub async fn run_stable_set(
    comm: Comm,
    mut receiver: Rx,
    myself: NetworkNode,
    peers: BTreeSet<NetworkNode>,
) -> Result<()> {
    comm.set_comm_targets(peers.clone()).await;

    // make sure peers are alive
    ping::ensure_peers_alive(&comm, &mut receiver, &peers).await;

    // start membership with hardcoded peers
    let hardcoded_network_nodes = peers.into_iter().chain([myself]).collect();
    let mut membership = Membership::new(&hardcoded_network_nodes);

    // infinite stableset loop
    while let Some(comm_event) = receiver.recv().await {
        match comm_event {
            CommEvent::Msg(msg) => {
                let stableset_msg = msg.wire_msg.payload;
                let sender = NetworkNode { addr: msg.sender };
                info!("Received {stableset_msg:?} from {sender:?}");

                let elders = &membership.elders();
                membership.on_msg(elders, myself, sender, stableset_msg);
            }
            CommEvent::Error { node_id: _, error } => info!("Comm Event Error: {error:?}"),
        }
    }

    Ok(())
}
