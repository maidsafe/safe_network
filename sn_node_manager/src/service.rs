// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
use libp2p::Multiaddr;
#[cfg(test)]
use mockall::automock;
use service_manager::{
    ServiceInstallCtx, ServiceLabel, ServiceManager, ServiceStartCtx, ServiceStopCtx,
    ServiceUninstallCtx,
};
use std::{
    ffi::OsString,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
};
use sysinfo::{Pid, System, SystemExt};

#[derive(Debug, PartialEq)]
pub struct InstallNodeServiceConfig {
    pub data_dir_path: PathBuf,
    pub genesis: bool,
    pub local: bool,
    pub log_dir_path: PathBuf,
    pub name: String,
    pub node_port: Option<u16>,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub rpc_socket_addr: SocketAddr,
    pub safenode_path: PathBuf,
    pub service_user: String,
    pub env_variables: Option<Vec<(String, String)>>,
}

impl InstallNodeServiceConfig {
    pub fn build_service_install_ctx(self) -> Result<ServiceInstallCtx> {
        let label: ServiceLabel = self.name.parse()?;
        let mut args = vec![
            OsString::from("--rpc"),
            OsString::from(self.rpc_socket_addr.to_string()),
            OsString::from("--root-dir"),
            OsString::from(self.data_dir_path.to_string_lossy().to_string()),
            OsString::from("--log-output-dest"),
            OsString::from(self.log_dir_path.to_string_lossy().to_string()),
        ];

        if self.genesis {
            args.push(OsString::from("--first"));
        }
        if self.local {
            args.push(OsString::from("--local"));
        }
        if let Some(node_port) = self.node_port {
            args.push(OsString::from("--port"));
            args.push(OsString::from(node_port.to_string()));
        }

        if !self.bootstrap_peers.is_empty() {
            let peers_str = self
                .bootstrap_peers
                .iter()
                .map(|peer| peer.to_string())
                .collect::<Vec<_>>()
                .join(",");
            args.push(OsString::from("--peer"));
            args.push(OsString::from(peers_str));
        }

        let mut service_ctx = ServiceInstallCtx {
            label: label.clone(),
            program: self.safenode_path.to_path_buf(),
            args,
            contents: None,
            username: Some(self.service_user.to_string()),
            working_directory: None,
            environment: self.env_variables,
        };
        // Temporary fix to enable the restart cmd to properly restart a running service.
        // 'ServiceInstallCtx::content' will override the other passed in fields.
        #[cfg(target_os = "linux")]
        {
            use std::fmt::Write;
            let mut service = String::new();

            let _ = writeln!(service, "[Unit]");
            let _ = writeln!(
                service,
                "Description={}",
                service_ctx.label.to_script_name()
            );
            let _ = writeln!(service, "[Service]");
            let program = service_ctx.program.to_string_lossy();
            let args = service_ctx
                .args
                .clone()
                .into_iter()
                .map(|a| a.to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            let _ = writeln!(service, "ExecStart={program} {args}");
            if let Some(env_vars) = &service_ctx.environment {
                for (var, val) in env_vars {
                    let _ = writeln!(service, "Environment=\"{}={}\"", var, val);
                }
            }
            let _ = writeln!(service, "Restart=on-failure");
            let _ = writeln!(service, "User={}", self.service_user);
            let _ = writeln!(service, "KillMode=process"); // fixes the restart issue
            let _ = writeln!(service, "[Install]");
            let _ = writeln!(service, "WantedBy=multi-user.target");

            service_ctx.contents = Some(service);
        }
        #[cfg(not(target_os = "linux"))]
        {
            service_ctx.contents = None;
        }
        Ok(service_ctx)
    }
}

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
    fn install(&self, install_ctx: ServiceInstallCtx) -> Result<()>;
    fn is_service_process_running(&self, pid: u32) -> bool;
    fn start(&self, service_name: &str) -> Result<()>;
    fn stop(&self, service_name: &str) -> Result<()>;
    fn uninstall(&self, service_name: &str) -> Result<()>;
    fn wait(&self, delay: u64);
}

pub struct NodeServiceManager {}

impl ServiceControl for NodeServiceManager {
    #[cfg(target_os = "linux")]
    fn create_service_user(&self, username: &str) -> Result<()> {
        use color_eyre::eyre::eyre;
        use std::process::Command;

        let output = Command::new("id").arg("-u").arg(username).output()?;
        if output.status.success() {
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
                return Err(eyre!("Failed to create service user account"));
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
        manager.uninstall(ServiceUninstallCtx { label })?;
        Ok(())
    }

    /// Provide a delay for the service to start or stop.
    ///
    /// This is wrapped mainly just for unit testing.
    fn wait(&self, delay: u64) {
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }
}
