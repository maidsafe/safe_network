// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use chrono::Utc;
use tracing::debug;

/// Git information separated by slashes: `<sha> / <branch> / <describe>`
pub const fn git_info() -> &'static str {
    concat!(
        env!("VERGEN_GIT_BRANCH"),
        " / ",
        env!("VERGEN_GIT_SHA"),
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

/// Nightly version format: YYYY.MM.DD
pub fn nightly_version() -> String {
    let now = Utc::now();
    now.format("%Y.%m.%d").to_string()
}

/// Git information for nightly builds: `<date> / <branch> / <sha>`
pub fn nightly_git_info() -> String {
    format!("{} / {} / {}", nightly_version(), git_branch(), git_sha(),)
}

pub fn package_version() -> String {
    format!(
        "{}.{}.{}.{}",
        env!("RELEASE_YEAR"),
        env!("RELEASE_MONTH"),
        env!("RELEASE_CYCLE"),
        env!("RELEASE_CYCLE_COUNTER")
    )
}

pub fn full_version_info(crate_version: &str, protocol_version: &str) -> String {
    format!(
        "v{}\nNetwork version: {}\nPackage version: {}\nGit info: {}",
        crate_version,
        protocol_version,
        package_version(),
        git_info()
    )
}

pub fn version_string(crate_version: &str, protocol_version: &str) -> String {
    if cfg!(feature = "nightly") {
        format!(
            "-- Nightly Release {}\nGit info: {} / {}",
            nightly_version(),
            git_branch(),
            git_sha(),
        )
    } else {
        full_version_info(crate_version, protocol_version)
    }
}

pub fn log_version_info(crate_version: &str, protocol_version: &str) {
    if cfg!(feature = "nightly") {
        debug!("nightly build info: {}", nightly_git_info());
    } else {
        debug!("version: {}", crate_version);
        debug!("network version: {}", protocol_version);
        debug!("package version: {}", package_version());
        debug!("git info: {}", git_info());
    }
}