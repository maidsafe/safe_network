// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::VerbosityLevel;
use color_eyre::{
    eyre::{eyre, OptionExt},
    Result,
};
use colored::Colorize;
use sn_service_management::{control::ServiceControl, ServiceStateActions, ServiceStatus};

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
                Err(eyre!("Service {} has been removed", self.service.name()))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use libp2p_identity::PeerId;
    use mockall::{mock, predicate::*};
    use service_manager::ServiceInstallCtx;
    use sn_service_management::error::Result as ServiceControlResult;
    use sn_service_management::node::{NodeService, NodeServiceData};
    use sn_service_management::rpc::{NetworkInfo, NodeInfo, RecordAddress, RpcActions};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::PathBuf,
        str::FromStr,
    };

    mock! {
        pub RpcClient {}
        #[async_trait]
        impl RpcActions for RpcClient {
            async fn node_info(&self) -> ServiceControlResult<NodeInfo>;
            async fn network_info(&self) -> ServiceControlResult<NetworkInfo>;
            async fn record_addresses(&self) -> ServiceControlResult<Vec<RecordAddress>>;
            async fn gossipsub_subscribe(&self, topic: &str) -> ServiceControlResult<()>;
            async fn gossipsub_unsubscribe(&self, topic: &str) -> ServiceControlResult<()>;
            async fn gossipsub_publish(&self, topic: &str, message: &str) -> ServiceControlResult<()>;
            async fn node_restart(&self, delay_millis: u64, retain_peer_id: bool) -> ServiceControlResult<()>;
            async fn node_stop(&self, delay_millis: u64) -> ServiceControlResult<()>;
            async fn node_update(&self, delay_millis: u64) -> ServiceControlResult<()>;
        }
    }

    mock! {
        pub ServiceControl {}
        impl ServiceControl for ServiceControl {
            fn create_service_user(&self, username: &str) -> ServiceControlResult<()>;
            fn get_available_port(&self) -> ServiceControlResult<u16>;
            fn install(&self, install_ctx: ServiceInstallCtx) -> ServiceControlResult<()>;
            fn get_process_pid(&self, name: &str) -> ServiceControlResult<u32>;
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

        let service_data = NodeServiceData {
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
        let service = NodeService::new(service_data, Box::new(mock_rpc_client));
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

        let service_data = NodeServiceData {
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
        let service = NodeService::new(service_data, Box::new(mock_rpc_client));
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

        let service_data = NodeServiceData {
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
        let service = NodeService::new(service_data, Box::new(mock_rpc_client));
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
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| false);
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

        let service_data = NodeServiceData {
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
        let service = NodeService::new(service_data, Box::new(mock_rpc_client));
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
}
