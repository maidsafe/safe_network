// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::time::Duration;

/// Returns whether a hex string is a valid secret key in hex format.
pub fn is_valid_key_hex(hex: &str) -> bool {
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn duration_to_minute_seconds_string(duration: Duration) -> String {
    let elapsed_minutes = duration.as_secs() / 60;
    let elapsed_seconds = duration.as_secs() % 60;
    if elapsed_minutes > 0 {
        format!("{elapsed_minutes} minutes {elapsed_seconds} seconds")
    } else {
        format!("{elapsed_seconds} seconds")
    }
}

pub fn duration_to_minute_seconds_miliseconds_string(duration: Duration) -> String {
    let elapsed_minutes = duration.as_secs() / 60;
    let elapsed_seconds = duration.as_secs() % 60;
    let elapsed_millis = duration.subsec_millis();
    if elapsed_minutes > 0 {
        format!("{elapsed_minutes} minutes {elapsed_seconds} seconds {elapsed_millis} milliseconds")
    } else if elapsed_seconds > 0 {
        format!("{elapsed_seconds} seconds {elapsed_millis} milliseconds")
    } else {
        format!("{elapsed_millis} milliseconds")
    }
}
