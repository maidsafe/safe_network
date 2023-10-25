mod install;
mod service;

use crate::install::{get_node_registry_path, install, NodeRegistry};
use crate::service::NodeServiceManager;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use sn_releases::SafeReleaseRepositoryInterface;
use std::path::PathBuf;

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
        /// The user the service should run as.
        ///
        /// If the account does not exist, it will be created.
        ///
        /// On Windows this argument will have no effect.
        #[clap(long)]
        user: Option<String>,
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
        SubCmd::Install {
            count,
            user,
            version,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The install command must run as the root user"));
            }
            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            install(
                get_safenode_install_path()?,
                count,
                user,
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
fn is_running_as_root() -> bool {
    users::get_effective_uid() == 0
}

#[cfg(windows)]
fn is_running_as_root() -> bool {
    // The Windows implementation for this will be much more complex.
    true
}

#[cfg(unix)]
fn get_safenode_install_path() -> Result<PathBuf> {
    Ok(PathBuf::from("/usr/local/bin"))
}

#[cfg(windows)]
fn get_safenode_install_path() -> Result<PathBuf> {
    let path = PathBuf::from("C:\\Program Files\\Maidsafe\\safenode");
    if !path.exists() {
        std::fs::create_dir_all(path.clone())?;
    }
    Ok(path)
}
