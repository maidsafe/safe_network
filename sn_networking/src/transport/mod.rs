// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg_attr(target_arch = "wasm32", path = "wasm32.rs")]
#[cfg_attr(not(target_arch = "wasm32"), path = "other.rs")]
pub(crate) mod mod_impl;

pub(crate) use mod_impl::build_transport;
