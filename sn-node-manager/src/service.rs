use color_eyre::Result;
#[cfg(test)]
use mockall::automock;
use service_manager::{ServiceInstallCtx, ServiceLabel, ServiceManager};
use std::ffi::OsString;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;

/// A thin wrapper around the `service_manager::ServiceManager`, which makes our own testing
/// easier.
///
/// We can make an assumption that this external component works correctly, so our own tests only
/// need assert that the service manager is used. Testing code that used the real service manager
/// would result in real services on the machines we are testing on; that can leave a bit of a mess
/// to clean up, especially if the tests fail.
#[cfg_attr(test, automock)]
pub trait ServiceControl {
    fn create_service_user(&self, username: &str) -> Result<()>;
    fn get_available_port(&self) -> Result<u16>;
    fn install(
        &self,
        name: &str,
        executable_path: &Path,
        node_port: u16,
        rpc_port: u16,
        service_user: &str,
    ) -> Result<()>;
}

pub struct NodeServiceManager {}

impl ServiceControl for NodeServiceManager {
    #[cfg(target_os = "linux")]
    fn create_service_user(&self, username: &str) -> Result<()> {
        use color_eyre::eyre::eyre;
        use std::process::Command;

        if Command::new("id").arg("-u").arg(username).output().is_ok() {
            println!("The {username} user already exists");
            return Ok(());
        }
        let output = Command::new("useradd")
            .arg("-m")
            .arg("-s")
            .arg("/bin/bash")
            .arg(username)
            .output()?;
        if !output.status.success() {
            return Err(eyre!("Failed to create user account"));
        }
        println!("Created {username} user account for running the service");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn create_service_user(&self, username: &str) -> Result<()> {
        use color_eyre::eyre::eyre;
        use std::process::Command;
        use std::str;

        let output = Command::new("dscl")
            .arg(".")
            .arg("-list")
            .arg("/Users")
            .output()
            .unwrap();
        let output_str = str::from_utf8(&output.stdout).unwrap();
        if output_str.lines().any(|line| line == username) {
            return Ok(());
        }

        let output = Command::new("dscl")
            .arg(".")
            .arg("-list")
            .arg("/Users")
            .arg("UniqueID")
            .output()
            .unwrap();
        let output_str = str::from_utf8(&output.stdout).unwrap();
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
                return Err(eyre!("Failed to create service user account"));
            }
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn create_service_user(&self, _username: &str) -> Result<()> {
        Ok(())
    }

    fn get_available_port(&self) -> Result<u16> {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        Ok(TcpListener::bind(addr)?.local_addr()?.port())
    }

    fn install(
        &self,
        name: &str,
        safenode_path: &Path,
        node_port: u16,
        rpc_port: u16,
        service_user: &str,
    ) -> Result<()> {
        let label: ServiceLabel = name.parse()?;
        let manager = <dyn ServiceManager>::native()?;
        manager.install(ServiceInstallCtx {
            label: label.clone(),
            program: safenode_path.to_path_buf(),
            args: vec![
                OsString::from("--port"),
                OsString::from(node_port.to_string()),
                OsString::from("--rpc"),
                OsString::from(format!("127.0.0.1:{rpc_port}")),
            ],
            contents: None,
            username: Some(service_user.to_string()),
        })?;
        Ok(())
    }
}
