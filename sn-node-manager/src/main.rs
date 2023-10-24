mod install;
mod service;

use crate::install::{get_node_registry_path, install, NodeRegistry};
use crate::service::NodeServiceManager;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use sn_releases::SafeReleaseRepositoryInterface;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
pub enum SubCmd {
    /// Install `safenode` as a service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "install")]
    Install {
        /// The number of service instances
        #[clap(long)]
        count: Option<u16>,
        /// The version of safenode
        #[clap(long)]
        version: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cmd::parse();
    match args.cmd {
        SubCmd::Install { count, version } => {
            if !is_running_as_root() {
                return Err(eyre!("The install command must run as the root user"));
            }
            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            install(
                count,
                None,
                version,
                &mut node_registry,
                &NodeServiceManager {},
                release_repo,
            )
            .await?;
            node_registry.save(&get_node_registry_path()?)?;
            Ok(())
        }
    }
}

#[cfg(unix)]
pub fn is_running_as_root() -> bool {
    users::get_effective_uid() == 0
}

#[cfg(windows)]
pub fn is_running_as_root() -> bool {
    // The Windows implementation for this will be much more complex.
    false
}
