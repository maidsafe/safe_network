use std::collections::BTreeSet;

use super::stable_set::{Member, StableSet};
use crate::comms::{MsgTrait, NetworkNode};

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Hash, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub enum StableSetMsg {
    #[default]
    Ping,
    Pong,
    ReqJoin(NetworkNode),
    ReqLeave(NetworkNode),
    JoinShare(Member),
    Sync(StableSet),
}

impl MsgTrait for StableSetMsg {}
