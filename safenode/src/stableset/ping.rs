use super::stableset_msg::StableSetMsg;
use super::Rx;
use crate::comms::{Comm, CommEvent, Error, MsgId, NetworkMsg, NetworkNode};

use std::collections::BTreeSet;

async fn ping_all_peers(sender: &Comm, peers: &BTreeSet<NetworkNode>) -> Result<(), Error> {
    for p in peers {
        let ping = StableSetMsg::Ping;
        let msg = NetworkMsg::<StableSetMsg> {
            id: MsgId::new(),
            payload: ping,
        };
        sender.send_out_bytes(*p, msg.id, msg.to_bytes()?).await;
    }
    Ok(())
}

async fn receive_ping(receiver: &mut Rx) -> Result<BTreeSet<NetworkNode>, Error> {
    let mut pongers = BTreeSet::new();

    if let Some(comm_event) = receiver.recv().await {
        match comm_event {
            CommEvent::Msg(msg) => {
                let stableset_msg = msg.wire_msg.payload;
                let sender = NetworkNode { addr: msg.sender };
                if stableset_msg == StableSetMsg::Ping {
                    pongers.insert(sender);
                }
                debug!("Received {stableset_msg:?} from {sender:?}");
            }
            CommEvent::Error { node_id: _, error } => return Err(error),
        }
    }
    Ok(pongers)
}

/// ensure peers are alive by pinging them and waiting for their ping
pub(crate) async fn ensure_peers_alive(
    sender: &Comm,
    receiver: &mut Rx,
    peers: &BTreeSet<NetworkNode>,
) {
    let mut alive_peers = BTreeSet::<NetworkNode>::new();
    let mut not_alive_peers: BTreeSet<NetworkNode> = peers.clone();

    // ping peers until they all showed up
    while !not_alive_peers.is_empty() {
        debug!("Pinging peers: {:?}", not_alive_peers);
        ping_all_peers(&sender, &not_alive_peers)
            .await
            .expect("ping failed");

        debug!("Checking responses...");
        let respondants = receive_ping(receiver).await.expect("pong failed");
        alive_peers.extend(respondants.clone());

        for r in respondants {
            not_alive_peers.remove(&r);
        }
    }

    debug!("Everyone is alive! {:?}", alive_peers);
}
