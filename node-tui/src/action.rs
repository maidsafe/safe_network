// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
    HomeActions(HomeActions),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum HomeActions {
    AddNode,
    AddNodeCompleted,
    StartNodes,
    StartNodesCompleted,
    StopNode,
    StopNodeCompleted,

    PreviousTableItem,
    NextTableItem,
}
