// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod error;
mod registers;
mod spends;

pub use self::{error::Result, registers::register};

pub(crate) use self::{registers::RegisterStorage, spends::SpendStorage};

use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use xor_name::XorName;

const BIT_TREE_DEPTH: usize = 20;

// Helper that returns the prefix tree path of depth BIT_TREE_DEPTH for a given xorname
// Example:
// - with a xorname with starting bits `010001110110....`
// - and a BIT_TREE_DEPTH of `6`
// returns the path `ROOT_PATH/0/1/0/0/0/1`
fn prefix_tree_path(root: &Path, xorname: XorName) -> PathBuf {
    let bin = format!("{xorname:b}");
    let prefix_dir_path: PathBuf = bin.chars().take(BIT_TREE_DEPTH).map(String::from).collect();
    root.join(prefix_dir_path)
}

fn list_files_in(path: &Path) -> Vec<PathBuf> {
    if !path.exists() {
        return vec![];
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| match e {
            Ok(direntry) => Some(direntry),
            Err(err) => {
                warn!("Store: failed to process filesystem entry: {}", err);
                None
            }
        })
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect()
}
