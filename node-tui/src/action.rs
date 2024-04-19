use serde::{Deserialize, Serialize};
use sn_node_manager::cmd::node::ProgressType;
use strum::Display;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
    ProgressMessage(ProgressType),
    AddNode,
    StartNodes,
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    StartNode,
    Quit,
    Refresh,
    Error(String),
    Help,
}
