// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::{Error, Result};

use service_manager::{
    ServiceInstallCtx, ServiceLabel, ServiceManager, ServiceStartCtx, ServiceStopCtx,
    ServiceUninstallCtx,
};
use std::net::{SocketAddr, TcpListener};
use sysinfo::{Pid, System};

/// A thin wrapper around the `service_manager::ServiceManager`, which makes our own testing
/// easier.
///
/// We can make an assumption that this external component works correctly, so our own tests only
/// need assert that the service manager is used. Testing code that used the real service manager
/// would result in real services on the machines we are testing on; that can leave a bit of a mess
/// to clean up, especially if the tests fail.
pub trait ServiceControl: Sync {
    fn create_service_user(&self, username: &str) -> Result<()>;
    fn get_available_port(&self) -> Result<u16>;
    fn install(&self, install_ctx: ServiceInstallCtx) -> Result<()>;
    fn get_process_pid(&self, name: &str) -> Result<u32>;
    fn is_service_process_running(&self, pid: u32) -> bool;
    fn start(&self, service_name: &str) -> Result<()>;
    fn stop(&self, service_name: &str) -> Result<()>;
    fn uninstall(&self, service_name: &str) -> Result<()>;
    fn wait(&self, delay: u64);
}

pub struct ServiceController {}

impl ServiceControl for ServiceController {
    #[cfg(target_os = "linux")]
    fn create_service_user(&self, username: &str) -> Result<()> {
        use std::process::Command;

        let output = Command::new("id").arg("-u").arg(username).output()?;
        if output.status.success() {
            println!("The {username} user already exists");
            return Ok(());
        }

        let useradd_exists = Command::new("which")
            .arg("useradd")
            .output()?
            .status
            .success();
        let adduser_exists = Command::new("which")
            .arg("adduser")
            .output()?
            .status
            .success();

        let output = if useradd_exists {
            Command::new("useradd")
                .arg("-m")
                .arg("-s")
                .arg("/bin/bash")
                .arg(username)
                .output()?
        } else if adduser_exists {
            Command::new("adduser")
                .arg("-s")
                .arg("/bin/busybox")
                .arg("-D")
                .arg(username)
                .output()?
        } else {
            return Err(Error::ServiceUserAccountCreationFailed);
        };

        if !output.status.success() {
            return Err(Error::ServiceUserAccountCreationFailed);
        }
        println!("Created {username} user account for running the service");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn create_service_user(&self, username: &str) -> Result<()> {
        use std::process::Command;
        use std::str;

        let output = Command::new("dscl")
            .arg(".")
            .arg("-list")
            .arg("/Users")
            .output()?;
        let output_str = str::from_utf8(&output.stdout)?;
        if output_str.lines().any(|line| line == username) {
            return Ok(());
        }

        let output = Command::new("dscl")
            .arg(".")
            .arg("-list")
            .arg("/Users")
            .arg("UniqueID")
            .output()?;
        let output_str = str::from_utf8(&output.stdout)?;
        let mut max_id = 0;

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                if let Ok(id) = parts[1].parse::<u32>() {
                    if id > max_id {
                        max_id = id;
                    }
                }
            }
        }
        let new_unique_id = max_id + 1;

        let commands = vec![
            format!("dscl . -create /Users/{}", username),
            format!(
                "dscl . -create /Users/{} UserShell /usr/bin/false",
                username
            ),
            format!(
                "dscl . -create /Users/{} UniqueID {}",
                username, new_unique_id
            ),
            format!("dscl . -create /Users/{} PrimaryGroupID 20", username),
        ];
        for cmd in commands {
            let status = Command::new("sh").arg("-c").arg(&cmd).status()?;
            if !status.success() {
                return Err(Error::ServiceUserAccountCreationFailed);
            }
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn create_service_user(&self, _username: &str) -> Result<()> {
        Ok(())
    }

    fn is_service_process_running(&self, pid: u32) -> bool {
        let mut system = System::new_all();
        system.refresh_all();
        system.process(Pid::from(pid as usize)).is_some()
    }

    fn get_available_port(&self) -> Result<u16> {
        let addr: SocketAddr = "127.0.0.1:0".parse()?;

        let socket = TcpListener::bind(addr)?;
        let port = socket.local_addr()?.port();
        drop(socket);

        Ok(port)
    }

    fn get_process_pid(&self, name: &str) -> Result<u32> {
        let mut system = System::new_all();
        system.refresh_all();
        for (pid, process) in system.processes() {
            if process.name() == name {
                // There does not seem to be any easy way to get the process ID from the `Pid`
                // type. Probably something to do with representing it in a cross-platform way.
                return Ok(pid.to_string().parse::<u32>()?);
            }
        }
        Err(Error::ServiceProcessNotFound(name.to_string()))
    }

    fn install(&self, install_ctx: ServiceInstallCtx) -> Result<()> {
        let manager = <dyn ServiceManager>::native()?;
        manager.install(install_ctx)?;
        Ok(())
    }

    fn start(&self, service_name: &str) -> Result<()> {
        let label: ServiceLabel = service_name.parse()?;
        let manager = <dyn ServiceManager>::native()?;
        manager.start(ServiceStartCtx { label })?;
        Ok(())
    }

    fn stop(&self, service_name: &str) -> Result<()> {
        let label: ServiceLabel = service_name.parse()?;
        let manager = <dyn ServiceManager>::native()?;
        manager.stop(ServiceStopCtx { label })?;
        Ok(())
    }

    fn uninstall(&self, service_name: &str) -> Result<()> {
        let label: ServiceLabel = service_name.parse()?;
        let manager = <dyn ServiceManager>::native()?;
        match manager.uninstall(ServiceUninstallCtx { label }) {
            Ok(()) => Ok(()),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    // In this case the user has removed the service definition file manually,
                    // which the service manager crate treats as an error. We can propagate the
                    // it to the caller and they can decide how to handle it.
                    Err(Error::ServiceRemovedManually(service_name.to_string()))
                }
                _ => Err(e.into()),
            },
        }
        // Ok(())
    }

    /// Provide a delay for the service to start or stop.
    ///
    /// This is wrapped mainly just for unit testing.
    fn wait(&self, delay: u64) {
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }
}
