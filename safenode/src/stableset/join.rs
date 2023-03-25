use super::stableset_msg::StableSetMsg;
use super::Rx;
use crate::comms::{Comm, CommEvent, Error, MsgId, NetworkMsg, NetworkNode};
use crate::error::Result;
use std::collections::BTreeSet;

pub(crate) async fn send_join_msg_to_peers(
    sender: &Comm,
    my_node: NetworkNode,
    peers: &BTreeSet<NetworkNode>,
) -> Result<()> {
    for p in peers {
        let join = StableSetMsg::ReqJoin(my_node);
        let msg = NetworkMsg::<StableSetMsg> {
            id: MsgId::new(),
            payload: join,
        };
        sender.send_msg(*p, msg.id, msg.to_bytes()?).await;
    }
    Ok(())
}

// async fn receive_ping(receiver: &mut Rx) -> Result<BTreeSet<NetworkNode>, Error> {
//     let mut pongers = BTreeSet::new();

//     if let Some(comm_event) = receiver.recv().await {
//         match comm_event {
//             CommEvent::Msg(msg) => {
//                 let stableset_msg = msg.wire_msg.payload;
//                 let sender = NetworkNode { addr: msg.sender };
//                 if stableset_msg == StableSetMsg::Ping {
//                     pongers.insert(sender);
//                 }
//                 debug!("Received {stableset_msg:?} from {sender:?}");
//             }
//             CommEvent::Error { node_id: _, error } => return Err(error),
//         }
//     }
//     Ok(pongers)
// }

// /// Try and join the network
// pub(crate) async fn try_and_join(
//     sender: &Comm,
//     receiver: &mut Rx,
//     peers: &BTreeSet<NetworkNode>,
// ) {
//     let mut known_peers = BTreeSet::<NetworkNode>::new();
//     // let mut not_alive_peer: BTreeSet<NetworkNode> = peers.clone();

//     try_and_join_peers(&sender, &known_peers)
//         .await
//         .expect("ping failed");
//     // ping peers until they all showed up
//     // while !not_alive_peers.is_empty() {
//     //     debug!("Trying to join with peers: {:?}", not_alive_peers);

//         // debug!("Checking responses...");
//         // let respondants = receive_ping(receiver).await.expect("pong failed");
//         // known_peers.extend(respondants.clone());

//         // for r in respondants {
//         //     not_alive_peers.remove(&r);
//         // }
//     // }

//     debug!("We joined with intitial set: {:?}", known_peers);
// }
