// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use sysinfo::Disks;

// Tries to get the default (drive name, mount point) of the current executable
// to be used as the default drive
pub fn get_default_mount_point() -> (String, String) {
    // Create a new System instance
    let disks = Disks::new_with_refreshed_list();

    // Get the current executable path
    let exe_path = env::current_exe().expect("Failed to get current executable path");

    // Iterate over the disks and find the one that matches the executable path
    for disk in disks.list() {
        if exe_path.starts_with(disk.mount_point()) {
            return (
                disk.name().to_string_lossy().into(),
                disk.mount_point().to_string_lossy().into_owned(),
            );
        }
    }
    // If no matching disk is found, return an empty string or handle the error as needed
    (String::new(), String::new())
}

// Checks if the given path (a drive) is read-only
fn is_read_only<P: AsRef<Path>>(path: P) -> bool {
    let test_file_path = path.as_ref().join("lauchpad_test_write_permission.tmp");

    // Try to create and write to a temporary file
    let result = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&test_file_path)
        .and_then(|mut file| file.write_all(b"test"));

    match result {
        Ok(_) => {
            // Clean up the test file if write was successful
            let _ = std::fs::remove_file(test_file_path);
            false
        }
        Err(err) => {
            // Check if the error is due to a read-only file system
            err.kind() == std::io::ErrorKind::PermissionDenied
        }
    }
}

// Gets a list of drives and their available space
pub fn get_list_of_drives_and_available_space() -> Vec<(String, String, u64)> {
    // Create a new System instance
    let disks = Disks::new_with_refreshed_list();

    // Get the list of disks
    let mut drives: Vec<(String, String, u64)> = Vec::new();
    for disk in disks.list() {
        // Check if the disk is already in the list
        let disk_info = (
            disk.name()
                .to_string_lossy()
                .into_owned()
                .trim()
                .to_string(),
            disk.mount_point()
                .to_string_lossy()
                .into_owned()
                .trim()
                .to_string(),
            disk.available_space(),
        );
        // We don't check for write permission on removable drives
        if !disk.is_removable() {
            // Check if the disk is read-only and skip it
            if is_read_only(disk.mount_point()) {
                continue;
            }
        }
        if !drives.contains(&disk_info) {
            drives.push(disk_info);
        }
    }
    debug!("Drives detected: {:?}", drives);
    drives
}

// Opens a folder in the file explorer
pub fn open_folder(path: &str) -> std::io::Result<()> {
    if Path::new(path).exists() {
        #[cfg(target_os = "macos")]
        Command::new("open").arg(path).spawn()?.wait()?;
        #[cfg(target_os = "windows")]
        Command::new("explorer").arg(path).spawn()?.wait()?;
        #[cfg(target_os = "linux")]
        Command::new("xdg-open").arg(path).spawn()?.wait()?;
    } else {
        error!("Path does not exist: {}", path);
    }
    Ok(())
}

#[cfg(unix)]
pub fn get_mount_point() -> PathBuf {
    PathBuf::from("/")
}
#[cfg(windows)]
pub fn get_mount_point() -> PathBuf {
    PathBuf::from("C:\\")
}
