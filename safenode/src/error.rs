// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::io;
use thiserror::Error;

use crate::stableset::StableSetMsg;

/// The type returned by the `sn_routing` message handling methods.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Comms error.
    #[error("Comms error: {0}")]
    Comms(#[from] crate::comms::Error),
    /// No response stream was returned
    #[error("No response stream provided from: {0:?}")]
    NoResponseStream(crate::comms::MsgReceived<StableSetMsg>),
    /// JSON serialisation error.
    #[error("JSON serialisation error:: {0}")]
    JsonSerialisation(#[from] serde_json::Error),
    #[error("Tokio channel could not be sent to: {0}")]
    TokioChannel(String),
    #[cfg(feature = "otlp")]
    #[error("OpenTelemetry Tracing error: {0}")]
    OpenTelemetryTracing(#[from] opentelemetry::trace::TraceError),
}
