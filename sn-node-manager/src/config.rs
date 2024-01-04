// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::{eyre::eyre, Result};
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub fn get_node_manager_path() -> Result<PathBuf> {
    // This needs to be a system-wide location rather than a user directory because the `install`
    // command will run as the root user. However, it should be readable by non-root users, because
    // other commands, e.g., requesting status, shouldn't require root.
    use std::os::unix::fs::PermissionsExt;

    let path = Path::new("/var/safenode-manager/");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
        let mut perm = std::fs::metadata(path)?.permissions();
        perm.set_mode(0o755); // set permissions to rwxr-xr-x
        std::fs::set_permissions(path, perm)?;
    }

    Ok(path.to_path_buf())
}

#[cfg(unix)]
pub fn get_node_registry_path() -> Result<PathBuf> {
    let path = get_node_manager_path()?;
    Ok(path.join("node_registry.json"))
}

pub fn get_local_node_registry_path() -> Result<PathBuf> {
    let path = dirs_next::data_dir()
        .ok_or_else(|| eyre!("Cannot obtain user's data directory"))?
        .join("safe")
        .join("local_node_registry.json");
    Ok(path)
}

#[cfg(unix)]
pub fn get_service_data_dir_path(custom_path: Option<PathBuf>, owner: &str) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => PathBuf::from("/var/safenode-manager/services"),
    };
    create_owned_dir(path.clone(), owner)?;
    Ok(path)
}

#[cfg(windows)]
pub fn get_service_data_dir_path(custom_path: Option<PathBuf>, _owner: &str) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => PathBuf::from("C:\\ProgramData\\safenode\\data"),
    };
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

#[cfg(unix)]
pub fn get_service_log_dir_path(custom_path: Option<PathBuf>, owner: &str) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => PathBuf::from("/var/log/safenode"),
    };
    create_owned_dir(path.clone(), owner)?;
    Ok(path)
}

#[cfg(windows)]
pub fn get_service_log_dir_path(custom_path: Option<PathBuf>, _owner: &str) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => PathBuf::from("C:\\ProgramData\\safenode\\logs"),
    };
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

#[cfg(unix)]
pub fn create_owned_dir(path: PathBuf, owner: &str) -> Result<()> {
    use nix::unistd::{chown, Gid, Uid};
    use std::os::unix::fs::PermissionsExt;
    use users::get_user_by_name;

    std::fs::create_dir_all(&path)?;
    let permissions = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&path, permissions)?;

    let user = get_user_by_name(owner).ok_or_else(|| eyre!("User '{owner}' does not exist"))?;
    let uid = Uid::from_raw(user.uid());
    let gid = Gid::from_raw(user.primary_group_id());
    chown(&path, Some(uid), Some(gid))?;
    Ok(())
}

#[cfg(windows)]
pub fn create_owned_dir(path: PathBuf, _owner: &str) -> Result<()> {
    std::fs::create_dir_all(&path)?;
    Ok(())
}

#[cfg(windows)]
pub fn get_node_registry_path() -> Result<PathBuf> {
    let path = Path::new("C:\\ProgramData\\safenode-manager");
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    Ok(path.join("node_registry.json"))
}
