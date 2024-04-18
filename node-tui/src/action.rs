use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
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
