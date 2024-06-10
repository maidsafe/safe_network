// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::eyre::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub(crate) struct State {
    seen_books: HashSet<u32>,
}

impl State {
    pub(crate) fn new() -> Self {
        State {
            seen_books: HashSet::new(),
        }
    }

    pub(crate) fn load_from_file(path: &Path) -> Result<Self> {
        if path.exists() {
            let file = File::open(path)?;
            let state: State = serde_json::from_reader(file)?;
            Ok(state)
        } else {
            Ok(Self::new())
        }
    }

    pub(crate) fn save_to_file(&self, path: &Path) -> Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer(file, self)?;
        Ok(())
    }

    pub(crate) fn mark_seen(&mut self, book_id: u32) {
        self.seen_books.insert(book_id);
    }

    pub(crate) fn has_seen(&self, book_id: u32) -> bool {
        if book_id == 0 && self.seen_books.is_empty() {
            return true;
        }
        self.seen_books.contains(&book_id)
    }

    pub(crate) fn max_seen(&self) -> u32 {
        if let Some(result) = self.seen_books.iter().max() {
            *result
        } else {
            0
        }
    }
}

pub(crate) async fn download_book(client: &Client, book_id: u32) -> Result<Vec<u8>> {
    let url = format!("http://www.gutenberg.org/ebooks/{book_id}.txt.utf-8");
    let response = client.get(&url).send().await?.bytes().await?;
    Ok(response.to_vec())
}
