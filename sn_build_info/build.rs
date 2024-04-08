// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
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

    Ok(())
}
