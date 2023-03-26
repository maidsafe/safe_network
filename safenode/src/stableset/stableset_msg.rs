use std::collections::BTreeSet;

use super::stable_set::{Member, StableSet};
use crate::comms::{MsgTrait, NetworkNode};

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Hash, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum StableSetMsg {
    #[default]
    Ping,
    Pong,
    ReqJoin(NetworkNode),
    LeaveWitness(NetworkNode),
    JoinWitness(Member),
    Sync(StableSet),
}

impl MsgTrait for StableSetMsg {}
