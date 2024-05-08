// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::{eyre::eyre, Result};
use sn_releases::ReleaseType;
use std::path::PathBuf;

#[cfg(unix)]
pub fn get_daemon_install_path() -> PathBuf {
    PathBuf::from("/usr/local/bin/safenodemand")
}

#[cfg(windows)]
pub fn get_daemon_install_path() -> PathBuf {
    PathBuf::from("C:\\ProgramData\\safenodemand\\safenodemand.exe")
}

#[cfg(unix)]
pub fn get_node_manager_path() -> Result<PathBuf> {
    // This needs to be a system-wide location rather than a user directory because the `install`
    // command will run as the root user. However, it should be readable by non-root users, because
    // other commands, e.g., requesting status, shouldn't require root.
    use std::os::unix::fs::PermissionsExt;

    let path = if is_running_as_root() {
        let path = PathBuf::from("/var/safenode-manager/");
        std::fs::create_dir_all(&path)?;
        let mut perm = std::fs::metadata(&path)?.permissions();
        perm.set_mode(0o755); // set permissions to rwxr-xr-x
        std::fs::set_permissions(&path, perm)?;
        path
    } else {
        get_user_safenode_data_dir()?
    };

    if is_running_as_root() && !path.exists() {
        std::fs::create_dir_all(&path)?;
        let mut perm = std::fs::metadata(&path)?.permissions();
        perm.set_mode(0o755); // set permissions to rwxr-xr-x
        std::fs::set_permissions(&path, perm)?;
    }

    Ok(path)
}

#[cfg(windows)]
pub fn get_node_manager_path() -> Result<PathBuf> {
    let path = Path::new("C:\\ProgramData\\safenode-manager");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(path.to_path_buf())
}

#[cfg(unix)]
pub fn get_node_registry_path() -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path = get_node_manager_path()?;
    let node_registry_path = path.join("node_registry.json");
    if is_running_as_root() && !node_registry_path.exists() {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true) // Do not append to the file if it already exists.
            .open(node_registry_path.clone())?;
        // Set the permissions of /var/safenode-manager/node_registry.json to rwxrwxrwx. The
        // `status` command updates the registry with the latest information it has on the
        // services at the time it runs. It's normally the case that service management status
        // operations do not require elevated privileges. If we want that to be the case, we
        // need to give all users the ability to write to the registry file. Everything else in
        // the /var/safenode-manager directory and its subdirectories will still require
        // elevated privileges.
        let mut perm = std::fs::metadata(node_registry_path.clone())?.permissions();
        perm.set_mode(0o777);
        std::fs::set_permissions(node_registry_path.clone(), perm)?;
    }

    Ok(node_registry_path)
}

#[cfg(windows)]
pub fn get_node_registry_path() -> Result<PathBuf> {
    let path = Path::new("C:\\ProgramData\\safenode-manager");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(path.join("node_registry.json"))
}

/// Get the data directory for the service.
///
/// It's a little counter-intuitive, but the owner will be `None` in the case of a user-mode
/// service, because it will always run as the current user. The `owner` is really to specify the
/// non-root user for running a system-wide service.
#[cfg(unix)]
pub fn get_service_data_dir_path(
    custom_path: Option<PathBuf>,
    owner: Option<String>,
) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => match owner.is_some() {
            true => PathBuf::from("/var/safenode-manager/services"),
            false => get_user_safenode_data_dir()?,
        },
    };
    if let Some(owner) = owner {
        create_owned_dir(path.clone(), &owner)?;
    }
    Ok(path)
}

#[cfg(windows)]
pub fn get_service_data_dir_path(
    custom_path: Option<PathBuf>,
    _owner: Option<String>,
) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => PathBuf::from("C:\\ProgramData\\safenode\\data"),
    };
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Get the logging directory for the service.
///
/// It's a little counter-intuitive, but the owner will be `None` in the case of a user-mode
/// service, because it will always run as the current user. The `owner` is really to specify the
/// non-root user for running a system-wide service.
#[cfg(unix)]
pub fn get_service_log_dir_path(
    bin_type: ReleaseType,
    custom_path: Option<PathBuf>,
    owner: Option<String>,
) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => match owner.is_some() {
            true => PathBuf::from("/var/log").join(bin_type.to_string()),
            false => get_user_safenode_data_dir()?,
        },
    };
    if let Some(owner) = owner {
        create_owned_dir(path.clone(), &owner)?;
    }
    Ok(path)
}

#[cfg(windows)]
pub fn get_service_log_dir_path(
    bin_type: ReleaseType,
    custom_path: Option<PathBuf>,
    _owner: Option<String>,
) -> Result<PathBuf> {
    let path = match custom_path {
        Some(p) => p,
        None => PathBuf::from("C:\\ProgramData")
            .join(bin_type.to_string())
            .join("logs"),
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
    std::fs::create_dir_all(path)?;
    Ok(())
}

#[cfg(unix)]
pub fn is_running_as_root() -> bool {
    users::get_effective_uid() == 0
}

#[cfg(windows)]
pub fn is_running_as_root() -> bool {
    // The Windows implementation for this will be much more complex.
    true
}

pub fn get_user_safenode_data_dir() -> Result<PathBuf> {
    Ok(dirs_next::data_dir()
        .ok_or_else(|| eyre!("Could not obtain user data directory"))?
        .join("safe")
        .join("node"))
}
