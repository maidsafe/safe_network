// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// The git commit pulled from the env var GIT_HASH (which is set during build at build.rs)
pub fn git_hash() -> String {
    if let Ok(git_hash) = std::env::var("GIT_HASH") {
        git_hash
    } else {
        "---- No git commit hash found ----".to_string()
    }
}
