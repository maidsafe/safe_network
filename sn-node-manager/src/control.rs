use crate::node::{InstalledNode, NodeStatus};
use crate::service::ServiceControl;
use color_eyre::Result;
use sn_rpc_client::RpcClientInterface;

pub async fn start(
    node: &mut InstalledNode,
    service_control: &dyn ServiceControl,
    rpc_client: &dyn RpcClientInterface,
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
    use sn_rpc_client::{NetworkInfo, NodeInfo, Result as RpcResult, RpcClientInterface};
    use std::path::PathBuf;
    use std::str::FromStr;

    mock! {
        pub RpcClient {}
        #[async_trait]
        impl RpcClientInterface for RpcClient {
            async fn node_info(&self) -> RpcResult<NodeInfo>;
            async fn network_info(&self) -> RpcResult<NetworkInfo>;
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
        };
        start(&mut node, &mock_service_control, &mock_rpc_client).await?;

        Ok(())
    }
}
