// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod config;
pub mod helpers;
pub mod local;
pub mod node_control;
pub mod service;

#[derive(Clone, PartialEq)]
pub enum VerbosityLevel {
    Minimal,
    Normal,
    Full,
}

impl From<u8> for VerbosityLevel {
    fn from(verbosity: u8) -> Self {
        match verbosity {
            1 => VerbosityLevel::Minimal,
            2 => VerbosityLevel::Normal,
            3 => VerbosityLevel::Full,
            _ => VerbosityLevel::Normal,
        }
    }
}
