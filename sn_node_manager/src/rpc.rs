// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    add_services::config::InstallNodeServiceCtxBuilder, config::create_owned_dir, ServiceManager,
    VerbosityLevel,
};
use color_eyre::{
    eyre::{eyre, OptionExt},
    Result,
};
use libp2p::PeerId;
use sn_service_management::{
    control::{ServiceControl, ServiceController},
    rpc::RpcClient,
    NodeRegistry, NodeService, NodeServiceData, ServiceStatus,
};

pub async fn restart_node_service(
    node_registry: &mut NodeRegistry,
    peer_id: PeerId,
    retain_peer_id: bool,
) -> Result<()> {
    let nodes_len = node_registry.nodes.len();
    let current_node_mut = node_registry
        .nodes
        .iter_mut()
        .find(|node| node.peer_id.is_some_and(|id| id == peer_id))
        .ok_or_eyre(format!("Could not find the provided PeerId: {peer_id:?}"))?;
    let current_node_clone = current_node_mut.clone();

    let rpc_client = RpcClient::from_socket_addr(current_node_mut.rpc_socket_addr);
    let service = NodeService::new(current_node_mut, Box::new(rpc_client));
    let mut service_manager = ServiceManager::new(
        service,
        Box::new(ServiceController {}),
        VerbosityLevel::Normal,
    );
    service_manager.stop().await?;

    let service_control = ServiceController {};
    if retain_peer_id {
        // reuse the same port and root dir to retain peer id.
        service_control
            .uninstall(&current_node_clone.service_name)
            .map_err(|err| {
                eyre!(
                    "Error while uninstalling node {:?} with: {err:?}",
                    current_node_clone.service_name
                )
            })?;
        let install_ctx = InstallNodeServiceCtxBuilder {
            bootstrap_peers: node_registry.bootstrap_peers.clone(),
            data_dir_path: current_node_clone.data_dir_path.clone(),
            env_variables: node_registry.environment_variables.clone(),
            genesis: current_node_clone.genesis,
            home_network: current_node_clone.home_network,
            local: current_node_clone.local,
            log_dir_path: current_node_clone.log_dir_path.clone(),
            metrics_port: None,
            name: current_node_clone.service_name.clone(),
            node_port: current_node_clone.get_safenode_port(),
            rpc_socket_addr: current_node_clone.rpc_socket_addr,
            safenode_path: current_node_clone.safenode_path.clone(),
            service_user: current_node_clone.user.clone(),
        }
        .build()?;
        service_control.install(install_ctx).map_err(|err| {
            eyre!(
                "Error while installing node {:?} with: {err:?}",
                current_node_clone.service_name
            )
        })?;
        service_manager.start().await?;
    } else {
        // else start a new node instance.
        let new_node_number = nodes_len + 1;
        let new_service_name = format!("safenode{new_node_number}");

        // modify the paths & copy safenode binary
        // example path "log_dir_path":"/var/log/safenode/safenode18"
        let log_dir_path = {
            let mut log_dir_path = current_node_clone.log_dir_path.clone();
            log_dir_path.pop();
            log_dir_path.join(&new_service_name)
        };
        // example path "data_dir_path":"/var/safenode-manager/services/safenode18"
        let data_dir_path = {
            let mut data_dir_path = current_node_clone.data_dir_path.clone();
            data_dir_path.pop();
            data_dir_path.join(&new_service_name)
        };
        create_owned_dir(log_dir_path.clone(), &current_node_clone.user).map_err(|err| {
            eyre!(
                "Error while creating owned dir for {:?}: {err:?}",
                current_node_clone.user
            )
        })?;
        create_owned_dir(data_dir_path.clone(), &current_node_clone.user).map_err(|err| {
            eyre!(
                "Error while creating owned dir for {:?}: {err:?}",
                current_node_clone.user
            )
        })?;
        // example path "safenode_path":"/var/safenode-manager/services/safenode18/safenode"
        let safenode_path = {
            let mut safenode_path = current_node_clone.safenode_path.clone();
            let safenode_file_name = safenode_path
                .file_name()
                .ok_or_eyre("Could not get filename from the current node's safenode path")?
                .to_string_lossy()
                .to_string();
            safenode_path.pop();
            safenode_path.pop();

            let safenode_path = safenode_path.join(&new_service_name);
            create_owned_dir(data_dir_path.clone(), &current_node_clone.user).map_err(|err| {
                eyre!(
                    "Error while creating owned dir for {:?}: {err:?}",
                    current_node_clone.user
                )
            })?;
            let safenode_path = safenode_path.join(safenode_file_name);

            std::fs::copy(&current_node_clone.safenode_path, &safenode_path).map_err(|err| {
                eyre!(
                    "Failed to copy safenode bin from {:?} to {safenode_path:?} with err: {err}",
                    current_node_clone.safenode_path
                )
            })?;
            safenode_path
        };

        let install_ctx = InstallNodeServiceCtxBuilder {
            bootstrap_peers: node_registry.bootstrap_peers.clone(),
            data_dir_path: data_dir_path.clone(),
            env_variables: node_registry.environment_variables.clone(),
            genesis: current_node_clone.genesis,
            home_network: current_node_clone.home_network,
            local: current_node_clone.local,
            log_dir_path: log_dir_path.clone(),
            name: new_service_name.clone(),
            metrics_port: None,
            node_port: None,
            rpc_socket_addr: current_node_clone.rpc_socket_addr,
            safenode_path: safenode_path.clone(),
            service_user: current_node_clone.user.clone(),
        }
        .build()?;
        service_control.install(install_ctx).map_err(|err| {
            eyre!("Error while installing node {new_service_name:?} with: {err:?}",)
        })?;

        let mut node = NodeServiceData {
            connected_peers: None,
            data_dir_path,
            genesis: current_node_clone.genesis,
            home_network: current_node_clone.home_network,
            listen_addr: None,
            local: current_node_clone.local,
            log_dir_path,
            number: new_node_number as u16,
            peer_id: None,
            pid: None,
            rpc_socket_addr: current_node_clone.rpc_socket_addr,
            safenode_path,
            service_name: new_service_name.clone(),
            status: ServiceStatus::Added,
            user: current_node_clone.user.clone(),
            version: current_node_clone.version.clone(),
        };

        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        let service = NodeService::new(&mut node, Box::new(rpc_client));
        let mut service_manager = ServiceManager::new(
            service,
            Box::new(ServiceController {}),
            VerbosityLevel::Normal,
        );
        service_manager.start().await?;
        node_registry
            .nodes
            .push(service_manager.service.service_data.clone());
    };

    Ok(())
}
