// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod disk_backed_record_store;
mod registers;
mod spends;

pub(crate) use self::{
    disk_backed_record_store::{
        DiskBackedRecordStore, DiskBackedRecordStoreConfig, REPLICATION_INTERVAL_LOWER_BOUND,
        REPLICATION_INTERVAL_UPPER_BOUND,
    },
    registers::{RegisterReplica, RegisterStorage},
    spends::SpendStorage,
};

use crate::protocol::error::StorageError;

use std::{
    path::{Path, PathBuf},
    result,
};
use xor_name::XorName;

// A specialised `Result` type used within this storage implementation.
type Result<T> = result::Result<T, StorageError>;

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
