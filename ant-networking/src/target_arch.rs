// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
/// Wasm32 target arch does not support `time` or spawning via tokio
/// so we shim in alternatives here when building for that architecture
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::{
    spawn,
    time::{interval, sleep, timeout, Interval},
};

#[cfg(target_arch = "wasm32")]
pub use std::time::Duration;

#[cfg(target_arch = "wasm32")]
pub use wasmtimer::{
    std::{Instant, SystemTime, UNIX_EPOCH},
    tokio::{interval, sleep, timeout, Interval},
};

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_futures::spawn_local as spawn;
