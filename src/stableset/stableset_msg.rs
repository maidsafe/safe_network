use crate::comms::MsgTrait;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum StableSetMsg {
    #[default]
    Ping,
    Pong,
}

impl MsgTrait for StableSetMsg {}
