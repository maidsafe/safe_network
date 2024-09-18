// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
use std::fs;
use std::path::Path;
use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EmitBuilder::builder()
        .build_date()
        // Emit the short SHA-1 hash of the current commit
        .git_sha(true)
        // Emit the current branch name
        .git_branch()
        // Emit the annotated tag of the current commit, or fall back to abbreviated commit object.
        .git_describe(true, false, None)
        .emit()?;

    let release_info_path = Path::new("../release-cycle-info");
    let contents =
        fs::read_to_string(release_info_path).expect("Failed to read release-cycle-info");

    let mut year = String::new();
    let mut month = String::new();
    let mut cycle = String::new();
    let mut counter = String::new();

    for line in contents.lines() {
        if line.starts_with("release-year:") {
            year = line.split(':').nth(1).unwrap().trim().to_string();
        } else if line.starts_with("release-month:") {
            month = line.split(':').nth(1).unwrap().trim().to_string();
        } else if line.starts_with("release-cycle:") {
            cycle = line.split(':').nth(1).unwrap().trim().to_string();
        } else if line.starts_with("release-cycle-counter:") {
            counter = line.split(':').nth(1).unwrap().trim().to_string();
        }
    }

    println!("cargo:rustc-env=RELEASE_YEAR={}", year);
    println!("cargo:rustc-env=RELEASE_MONTH={}", month);
    println!("cargo:rustc-env=RELEASE_CYCLE={}", cycle);
    println!("cargo:rustc-env=RELEASE_CYCLE_COUNTER={}", counter);

    Ok(())
}
