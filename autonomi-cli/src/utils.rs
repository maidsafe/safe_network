// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::client::{Amount, ClientEvent};

/// Summary of the upload operation.
#[derive(Debug, Clone)]
pub struct CliUploadSummary {
    /// Total tokens spent during the upload.
    pub tokens_spent: Amount,
    /// Total number of records uploaded.
    pub record_count: usize,
}

/// Collects upload summary from the event receiver.
/// Send a signal to the returned sender to stop collecting and to return the result via the join handle.
pub fn collect_upload_summary(
    mut event_receiver: tokio::sync::mpsc::Receiver<ClientEvent>,
) -> (
    tokio::task::JoinHandle<CliUploadSummary>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (upload_completed_tx, mut upload_completed_rx) = tokio::sync::oneshot::channel::<()>();
    let stats_thread = tokio::spawn(async move {
        let mut tokens: Amount = Amount::from(0);
        let mut records = 0;

        loop {
            tokio::select! {
                event = event_receiver.recv() => {
                    match event {
                        Some(ClientEvent::UploadComplete {
                            tokens_spent,
                            record_count
                        }) => {
                            tokens += tokens_spent;
                            records += record_count;
                        }
                        None => break,
                    }
                }
                _ = &mut upload_completed_rx => break,
            }
        }

        // try to drain the event receiver in case there are any more events
        while let Ok(event) = event_receiver.try_recv() {
            match event {
                ClientEvent::UploadComplete {
                    tokens_spent,
                    record_count,
                } => {
                    tokens += tokens_spent;
                    records += record_count;
                }
            }
        }

        CliUploadSummary {
            tokens_spent: tokens,
            record_count: records,
        }
    });

    (stats_thread, upload_completed_tx)
}
