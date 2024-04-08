// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Git information separated by slashes: `<sha> / <branch> / <describe>`
pub const fn git_info() -> &'static str {
    concat!(
        env!("VERGEN_GIT_SHA"),
        " / ",
        env!("VERGEN_GIT_BRANCH"),
        " / ",
        env!("VERGEN_GIT_DESCRIBE"),
        " / ",
        env!("VERGEN_BUILD_DATE")
    )
}

/// Annotated tag description, or fall back to abbreviated commit object.
pub const fn git_describe() -> &'static str {
    env!("VERGEN_GIT_DESCRIBE")
}

/// The current git branch.
pub const fn git_branch() -> &'static str {
    env!("VERGEN_GIT_BRANCH")
}

/// Shortened SHA-1 hash.
pub const fn git_sha() -> &'static str {
    env!("VERGEN_GIT_SHA")
}
