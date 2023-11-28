use crate::node::{InstalledNode, NodeRegistry, NodeStatus};
use crate::service::ServiceControl;
use color_eyre::{eyre::eyre, Result};
use colored::Colorize;
use sn_node_rpc_client::RpcActions;

pub async fn start(
    node: &mut InstalledNode,
    service_control: &dyn ServiceControl,
    rpc_client: &dyn RpcActions,
) -> Result<()> {
    if let NodeStatus::Running = node.status {
        // The last time we checked the service was running, but it doesn't mean it's actually
        // running at this point in time. If it is running, we don't need to do anything. If it
        // stopped because of a fault, we will drop to the code below and attempt to start it
        // again.
        if service_control.is_service_process_running(node.pid.unwrap()) {
            println!("The {} service is already running", node.service_name);
            return Ok(());
        }
    }

    // At this point the service either hasn't been started for the first time or it has been
    // stopped. If it was stopped, it was either intentional or because it crashed.
    println!("Attempting to start {}...", node.service_name);
    service_control.start(&node.service_name)?;

    // Give the node a little bit of time to start before initiating the node info query.
    service_control.wait(3);
    let node_info = rpc_client.node_info().await?;
    node.pid = Some(node_info.pid);
    node.peer_id = Some(node_info.peer_id);
    node.status = NodeStatus::Running;

    println!("✓ Started {} service", node.service_name);
    println!("  - Peer ID: {}", node_info.peer_id);
    println!("  - Logs: {}", node_info.log_path.to_string_lossy());

    Ok(())
}

pub async fn stop(node: &mut InstalledNode, service_control: &dyn ServiceControl) -> Result<()> {
    match node.status {
        NodeStatus::Installed => Err(eyre!(
            "Service {} has not been started since it was installed",
            node.service_name
        )),
        NodeStatus::Running => {
            let pid = node.pid.unwrap();
            if service_control.is_service_process_running(pid) {
                println!("Attempting to stop {}...", node.service_name);
                service_control.stop(&node.service_name)?;
                println!(
                    "✓ Service {} with PID {} was stopped",
                    node.service_name, pid
                );
            } else {
                println!("✓ Service {} was already stopped", node.service_name);
            }
            node.pid = None;
            node.status = NodeStatus::Stopped;
            Ok(())
        }
        NodeStatus::Stopped => {
            println!("✓ Service {} was already stopped", node.service_name);
            Ok(())
        }
    }
}

pub async fn status(
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    detailed_view: bool,
) -> Result<()> {
    // Again confirm that services which are marked running are still actually running.
    // If they aren't we'll mark them as stopped.
    for node in &mut node_registry.installed_nodes {
        if let NodeStatus::Running = node.status {
            if let Some(pid) = node.pid {
                if !service_control.is_service_process_running(pid) {
                    node.status = NodeStatus::Stopped;
                    node.pid = None;
                }
            }
        }
    }

    if detailed_view {
        for node in &node_registry.installed_nodes {
            println!("{} - {}", node.service_name, format_status(&node.status));
            println!("\tVersion: {}", node.version);
            println!(
                "\tPeer ID: {}",
                node.peer_id.map_or("-".to_string(), |p| p.to_string())
            );
            println!("\tPort: {}", node.port);
            println!("\tRPC Port: {}", node.rpc_port);
            println!(
                "\tPID: {}",
                node.pid.map_or("-".to_string(), |p| p.to_string())
            );
            println!("\tData path: {}", node.data_dir_path.to_string_lossy());
            println!("\tLog path: {}", node.log_dir_path.to_string_lossy());
        }
    } else {
        println!("{:<20} {:<52} Status", "Service Name", "Peer ID");
        for node in &node_registry.installed_nodes {
            let peer_id = node
                .peer_id
                .map_or_else(|| "-".to_string(), |p| p.to_string());
            println!(
                "{:<20} {:<52} {}",
                node.service_name,
                peer_id,
                format_status(&node.status)
            );
        }
    }

    Ok(())
}

fn format_status(status: &NodeStatus) -> String {
    match status {
        NodeStatus::Running => "RUNNING".green().to_string(),
        NodeStatus::Stopped => "STOPPED".red().to_string(),
        NodeStatus::Installed => "INSTALLED".yellow().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{InstalledNode, NodeStatus};
    use crate::service::MockServiceControl;
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use libp2p_identity::PeerId;
    use mockall::mock;
    use mockall::predicate::*;
    use mockall::Sequence;
    use sn_node_rpc_client::{
        NetworkInfo, NodeInfo, RecordAddress, Result as RpcResult, RpcActions,
    };
    use std::path::PathBuf;
    use std::str::FromStr;

    mock! {
        pub RpcClient {}
        #[async_trait]
        impl RpcActions for RpcClient {
            async fn node_info(&self) -> RpcResult<NodeInfo>;
            async fn network_info(&self) -> RpcResult<NetworkInfo>;
            async fn record_addresses(&self) -> RpcResult<Vec<RecordAddress>>;
            async fn gossipsub_subscribe(&self, topic: &str) -> RpcResult<()>;
            async fn gossipsub_unsubscribe(&self, topic: &str) -> RpcResult<()>;
            async fn gossipsub_publish(&self, topic: &str, message: &str) -> RpcResult<()>;
            async fn node_restart(&self, delay_millis: u64) -> RpcResult<()>;
            async fn node_stop(&self, delay_millis: u64) -> RpcResult<()>;
            async fn node_update(&self, delay_millis: u64) -> RpcResult<()>;
        }
    }

    #[tokio::test]
    async fn start_should_start_a_newly_installed_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_start()
            .with(eq("Safenode service 1"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3))
            .times(1)
            .returning(|_| ());
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 1000,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(1), // the service was just started
            })
        });

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "Safenode service 1".to_string(),
            user: "safe".to_string(),
            number: 1,
            port: 8080,
            rpc_port: 8081,
            status: NodeStatus::Installed,
            pid: None,
            peer_id: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };
        start(&mut node, &mock_service_control, &mock_rpc_client).await?;

        assert_eq!(node.pid, Some(1000));
        assert_eq!(
            node.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
            )?)
        );
        assert_matches!(node.status, NodeStatus::Running);

        Ok(())
    }

    #[tokio::test]
    async fn start_should_start_a_stopped_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_start()
            .with(eq("Safenode service 2"))
            .times(1)
            .returning(|_| Ok(()));
        mock_service_control
            .expect_wait()
            .with(eq(3))
            .times(1)
            .returning(|_| ());
        mock_rpc_client.expect_node_info().times(1).returning(|| {
            Ok(NodeInfo {
                pid: 1001,
                peer_id: PeerId::from_str("12D3KooWAAqZWsjhdZTX7tniJ7Dwye3nEbp1dx1wE96sbgL51obs")?,
                log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(1),
            })
        });

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "Safenode service 2".to_string(),
            user: "safe".to_string(),
            number: 2,
            port: 8082,
            rpc_port: 8083,
            status: NodeStatus::Stopped,
            pid: Some(1001),
            peer_id: Some(PeerId::from_str(
                "12D3KooWAAqZWsjhdZTX7tniJ7Dwye3nEbp1dx1wE96sbgL51obs",
            )?),
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };
        start(&mut node, &mock_service_control, &mock_rpc_client).await?;

        assert_matches!(node.status, NodeStatus::Running);
        assert_eq!(node.pid, Some(1001));
        assert_eq!(
            node.peer_id,
            Some(PeerId::from_str(
                "12D3KooWAAqZWsjhdZTX7tniJ7Dwye3nEbp1dx1wE96sbgL51obs"
            )?)
        );

        Ok(())
    }

    #[tokio::test]
    async fn start_should_not_attempt_to_start_a_running_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();
        let mut mock_rpc_client = MockRpcClient::new();

        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true);
        mock_service_control
            .expect_start()
            .with(eq("Safenode service 1"))
            .times(0)
            .returning(|_| Ok(()));
        mock_rpc_client.expect_node_info().times(0).returning(|| {
            Ok(NodeInfo {
                pid: 1001,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(24 * 60 * 60),
            })
        });

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "Safenode service 1".to_string(),
            user: "safe".to_string(),
            number: 1,
            port: 8080,
            rpc_port: 8081,
            status: NodeStatus::Running,
            pid: Some(1000),
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };
        start(&mut node, &mock_service_control, &mock_rpc_client).await?;

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
            .returning(|_| true);
        mock_service_control
            .expect_start()
            .with(eq("Safenode service 1"))
            .times(0)
            .returning(|_| Ok(()));
        mock_rpc_client.expect_node_info().times(0).returning(|| {
            Ok(NodeInfo {
                pid: 1002,
                peer_id: PeerId::from_str("12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR")?,
                log_path: PathBuf::from("~/.local/share/safe/service1/logs"),
                version: "0.98.1".to_string(),
                uptime: std::time::Duration::from_secs(1),
            })
        });

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "Safenode service 1".to_string(),
            user: "safe".to_string(),
            number: 1,
            port: 8080,
            rpc_port: 8081,
            status: NodeStatus::Running,
            pid: Some(1000),
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };
        start(&mut node, &mock_service_control, &mock_rpc_client).await?;

        Ok(())
    }

    #[tokio::test]
    async fn stop_should_stop_a_running_service() -> Result<()> {
        let mut mock_service_control = MockServiceControl::new();

        let mut seq = Sequence::new();
        mock_service_control
            .expect_is_service_process_running()
            .with(eq(1000))
            .times(1)
            .returning(|_| true)
            .in_sequence(&mut seq);
        mock_service_control
            .expect_stop()
            .with(eq("Safenode service 1"))
            .times(1)
            .returning(|_| Ok(()))
            .in_sequence(&mut seq);

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "Safenode service 1".to_string(),
            user: "safe".to_string(),
            number: 1,
            port: 8080,
            rpc_port: 8081,
            status: NodeStatus::Running,
            pid: Some(1000),
            peer_id: Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR",
            )?),
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };
        stop(&mut node, &mock_service_control).await?;

        assert_eq!(node.pid, None);
        // The peer ID should be retained on a service stop.
        assert_eq!(
            node.peer_id,
            Some(PeerId::from_str(
                "12D3KooWS2tpXGGTmg2AHFiDh57yPQnat49YHnyqoggzXZWpqkCR"
            )?)
        );
        assert_matches!(node.status, NodeStatus::Stopped);

        Ok(())
    }

    #[tokio::test]
    async fn stop_should_return_error_for_attempt_to_stop_installed_service() -> Result<()> {
        let mock_service_control = MockServiceControl::new();

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "safenode1".to_string(),
            user: "safe".to_string(),
            number: 1,
            port: 8080,
            rpc_port: 8081,
            status: NodeStatus::Installed,
            pid: None,
            peer_id: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };

        let result = stop(&mut node, &mock_service_control).await;

        match result {
            Ok(()) => panic!("This test should result in an error"),
            Err(e) => {
                assert_eq!(
                    "Service safenode1 has not been started since it was installed",
                    e.to_string()
                );
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn stop_should_return_ok_when_attempting_to_stop_service_that_was_already_stopped(
    ) -> Result<()> {
        let mock_service_control = MockServiceControl::new();

        let mut node = InstalledNode {
            version: "0.98.1".to_string(),
            service_name: "Safenode service 1".to_string(),
            user: "safe".to_string(),
            number: 1,
            port: 8080,
            rpc_port: 8081,
            status: NodeStatus::Stopped,
            pid: None,
            peer_id: None,
            log_dir_path: PathBuf::from("/var/log/safenode/safenode1"),
            data_dir_path: PathBuf::from("/var/safenode-manager/services/safenode1"),
        };

        stop(&mut node, &mock_service_control).await?;

        assert_eq!(node.pid, None);
        assert_matches!(node.status, NodeStatus::Stopped);

        Ok(())
    }
}
