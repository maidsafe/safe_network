// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod commands;
mod log_metrics;

use color_eyre::Result;
use clap::Parser;
use indicatif::ProgressBar;

async fn main() -> Result<()> {
    color_eyre::install().expect("Failed to initialise error handler");
    let opt = Opt::parse();
    log_metrics::init_logging_and_metrics(&opt)
        .expect("Failed to initialise logging and metrics");

    // Log the full command that was run and the git version
    info!("\"{}\"", std::env::args().collect::<Vec<_>>().join(" "));
    let version = sn_build_info::git_info();
    info!("safe client built with git version: {version}");
    println!("safe client built with git version: {version}");


    let client_data_dir_path = get_client_data_dir_path()?;
}

fn get_client_data_dir_path() -> Result<PathBuf> {
    let mut home_dirs = dirs_next::data_dir().expect("Data directory should be obtainable");
    home_dirs.push("safe");
    home_dirs.push("client");
    std::fs::create_dir_all(home_dirs.as_path())?;
    Ok(home_dirs)
}



/* 
env:
- INITAL_PEERS: [peers] // always required
- `SECRET_KEY`: hex String // only needed for cmds that pay or decrypt
- `REGISTER_SIGNING_KEY`: hex String // only needed for cmds that edit registers

commands:

- file
    - cost [file]
        - prints estimate cost to upload file (gas+ANT)
    - upload [file]
        - uploads and pays for file and prints addr and price
        - need `SECRET_KEY` env var to be set
        - COSTS MONEY
    - download [addr] [dest_file]
        - downloads file from addr to dest_file
    - list
        - prints list of previous uploads (kept in a local json file)
- register
    - cost [name]
        - prints estimate cost to register name (gas+ANT)
        - need `REGISTER_SIGNING_KEY` env var to be set or register_key file to be present
    - create [name] [value]
        - creates new register with name and value
        - need `SECRET_KEY` env var to be set
        - need `REGISTER_SIGNING_KEY` env var to be set or register_key file to be present
        - COSTS MONEY
    - edit [name] [value]
        - edits register at addr with new value
        - need `REGISTER_SIGNING_KEY` env var to be set or register_key file to be present
    - get [name]
        - gets value of register with name
        - need `REGISTER_SIGNING_KEY` env var to be set or register_key file to be present
    - list
        - prints list of previous registers (kept in a local json file)
- vault
    - cost
        - prints estimate cost to create vault (gas+ANT)
    - create
        - create vauld at deterministic addr based on your `SECRET_KEY`
        - need `SECRET_KEY` env var to be set
        - need `REGISTER_SIGNING_KEY` env var to be set or register_key file to be present
        - COSTS MONEY
    - sync
        - syncs vault with network
        - including register_key file, register list, and file list
        - need `SECRET_KEY` env var to be set
- transfer
    - TBD: not used for evm
*/
