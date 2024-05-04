// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use color_eyre::{eyre::eyre, eyre::Result};

use sn_node_manager::{
    helpers::{create_temp_dir, download_and_extract_release},
    VerbosityLevel,
};
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use which::which;

#[derive(Debug)]
enum TerminalType {
    Alacritty(PathBuf),
    Gnome(PathBuf),
    ITerm2(PathBuf),
    Kitty(PathBuf),
    Konsole(PathBuf),
    MacOS(PathBuf),
    WindowsCmd(PathBuf),
    WindowsPowershell(PathBuf),
    WindowsTerminal(PathBuf),
    Xterm(PathBuf),
}

fn detect_terminal() -> Result<TerminalType> {
    if cfg!(target_os = "windows") {
        if let Ok(path) = which("wt.exe") {
            Ok(TerminalType::WindowsTerminal(path))
        } else if let Ok(path) = which("powershell.exe") {
            Ok(TerminalType::WindowsPowershell(path))
        } else if let Ok(path) = which("cmd.exe") {
            Ok(TerminalType::WindowsCmd(path))
        } else {
            Err(eyre!("Could not find suitable terminal on Windows"))
        }
    } else if cfg!(target_os = "macos") {
        if which("iTerm.app").is_ok() {
            Ok(TerminalType::ITerm2(PathBuf::from("osascript")))
        } else {
            Ok(TerminalType::MacOS(PathBuf::from("osascript")))
        }
    } else {
        get_linux_terminal()
    }
}

fn get_linux_terminal() -> Result<TerminalType> {
    match std::env::var("TERM") {
        Ok(val) => {
            if let Ok(path) = which(val.clone()) {
                match val.as_str() {
                    "alacritty" => Ok(TerminalType::Alacritty(path)),
                    "gnome" => Ok(TerminalType::Gnome(path)),
                    "kitty" => Ok(TerminalType::Kitty(path)),
                    "konsole" => Ok(TerminalType::Konsole(path)),
                    "xterm" => Ok(TerminalType::Xterm(path)),
                    "xterm-256color" => Ok(TerminalType::Xterm(path)),
                    _ => Err(eyre!("Terminal '{val}' is not supported")),
                }
            } else {
                try_available_linux_terminals()
            }
        }
        Err(_) => try_available_linux_terminals(),
    }
}

fn try_available_linux_terminals() -> Result<TerminalType> {
    if let Ok(path) = which("alacritty") {
        Ok(TerminalType::Alacritty(path))
    } else if let Ok(path) = which("gnome-terminal") {
        Ok(TerminalType::Gnome(path))
    } else if let Ok(path) = which("kitty") {
        Ok(TerminalType::Kitty(path))
    } else if let Ok(path) = which("konsole") {
        Ok(TerminalType::Konsole(path))
    } else if let Ok(path) = which("xterm") {
        Ok(TerminalType::Xterm(path))
    } else if let Ok(path) = which("xterm-256color") {
        Ok(TerminalType::Xterm(path))
    } else {
        Err(eyre!("Could not find terminal on Linux"))
    }
}

fn launch_terminal(terminal_type: &TerminalType, launchpad_path: &Path) -> Result<()> {
    let launchpad_path = launchpad_path.to_string_lossy().to_string();
    match terminal_type {
        TerminalType::Kitty(path) | TerminalType::Konsole(path) | TerminalType::Xterm(path) => {
            Command::new(path).arg("-e").arg(launchpad_path).spawn()?;
            Ok(())
        }
        TerminalType::Alacritty(path) => {
            Command::new(path)
                .arg("--command")
                .arg("sudo")
                .arg("sh")
                .arg("-c")
                .arg(launchpad_path)
                .spawn()?;
            Ok(())
        }
        TerminalType::Gnome(path) => {
            Command::new(path)
                .arg("--")
                .arg("sudo")
                .arg(launchpad_path)
                .spawn()?;
            Ok(())
        }
        TerminalType::MacOS(path) => {
            Command::new(path)
                .arg("-e")
                .arg(format!(
                    "tell application \"Terminal\" to do script \"sudo {}\"",
                    launchpad_path
                ))
                .spawn()?;
            Ok(())
        }
        TerminalType::ITerm2(path) => {
            Command::new(path)
            .arg("-e")
            .arg(format!("tell application \"iTerm\" to create window with default profile command \"sudo {}\"", launchpad_path))
            .spawn()?;
            Ok(())
        }
        TerminalType::WindowsCmd(path)
        | TerminalType::WindowsPowershell(path)
        | TerminalType::WindowsTerminal(path) => {
            Command::new(path).arg("/c").arg(launchpad_path).spawn()?;
            Ok(())
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, about)]
pub struct Cli {
    #[arg(long)]
    pub launchpad_path: Option<PathBuf>,
    #[arg(long)]
    pub launchpad_version: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let launchpad_path = if let Some(path) = args.launchpad_path {
        path
    } else {
        let path = get_node_launchpad_path()?;
        if path.exists() {
            path
        } else {
            println!("Retrieving latest version of Node Launchpad...");
            let release_repo = <dyn SafeReleaseRepoActions>::default_config();
            let (bin_path, _) = download_and_extract_release(
                ReleaseType::NodeLaunchpad,
                None,
                None,
                &*release_repo,
                VerbosityLevel::Normal,
                Some(create_temp_dir()?),
            )
            .await?;

            std::fs::copy(bin_path, path.clone())?;
            path
        }
    };

    let terminal_type = detect_terminal()?;
    launch_terminal(&terminal_type, &launchpad_path)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_node_launchpad_path() -> Result<PathBuf> {
    let home_dir_path =
        dirs_next::home_dir().ok_or_else(|| eyre!("Could not retrieve user's home directory"))?;
    let safe_dir_path = home_dir_path.join("safe");
    std::fs::create_dir_all(safe_dir_path.clone())?;
    Ok(safe_dir_path.join("node-launchpad.exe"))
}

#[cfg(target_family = "unix")]
fn get_node_launchpad_path() -> Result<PathBuf> {
    let home_dir_path =
        dirs_next::home_dir().ok_or_else(|| eyre!("Could not retrieve user's home directory"))?;
    let safe_dir_path = home_dir_path.join(".local").join("bin");
    std::fs::create_dir_all(safe_dir_path.clone())?;
    Ok(safe_dir_path.join("node-launchpad.exe"))
}
