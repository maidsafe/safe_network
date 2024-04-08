// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod dag_error;
mod spend_check;
mod spend_dag;
mod spend_dag_building;

#[cfg(test)]
mod tests;

pub use dag_error::{DagError, SpendFault};
pub use spend_dag::{SpendDag, SpendDagGet};
