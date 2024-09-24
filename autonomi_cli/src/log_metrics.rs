// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
#[cfg(feature = "metrics")]
use sn_logging::{metrics::init_metrics, Level, LogBuilder, LogFormat};

fn init_logging_and_metrics(opt: &Opt) -> Result<()> {
    let logging_targets = vec![
        ("sn_networking".to_string(), Level::INFO),
        ("sn_build_info".to_string(), Level::TRACE),
        ("autonomi".to_string(), Level::TRACE),
        ("sn_logging".to_string(), Level::TRACE),
        ("sn_peers_acquisition".to_string(), Level::TRACE),
        ("sn_protocol".to_string(), Level::TRACE),
        ("sn_registers".to_string(), Level::TRACE),
        ("sn_evm".to_string(), Level::TRACE),
    ];
    let mut log_builder = LogBuilder::new(logging_targets);
    log_builder.output_dest(opt.log_output_dest);
    log_builder.format(opt.log_format.unwrap_or(LogFormat::Default));
    let _log_handles = log_builder.initialize()?;

    #[cfg(feature = "metrics")]
    tokio::spawn(init_metrics(std::process::id()));
    Ok(())
}