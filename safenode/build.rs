// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
use std::fs::File;
use std::io::Write;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("./src/protocol/safenode_proto/safenode.proto")?;

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git command");

    let git_hash = String::from_utf8(output.stdout).unwrap().trim().to_string();

    let mut file = File::create("src/git_hash.rs").expect("Failed to create git_hash.rs");
    writeln!(
        file,
        "/// The git commit at build time\npub const GIT_HASH: &str = \"{}\";",
        git_hash
    )
    .expect("Failed to write to git_hash.rs");
    Ok(())
}
