// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Used by client to choose how fast their spend will be processed.
/// The chosen variant will map to a fee using the spend queue stats fetched from Nodes.
#[derive(
    Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum SpendPriority {
    /// `High` + 1 std dev.
    Highest,
    /// The highest fee in spend queue.
    High,
    /// Avg of `High` and `Normal`.
    MediumHigh,
    /// The avg fee in spend queue.
    Normal,
    /// Avg of `Normal` and `Low`.
    MediumLow,
    /// The lowest fee in spend queue.
    Low,
    /// `Low` - 1 std dev.
    Lowest,
}
