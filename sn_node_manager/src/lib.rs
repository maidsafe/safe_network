// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod add_services;
pub mod cmd;
pub mod config;
pub mod helpers;
pub mod local;
pub mod rpc;

#[derive(Clone, PartialEq)]
pub enum VerbosityLevel {
    Minimal,
    Normal,
    Full,
}

impl From<u8> for VerbosityLevel {
    fn from(verbosity: u8) -> Self {
        match verbosity {
            1 => VerbosityLevel::Minimal,
            2 => VerbosityLevel::Normal,
            3 => VerbosityLevel::Full,
            _ => VerbosityLevel::Normal,
        }
    }
}

use color_eyre::{
    eyre::{eyre, OptionExt},
    Help, Result,
};
use colored::Colorize;
use semver::Version;
use sn_service_management::{
    control::ServiceControl,
    error::Error as ServiceError,
    rpc::{RpcActions, RpcClient},
    NodeRegistry, NodeServiceData, ServiceStateActions, ServiceStatus, UpgradeOptions,
    UpgradeResult,
};
use sn_transfers::HotWallet;
use tracing::debug;

pub const DAEMON_DEFAULT_PORT: u16 = 12500;
pub const DAEMON_SERVICE_NAME: &str = "safenodemand";
const RPC_START_UP_DELAY_MS: u64 = 3000;

pub struct ServiceManager<T: ServiceStateActions + Send> {
    pub service: T,
    pub service_control: Box<dyn ServiceControl + Send>,
    pub verbosity: VerbosityLevel,
}

impl<T: ServiceStateActions + Send> ServiceManager<T> {
    pub fn new(
        service: T,
        service_control: Box<dyn ServiceControl + Send>,
        verbosity: VerbosityLevel,
    ) -> Self {
        ServiceManager {
            service,
            service_control,
            verbosity,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if ServiceStatus::Running == self.service.status() {
            // The last time we checked the service was running, but it doesn't mean it's actually
            // running at this point in time. If it is running, we don't need to do anything. If it
            // stopped because of a fault, we will drop to the code below and attempt to start it
            // again.
            if self
                .service_control
                .is_service_process_running(self.service.pid().unwrap())
            {
                println!("The {} service is already running", self.service.name());
                return Ok(());
            }
        }

        // At this point the service either hasn't been started for the first time or it has been
        // stopped. If it was stopped, it was either intentional or because it crashed.
        if self.verbosity != VerbosityLevel::Minimal {
            println!("Attempting to start {}...", self.service.name());
        }
        self.service_control.start(&self.service.name())?;
        self.service_control.wait(RPC_START_UP_DELAY_MS);

        // This is an attempt to see whether the service process has actually launched. You don't
        // always get an error from the service infrastructure.
        //
        // There might be many different `safenode` processes running, but since each service has
        // its own isolated binary, we use the binary path to uniquely identify it.
        match self
            .service_control
            .get_process_pid(&self.service.bin_path())
        {
            Ok(pid) => {
                debug!(
                    "Service process started for {} with PID {}",
                    self.service.name(),
                    pid
                );
            }
            Err(sn_service_management::error::Error::ServiceProcessNotFound(_)) => {
                return Err(eyre!(
                    "The '{}' service has failed to start",
                    self.service.name()
                ));
            }
            Err(e) => return Err(e.into()),
        }

        self.service.on_start().await?;

        println!("{} Started {} service", "✓".green(), self.service.name());
        if self.verbosity != VerbosityLevel::Minimal {
            println!(
                "  - PID: {}",
                self.service
                    .pid()
                    .map_or("-".to_string(), |p| p.to_string())
            );
            println!(
                "  - Bin path: {}",
                self.service.bin_path().to_string_lossy()
            );
            println!(
                "  - Data path: {}",
                self.service.data_dir_path().to_string_lossy()
            );
            println!(
                "  - Logs path: {}",
                self.service.log_dir_path().to_string_lossy()
            );
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        match self.service.status() {
            ServiceStatus::Added => {
                println!(
                    "Service {} has not been started since it was installed",
                    self.service.name()
                );
                Ok(())
            }
            ServiceStatus::Removed => {
                println!("Service {} has been removed", self.service.name());
                Ok(())
            }
            ServiceStatus::Running => {
                let pid = self.service.pid().ok_or_eyre("The PID was not set")?;
                let name = self.service.name();

                if self.service_control.is_service_process_running(pid) {
                    println!("Attempting to stop {}...", name);
                    self.service_control.stop(&name)?;
                    println!(
                        "{} Service {} with PID {} was stopped",
                        "✓".green(),
                        name,
                        pid
                    );
                } else {
                    println!("{} Service {} was already stopped", "✓".green(), name);
                }

                self.service.on_stop().await?;
                Ok(())
            }
            ServiceStatus::Stopped => {
                println!(
                    "{} Service {} was already stopped",
                    "✓".green(),
                    self.service.name()
                );
                Ok(())
            }
        }
    }

    pub async fn remove(&mut self, keep_directories: bool) -> Result<()> {
        if let ServiceStatus::Running = self.service.status() {
            if self.service_control.is_service_process_running(
                self.service
                    .pid()
                    .ok_or_eyre("Could not obtain PID for running node")?,
            ) {
                return Err(eyre!("A running service cannot be removed")
                    .suggestion("Stop the node first then try again"));
            }
            // If the node wasn't actually running, we should give the user an opportunity to
            // check why it may have failed before removing everything.
            self.service.on_stop().await?;
            return Err(
                eyre!("This service was marked as running but it had actually stopped")
                    .suggestion("You may want to check the logs for errors before removing it")
                    .suggestion("To remove the service, run the command again."),
            );
        }

        match self.service_control.uninstall(&self.service.name()) {
            Ok(()) => {}
            Err(e) => match e {
                ServiceError::ServiceRemovedManually(name) => {
                    // The user has deleted the service definition file, which the service manager
                    // crate treats as an error. We then return our own error type, which allows us
                    // to handle it here and just proceed with removing the service from the
                    // registry.
                    println!("The user appears to have removed the {name} service manually");
                }
                _ => return Err(e.into()),
            },
        }

        if !keep_directories {
            // It's possible the user deleted either of these directories manually.
            // We can just proceed with removing the service from the registry.
            if self.service.data_dir_path().exists() {
                std::fs::remove_dir_all(self.service.data_dir_path())?;
            }
            if self.service.log_dir_path().exists() {
                std::fs::remove_dir_all(self.service.log_dir_path())?;
            }
        }

        self.service.on_remove();

        println!(
            "{} Service {} was removed",
            "✓".green(),
            self.service.name()
        );

        Ok(())
    }

    pub async fn upgrade(&mut self, options: UpgradeOptions) -> Result<UpgradeResult> {
        let current_version = Version::parse(&self.service.version())?;
        if !options.force
            && (current_version == options.target_version
                || options.target_version < current_version)
        {
            return Ok(UpgradeResult::NotRequired);
        }

        self.stop().await?;
        std::fs::copy(options.clone().target_bin_path, self.service.bin_path())?;

        self.service_control.uninstall(&self.service.name())?;
        self.service_control.install(
            self.service
                .build_upgrade_install_context(options.clone())?,
        )?;

        if options.start_service {
            match self.start().await {
                Ok(()) => {}
                Err(e) => {
                    self.service
                        .set_version(&options.target_version.to_string());
                    return Ok(UpgradeResult::UpgradedButNotStarted(
                        current_version.to_string(),
                        options.target_version.to_string(),
                        e.to_string(),
                    ));
                }
            }
        }
        self.service
            .set_version(&options.target_version.to_string());

        match options.force {
            true => Ok(UpgradeResult::Forced(
                current_version.to_string(),
                options.target_version.to_string(),
            )),
            false => Ok(UpgradeResult::Upgraded(
                current_version.to_string(),
                options.target_version.to_string(),
            )),
        }
    }
}

pub async fn status_report(
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    detailed_view: bool,
    output_json: bool,
    fail: bool,
) -> Result<()> {
    refresh_node_registry(node_registry, service_control, !output_json).await?;

    if output_json {
        let json = serde_json::to_string_pretty(&node_registry.to_status_summary())?;
        println!("{json}");
    } else if detailed_view {
        for node in &node_registry.nodes {
            print_banner(&node.service_name, &node.status);
            println!("Version: {}", node.version);
            println!(
                "Peer ID: {}",
                node.peer_id.map_or("-".to_string(), |p| p.to_string())
            );
            println!("RPC Socket: {}", node.rpc_socket_addr);
            println!("Listen Addresses: {:?}", node.listen_addr);
            println!(
                "PID: {}",
                node.pid.map_or("-".to_string(), |p| p.to_string())
            );
            println!("Data path: {}", node.data_dir_path.to_string_lossy());
            println!("Log path: {}", node.log_dir_path.to_string_lossy());
            println!("Bin path: {}", node.safenode_path.to_string_lossy());
            println!(
                "Connected peers: {}",
                node.connected_peers
                    .as_ref()
                    .map_or("-".to_string(), |p| p.len().to_string())
            );
            let wallet = HotWallet::load_from(&node.data_dir_path)?;
            println!("Reward balance: {}", wallet.balance());
            println!();
        }

        if let Some(daemon) = &node_registry.daemon {
            print_banner(&daemon.service_name, &daemon.status);
            println!("Version: {}", daemon.version);
            println!("Bin path: {}", daemon.daemon_path.to_string_lossy());
        }

        if let Some(faucet) = &node_registry.faucet {
            print_banner(&faucet.service_name, &faucet.status);
            println!("Version: {}", faucet.version);
            println!("Bin path: {}", faucet.faucet_path.to_string_lossy());
            println!("Log path: {}", faucet.log_dir_path.to_string_lossy());
        }
    } else {
        println!(
            "{:<18} {:<52} {:<7} {:>15}",
            "Service Name", "Peer ID", "Status", "Connected Peers"
        );
        let nodes = node_registry
            .nodes
            .iter()
            .filter(|n| n.status != ServiceStatus::Removed)
            .collect::<Vec<&NodeServiceData>>();
        for node in nodes {
            let peer_id = node.peer_id.map_or("-".to_string(), |p| p.to_string());
            let connected_peers = node
                .connected_peers
                .clone()
                .map_or("-".to_string(), |p| p.len().to_string());
            println!(
                "{:<18} {:<52} {:<7} {:>15}",
                node.service_name,
                peer_id,
                format_status(&node.status),
                connected_peers
            );
        }
        if let Some(daemon) = &node_registry.daemon {
            println!(
                "{:<18} {:<52} {:<7} {:>15}",
                daemon.service_name,
                "-",
                format_status(&daemon.status),
                "-"
            );
        }
        if let Some(faucet) = &node_registry.faucet {
            println!(
                "{:<18} {:<52} {:<7} {:>15}",
                faucet.service_name,
                "-",
                format_status(&faucet.status),
                "-"
            );
        }
    }

    if fail
        && node_registry
            .nodes
            .iter()
            .any(|n| n.status != ServiceStatus::Running)
    {
        return Err(eyre!("One or more nodes are not in a running state"));
    }

    Ok(())
}

pub async fn refresh_node_registry(
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    print_refresh_message: bool,
) -> Result<()> {
    // This message is useful for users, but needs to be suppressed when a JSON output is requested.
    if print_refresh_message {
        println!("Refreshing the node registry...");
    }

    for node in &mut node_registry.nodes {
        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        if let ServiceStatus::Running = node.status {
            if let Some(pid) = node.pid {
                // First we can try the PID we have now. If there is still a process running with
                // that PID, we know the node is still running.
                if service_control.is_service_process_running(pid) {
                    match rpc_client.network_info().await {
                        Ok(info) => {
                            node.connected_peers = Some(info.connected_peers);
                        }
                        Err(_) => {
                            node.connected_peers = None;
                        }
                    }
                } else {
                    // The process with the PID we had has died at some point. However, if the
                    // service has been configured to restart on failures, it's possible that a new
                    // process has been launched and hence we would have a new PID. We can use the
                    // RPC service to try and retrieve it.
                    match rpc_client.node_info().await {
                        Ok(info) => {
                            node.pid = Some(info.pid);
                        }
                        Err(_) => {
                            // Finally, if there was an error communicating with the RPC client, we
                            // can assume that this node is actually stopped.
                            node.status = ServiceStatus::Stopped;
                            node.pid = None;
                        }
                    }
                    match rpc_client.network_info().await {
                        Ok(info) => {
                            node.connected_peers = Some(info.connected_peers);
                        }
                        Err(_) => {
                            node.connected_peers = None;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn format_status(status: &ServiceStatus) -> String {
    match status {
        ServiceStatus::Running => "RUNNING".green().to_string(),
        ServiceStatus::Stopped => "STOPPED".red().to_string(),
        ServiceStatus::Added => "ADDED".yellow().to_string(),
        ServiceStatus::Removed => "REMOVED".red().to_string(),
    }
}

fn print_banner(service_name: &str, status: &ServiceStatus) {
    let service_status = format!("{} - {}", service_name, format_status(status));
    let banner = "=".repeat(service_status.len());
    println!("{}", banner);
    println!("{service_status}");
    println!("{}", banner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use libp2p_identity::PeerId;
    use mockall::{mock, predicate::*};
    use predicates::prelude::*;
    use service_manager::ServiceInstallCtx;
    use sn_service_management::{
        error::{Error as ServiceControlError, Result as ServiceControlResult},
        node::{NodeService, NodeServiceData},
        rpc::{NetworkInfo, NodeInfo, RecordAddress, RpcActions},
        UpgradeOptions, UpgradeResult,
    };
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::{Path, PathBuf},
        str::FromStr,
    };

    mock! {
        pub RpcClient {}
        #[async_trait]
        impl RpcActions for RpcClient {
            async fn node_info(&self) -> ServiceControlResult<NodeInfo>;
            async fn network_info(&self) -> ServiceControlResult<NetworkInfo>;
            async fn record_addresses(&self) -> ServiceControlResult<Vec<RecordAddress>>;
            async fn node_restart(&self, delay_millis: u64, retain_peer_id: bool) -> ServiceControlResult<()>;
            async fn node_stop(&self, delay_millis: u64) -> ServiceControlResult<()>;
            async fn node_update(&self, delay_millis: u64) -> ServiceControlResult<()>;
            async fn update_log_level(&self, log_levels: String) -> ServiceControlResult<()>;
        }
    }

    mock! {
        pub ServiceControl {}
        impl ServiceControl for ServiceControl {
            fn create_service_user(&self, username: &str) -> ServiceControlResult<()>;
            fn get_available_port(&self) -> ServiceControlResult<u16>;
            fn install(&self, install_ctx: ServiceInstallCtx) -> ServiceControlResult<()>;
            fn get_process_pid(&self, bin_path: &Path) -> ServiceControlResult<u32>;
            fn is_service_process_running(&self, pid: u32) -> bool;
            fn start(&self, service_name: &str) -> ServiceControlResult<()>;
            fn stop(&self, service_name: &str) -> ServiceControlResult<()>;
            fn uninstall(&self, service_name: &str) -> ServiceControlResult<()>;
            fn wait(&self, delay: u64);
        }
    }

    #[tokio::test]
    async fn start_should_start_a_newly_installed_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(PathBuf::from(
                "/var/safenode-manager/services/safenode1/safenode",
            )))
            .times(1)
            .returning(|_| Ok(100));
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 1000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                data_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
                log_path: PathBuf::from("/var/log/safenode/safenode1"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });
        mock_rpc_client
            .expect_network_info()
            .times(1)
            .returning(|| {
                Ok(NetworkInfo {
                    connected_peers: Vec::new(),
                    listeners: Vec::new(),
                })
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: None,
            pid: None,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Added,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.start().await?;

        assert_eq!(service_manager.service.service_data.pid, Some(1000));
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
            )?)
        );
        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Running
        );

        Ok(())
    }

    #[tokio::test]
    async fn start_should_start_a_stopped_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(PathBuf::from(
                "/var/safenode-manager/services/safenode1/safenode",
            )))
            .times(1)
            .returning(|_| Ok(100));
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 1000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                data_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
                log_path: PathBuf::from("/var/log/safenode/safenode1"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });
        mock_rpc_client
            .expect_network_info()
            .times(1)
            .returning(|| {
                Ok(NetworkInfo {
                    connected_peers: Vec::new(),
                    listeners: Vec::new(),
                })
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: None,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Stopped,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.start().await?;

        assert_eq!(service_manager.service.service_data.pid, Some(1000));
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
            )?)
        );
        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Running
        );

        Ok(())
    }

    #[tokio::test]
    async fn start_should_not_attempt_to_start_a_running_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.start().await?;

        assert_eq!(service_manager.service.service_data.pid, Some(1000));
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
            )?)
        );
        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Running
        );

        Ok(())
    }

    #[tokio::test]
    async fn start_should_start_a_service_marked_as_running_but_had_since_stopped() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| false);
        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(PathBuf::from(
                "/var/safenode-manager/services/safenode1/safenode",
            )))
            .times(1)
            .returning(|_| Ok(100));
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 1000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                data_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
                log_path: PathBuf::from("/var/log/safenode/safenode1"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });
        mock_rpc_client
            .expect_network_info()
            .times(1)
            .returning(|| {
                Ok(NetworkInfo {
                    connected_peers: Vec::new(),
                    listeners: Vec::new(),
                })
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.start().await?;

        assert_eq!(service_manager.service.service_data.pid, Some(1000));
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
            )?)
        );
        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Running
        );

        Ok(())
    }

    #[tokio::test]
    async fn start_should_return_an_error_if_the_process_was_not_found() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();

        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(PathBuf::from(
                "/var/safenode-manager/services/safenode1/safenode",
            )))
            .times(1)
            .returning(|_| {
                Err(ServiceControlError::ServiceProcessNotFound(
                    "/var/safenode-manager/services/safenode1/safenode".to_string(),
                ))
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: None,
            pid: None,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Added,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let result = service_manager.start().await;
        match result {
            Ok(_) => panic!("This test should have resulted in an error"),
            Err(e) => assert_eq!("The 'safenode1' service has failed to start", e.to_string()),
        }

        Ok(())
    }

    #[tokio::test]
    async fn stop_should_stop_a_running_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();

        mock_service_control
            .expect_stop()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.stop().await?;

        assert_eq!(service_manager.service.service_data.pid, None);
        assert_eq!(service_manager.service.service_data.connected_peers, None);
        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Stopped
        );
        Ok(())
    }

    #[tokio::test]
    async fn stop_should_not_return_error_for_attempt_to_stop_installed_service() -> Result<()> {
        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: None,
            pid: None,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Added,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(MockServiceControl::new()),
            VerbosityLevel::Normal,
        );

        let result = service_manager.stop().await;

        match result {
            Ok(()) => Ok(()),
            Err(_) => {
                panic!("The stop command should be idempotent and do nothing for an added service");
            }
        }
    }

    #[tokio::test]
    async fn stop_should_return_ok_when_attempting_to_stop_service_that_was_already_stopped(
    ) -> Result<()> {
        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: None,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Stopped,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(MockServiceControl::new()),
            VerbosityLevel::Normal,
        );

        let result = service_manager.stop().await;

        match result {
            Ok(()) => Ok(()),
            Err(_) => {
                panic!(
                    "The stop command should be idempotent and do nothing for an stopped service"
                );
            }
        }
    }

    #[tokio::test]
    async fn stop_should_return_ok_when_attempting_to_stop_a_removed_service() -> Result<()> {
        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: None,
            pid: None,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Removed,
            user: "safe".to_string(),
            version: "0.98.1".to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(MockServiceControl::new()),
            VerbosityLevel::Normal,
        );

        let result = service_manager.stop().await;

        match result {
            Ok(()) => Ok(()),
            Err(_) => {
                panic!(
                    "The stop command should be idempotent and do nothing for a removed service"
                );
            }
        }
    }

    #[tokio::test]
    async fn upgrade_should_upgrade_a_service_to_a_new_version() -> Result<()> {
        let current_version = "0.1.0";
        let target_version = "0.2.0";

        let tmp_data_dir = assert_fs::TempDir::new()?;
        let current_install_dir = tmp_data_dir.child("safenode_install");
        current_install_dir.create_dir_all()?;

        let current_node_bin = current_install_dir.child("safenode");
        current_node_bin.write_binary(b"fake safenode binary")?;
        let target_node_bin = tmp_data_dir.child("safenode");
        target_node_bin.write_binary(b"fake safenode binary")?;

        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        // before binary upgrade
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);
        mock_service_control
            .expect_stop()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));

        // after binary upgrade
        mock_service_control
            .expect_uninstall()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_install()
            .with(always())
            .times(1)
            .returning(|_| Ok(()));

        // after service restart
        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(current_node_bin.to_path_buf().clone()))
            .times(1)
            .returning(|_| Ok(100));
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 2000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                data_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
                log_path: PathBuf::from("/var/log/safenode/safenode1"),
                version: target_version.to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });
        mock_rpc_client
            .expect_network_info()
            .times(1)
            .returning(|| {
                Ok(NetworkInfo {
                    connected_peers: Vec::new(),
                    listeners: Vec::new(),
                })
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: current_node_bin.to_path_buf(),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: current_version.to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let upgrade_result = service_manager
            .upgrade(UpgradeOptions {
                bootstrap_peers: Vec::new(),
                env_variables: None,
                force: false,
                start_service: true,
                target_bin_path: target_node_bin.to_path_buf(),
                target_version: Version::parse(target_version).unwrap(),
            })
            .await?;

        match upgrade_result {
            UpgradeResult::Upgraded(old_version, new_version) => {
                assert_eq!(old_version, current_version);
                assert_eq!(new_version, target_version);
            }
            _ => panic!(
                "Expected UpgradeResult::Upgraded but was {:#?}",
                upgrade_result
            ),
        }

        assert_eq!(service_manager.service.service_data.pid, Some(2000));
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?)
        );
        assert_eq!(service_manager.service.service_data.version, target_version);

        Ok(())
    }

    #[tokio::test]
    async fn upgrade_should_not_be_required_if_target_is_less_than_current_version() -> Result<()> {
        let current_version = "0.2.0";
        let target_version = "0.1.0";

        let tmp_data_dir = assert_fs::TempDir::new()?;
        let current_install_dir = tmp_data_dir.child("safenode_install");
        current_install_dir.create_dir_all()?;

        let current_node_bin = current_install_dir.child("safenode");
        current_node_bin.write_binary(b"fake safenode binary")?;
        let target_node_bin = tmp_data_dir.child("safenode");
        target_node_bin.write_binary(b"fake safenode binary")?;

        let mock_service_control = MockServiceControl::new();
        let mock_rpc_client = MockRpcClient::new();

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: current_node_bin.to_path_buf(),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: current_version.to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let upgrade_result = service_manager
            .upgrade(UpgradeOptions {
                bootstrap_peers: Vec::new(),
                env_variables: None,
                force: false,
                start_service: true,
                target_bin_path: target_node_bin.to_path_buf(),
                target_version: Version::parse(target_version).unwrap(),
            })
            .await?;

        assert_matches!(upgrade_result, UpgradeResult::NotRequired);

        Ok(())
    }

    #[tokio::test]
    async fn upgrade_should_downgrade_to_a_previous_version_if_force_is_used() -> Result<()> {
        let current_version = "0.1.0";
        let target_version = "0.2.0";

        let tmp_data_dir = assert_fs::TempDir::new()?;
        let current_install_dir = tmp_data_dir.child("safenode_install");
        current_install_dir.create_dir_all()?;

        let current_node_bin = current_install_dir.child("safenode");
        current_node_bin.write_binary(b"fake safenode binary")?;
        let target_node_bin = tmp_data_dir.child("safenode");
        target_node_bin.write_binary(b"fake safenode binary")?;

        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        // before binary upgrade
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);
        mock_service_control
            .expect_stop()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));

        // after binary upgrade
        mock_service_control
            .expect_uninstall()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_install()
            .with(always())
            .times(1)
            .returning(|_| Ok(()));

        // after service restart
        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(current_node_bin.to_path_buf().clone()))
            .times(1)
            .returning(|_| Ok(100));
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 2000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                data_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
                log_path: PathBuf::from("/var/log/safenode/safenode1"),
                version: target_version.to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });
        mock_rpc_client
            .expect_network_info()
            .times(1)
            .returning(|| {
                Ok(NetworkInfo {
                    connected_peers: Vec::new(),
                    listeners: Vec::new(),
                })
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: current_node_bin.to_path_buf(),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: current_version.to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let upgrade_result = service_manager
            .upgrade(UpgradeOptions {
                bootstrap_peers: Vec::new(),
                env_variables: None,
                force: true,
                start_service: true,
                target_bin_path: target_node_bin.to_path_buf(),
                target_version: Version::parse(target_version).unwrap(),
            })
            .await?;

        match upgrade_result {
            UpgradeResult::Forced(old_version, new_version) => {
                assert_eq!(old_version, current_version);
                assert_eq!(new_version, target_version);
            }
            _ => panic!(
                "Expected UpgradeResult::Forced but was {:#?}",
                upgrade_result
            ),
        }

        assert_eq!(service_manager.service.service_data.pid, Some(2000));
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?)
        );
        assert_eq!(service_manager.service.service_data.version, target_version);

        Ok(())
    }

    #[tokio::test]
    async fn upgrade_should_upgrade_and_not_start_the_service() -> Result<()> {
        let current_version = "0.1.0";
        let target_version = "0.2.0";

        let tmp_data_dir = assert_fs::TempDir::new()?;
        let current_install_dir = tmp_data_dir.child("safenode_install");
        current_install_dir.create_dir_all()?;

        let current_node_bin = current_install_dir.child("safenode");
        current_node_bin.write_binary(b"fake safenode binary")?;
        let target_node_bin = tmp_data_dir.child("safenode");
        target_node_bin.write_binary(b"fake safenode binary")?;

        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        // before binary upgrade
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);
        mock_service_control
            .expect_stop()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));

        // after binary upgrade
        mock_service_control
            .expect_uninstall()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_install()
            .with(always())
            .times(1)
            .returning(|_| Ok(()));

        // after service restart
        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(0)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(0)
            .returning(|_| ());
        mock_rpc_client.expect_node_info().times(0).returning(|| {
            Ok(NodeInfo {
                pid: 2000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                data_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
                log_path: PathBuf::from("/var/log/safenode/safenode1"),
                version: target_version.to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });
        mock_rpc_client
            .expect_network_info()
            .times(0)
            .returning(|| {
                Ok(NetworkInfo {
                    connected_peers: Vec::new(),
                    listeners: Vec::new(),
                })
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: current_node_bin.to_path_buf(),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: current_version.to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(mock_rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let upgrade_result = service_manager
            .upgrade(UpgradeOptions {
                bootstrap_peers: Vec::new(),
                env_variables: None,
                force: false,
                start_service: false,
                target_bin_path: target_node_bin.to_path_buf(),
                target_version: Version::parse(target_version).unwrap(),
            })
            .await?;

        match upgrade_result {
            UpgradeResult::Upgraded(old_version, new_version) => {
                assert_eq!(old_version, current_version);
                assert_eq!(new_version, target_version);
            }
            _ => panic!(
                "Expected UpgradeResult::Upgraded but was {:#?}",
                upgrade_result
            ),
        }

        assert_eq!(service_manager.service.service_data.pid, None);
        assert_eq!(
            service_manager.service.service_data.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?)
        );
        assert_eq!(service_manager.service.service_data.version, target_version);
        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Stopped
        );

        Ok(())
    }

    #[tokio::test]
    async fn upgrade_should_return_upgraded_but_not_started_if_service_did_not_start() -> Result<()>
    {
        let current_version = "0.1.0";
        let target_version = "0.2.0";

        let tmp_data_dir = assert_fs::TempDir::new()?;
        let current_install_dir = tmp_data_dir.child("safenode_install");
        current_install_dir.create_dir_all()?;

        let current_node_bin = current_install_dir.child("safenode");
        current_node_bin.write_binary(b"fake safenode binary")?;
        let target_node_bin = tmp_data_dir.child("safenode");
        target_node_bin.write_binary(b"fake safenode binary")?;

        let current_node_bin_str = current_node_bin.to_path_buf().to_string_lossy().to_string();

        let mut mock_service_control = MockServiceControl::new();

        // before binary upgrade
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);
        mock_service_control
            .expect_stop()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));

        // after binary upgrade
        mock_service_control
            .expect_uninstall()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_install()
            .with(always())
            .times(1)
            .returning(|_| Ok(()));

        // after service restart
        mock_service_control
            .expect_start()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3000))
            .times(1)
            .returning(|_| ());
        mock_service_control
            .expect_get_process_pid()
            .with(eq(current_node_bin.to_path_buf().clone()))
            .times(1)
            .returning(move |_| {
                Err(ServiceControlError::ServiceProcessNotFound(
                    current_node_bin_str.clone(),
                ))
            });

        let mut service_data = NodeServiceData {
            connected_peers: None,
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            genesis: false,
            listen_addr: None,
            local: false,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            number: 1,
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            pid: Some(1000),
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            safenode_path: current_node_bin.to_path_buf(),
            service_name: "safenode1".to_string(),
            status: ServiceStatus::Running,
            user: "safe".to_string(),
            version: current_version.to_string(),
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let upgrade_result = service_manager
            .upgrade(UpgradeOptions {
                bootstrap_peers: Vec::new(),
                env_variables: None,
                force: false,
                start_service: true,
                target_bin_path: target_node_bin.to_path_buf(),
                target_version: Version::parse(target_version).unwrap(),
            })
            .await?;

        match upgrade_result {
            UpgradeResult::UpgradedButNotStarted(old_version, new_version, _) => {
                assert_eq!(old_version, current_version);
                assert_eq!(new_version, target_version);
            }
            _ => panic!(
                "Expected UpgradeResult::UpgradedButNotStarted but was {:#?}",
                upgrade_result
            ),
        }

        Ok(())
    }

    #[tokio::test]
    async fn remove_should_remove_an_added_node() -> Result<()> {
        let temp_dir = assert_fs::TempDir::new()?;
        let log_dir = temp_dir.child("safenode1-logs");
        log_dir.create_dir_all()?;
        let data_dir = temp_dir.child("safenode1-data");
        data_dir.create_dir_all()?;
        let safenode_bin = data_dir.child("safenode");
        safenode_bin.write_binary(b"fake safenode binary")?;

        let mut mock_service_control = MockServiceControl::new();
        mock_service_control
            .expect_uninstall()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));

        let mut service_data = NodeServiceData {
            genesis: false,
            local: false,
            version: "0.98.1".to_string(),
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            status: ServiceStatus::Stopped,
            pid: None,
            peer_id: None,
            listen_addr: None,
            log_dir_path: log_dir.to_path_buf(),
            data_dir_path: data_dir.to_path_buf(),
            safenode_path: safenode_bin.to_path_buf(),
            connected_peers: None,
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.remove(false).await?;

        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Removed
        );
        log_dir.assert(predicate::path::missing());
        data_dir.assert(predicate::path::missing());

        Ok(())
    }

    #[tokio::test]
    async fn remove_should_return_an_error_if_attempting_to_remove_a_running_node() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);

        let mut service_data = NodeServiceData {
            genesis: false,
            local: false,
            version: "0.98.1".to_string(),
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            status: ServiceStatus::Running,
            pid: Some(1000),
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            listen_addr: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            connected_peers: None,
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let result = service_manager.remove(false).await;
        match result {
            Ok(_) => panic!("This test should result in an error"),
            Err(e) => assert_eq!("A running service cannot be removed", e.to_string()),
        }

        Ok(())
    }

    #[tokio::test]
    async fn remove_should_return_an_error_for_a_node_that_was_marked_running_but_was_not_actually_running(
    ) -> Result<()> {
        let temp_dir = assert_fs::TempDir::new()?;
        let log_dir = temp_dir.child("safenode1-logs");
        log_dir.create_dir_all()?;
        let data_dir = temp_dir.child("safenode1-data");
        data_dir.create_dir_all()?;
        let safenode_bin = data_dir.child("safenode");
        safenode_bin.write_binary(b"fake safenode binary")?;

        let mut mock_service_control = MockServiceControl::new();
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| false);

        let mut service_data = NodeServiceData {
            genesis: false,
            local: false,
            version: "0.98.1".to_string(),
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            status: ServiceStatus::Running,
            pid: Some(1000),
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            listen_addr: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
            safenode_path: PathBuf::from("/var/safenode-manager/services/safenode1/safenode"),
            connected_peers: None,
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        let result = service_manager.remove(false).await;
        match result {
            Ok(_) => panic!("This test should result in an error"),
            Err(e) => assert_eq!(
                "This service was marked as running but it had actually stopped",
                e.to_string()
            ),
        }

        Ok(())
    }

    #[tokio::test]
    async fn remove_should_remove_an_added_node_and_keep_directories() -> Result<()> {
        let temp_dir = assert_fs::TempDir::new()?;
        let log_dir = temp_dir.child("safenode1-logs");
        log_dir.create_dir_all()?;
        let data_dir = temp_dir.child("safenode1-data");
        data_dir.create_dir_all()?;
        let safenode_bin = data_dir.child("safenode");
        safenode_bin.write_binary(b"fake safenode binary")?;

        let mut mock_service_control = MockServiceControl::new();
        mock_service_control
            .expect_uninstall()
            .with(eq("safenode1"))
            .times(1)
            .returning(|_| Ok(()));

        let mut service_data = NodeServiceData {
            genesis: false,
            local: false,
            version: "0.98.1".to_string(),
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            rpc_socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
            status: ServiceStatus::Stopped,
            pid: None,
            peer_id: None,
            listen_addr: None,
            log_dir_path: log_dir.to_path_buf(),
            data_dir_path: data_dir.to_path_buf(),
            safenode_path: safenode_bin.to_path_buf(),
            connected_peers: None,
        };
        let service = NodeService::new(&mut service_data, Box::new(MockRpcClient::new()));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(mock_service_control),
            VerbosityLevel::Normal,
        );

        service_manager.remove(true).await?;

        assert_matches!(
            service_manager.service.service_data.status,
            ServiceStatus::Removed
        );
        log_dir.assert(predicate::path::is_dir());
        data_dir.assert(predicate::path::is_dir());

        Ok(())
    }
}
