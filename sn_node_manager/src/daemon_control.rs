// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{config::create_owned_dir, node_control, service::ServiceControl, VerbosityLevel};
use color_eyre::{
    eyre::{eyre, OptionExt},
    Result,
};
use libp2p::PeerId;
use service_manager::{ServiceInstallCtx, ServiceLabel};
use sn_node_rpc_client::RpcActions;
use sn_protocol::node_registry::{Node, NodeRegistry, NodeStatus};
use std::{ffi::OsString, net::Ipv4Addr, path::PathBuf};

pub fn run_daemon(
    address: Ipv4Addr,
    port: u16,
    daemon_path: PathBuf,
    service_control: &dyn ServiceControl,
    _verbosity: VerbosityLevel,
) -> Result<()> {
    let service_name_str = "safenode-manager-daemon";
    let service_name: ServiceLabel = service_name_str.parse()?;

    // try to stop and uninstall if already installed
    if let Err(err) = service_control.stop(service_name_str) {
        println!("Error while stopping manager daemon. Ignoring the error. {err:?}");
    }
    if let Err(err) = service_control.uninstall(service_name_str) {
        println!("Error while uninstalling manager daemon. Ignoring the error. {err:?}");
    }

    let install_ctx = ServiceInstallCtx {
        label: service_name.clone(),
        program: daemon_path,
        args: vec![
            OsString::from("--port"),
            OsString::from(port.to_string()),
            OsString::from("--address"),
            OsString::from(address.to_string()),
        ],
        contents: None,
        username: None,
        working_directory: None,
        environment: None,
    };
    service_control.install(install_ctx)?;
    service_control.start(&service_name.to_string())?;

    Ok(())
}

pub async fn restart_node_service(
    node_registry: &mut NodeRegistry,
    peer_id: PeerId,
    retain_peer_id: bool,
    rpc_client: &dyn RpcActions,
    service_control: &dyn ServiceControl,
) -> Result<()> {
    let nodes_len = node_registry.nodes.len();
    let current_node = node_registry
        .nodes
        .iter_mut()
        .find(|node| node.peer_id.is_some_and(|id| id == peer_id))
        .ok_or_eyre(format!("Could not find the provided PeerId: {peer_id:?}"))?;
    node_control::stop(current_node, service_control)
        .await
        .map_err(|err| {
            eyre!(
                "Error while stopping node {:?} with: {err:?}",
                current_node.service_name
            )
        })?;

    if retain_peer_id {
        // reuse the same port and root dir to retain peer id.
        service_control
            .uninstall(&current_node.service_name.clone())
            .map_err(|err| {
                eyre!(
                    "Error while uninstalling node {:?} with: {err:?}",
                    current_node.service_name
                )
            })?;
        let install_ctx = node_control::InstallNodeServiceCtxBuilder {
            local: current_node.local,
            data_dir_path: current_node.data_dir_path.clone(),
            genesis: current_node.genesis,
            name: current_node.service_name.clone(),
            node_port: current_node.get_safenode_port(),
            bootstrap_peers: node_registry.bootstrap_peers.clone(),
            rpc_socket_addr: current_node.rpc_socket_addr,
            log_dir_path: current_node.log_dir_path.clone(),
            safenode_path: current_node.safenode_path.clone(),
            service_user: current_node.user.clone(),
            env_variables: node_registry.environment_variables.clone(),
        }
        .build()?;
        service_control.install(install_ctx).map_err(|err| {
            eyre!(
                "Error while installing node {:?} with: {err:?}",
                current_node.service_name
            )
        })?;
        node_control::start(
            current_node,
            service_control,
            rpc_client,
            VerbosityLevel::Normal,
        )
        .await
        .map_err(|err| {
            eyre!(
                "Error while starting node {:?} with: {err:?}",
                current_node.service_name
            )
        })?;
    } else {
        // else start a new node instance.
        let new_node_number = nodes_len + 1;
        let new_service_name = format!("safenode{new_node_number}");

        // modify the paths & copy safenode binary
        // example path "log_dir_path":"/var/log/safenode/safenode18"
        let log_dir_path = {
            let mut log_dir_path = current_node.log_dir_path.clone();
            log_dir_path.pop();
            log_dir_path.join(&new_service_name)
        };
        // example path "data_dir_path":"/var/safenode-manager/services/safenode18"
        let data_dir_path = {
            let mut data_dir_path = current_node.data_dir_path.clone();
            data_dir_path.pop();
            data_dir_path.join(&new_service_name)
        };
        create_owned_dir(log_dir_path.clone(), &current_node.user).map_err(|err| {
            eyre!(
                "Error while creating owned dir for {:?}: {err:?}",
                current_node.user
            )
        })?;
        create_owned_dir(data_dir_path.clone(), &current_node.user).map_err(|err| {
            eyre!(
                "Error while creating owned dir for {:?}: {err:?}",
                current_node.user
            )
        })?;
        // example path "safenode_path":"/var/safenode-manager/services/safenode18/safenode"
        let safenode_path = {
            let mut safenode_path = current_node.safenode_path.clone();
            let safenode_file_name = safenode_path
                .file_name()
                .ok_or_eyre("Could not get filename from the current node's safenode path")?
                .to_string_lossy()
                .to_string();
            safenode_path.pop();
            safenode_path.pop();

            let safenode_path = safenode_path.join(&new_service_name);
            create_owned_dir(data_dir_path.clone(), &current_node.user).map_err(|err| {
                eyre!(
                    "Error while creating owned dir for {:?}: {err:?}",
                    current_node.user
                )
            })?;
            let safenode_path = safenode_path.join(safenode_file_name);

            std::fs::copy(&current_node.safenode_path, &safenode_path).map_err(|err| {
                eyre!(
                    "Failed to copy safenode bin from {:?} to {safenode_path:?} with err: {err}",
                    current_node.safenode_path
                )
            })?;
            safenode_path
        };

        let install_ctx = node_control::InstallNodeServiceCtxBuilder {
            local: current_node.local,
            genesis: current_node.genesis,
            name: new_service_name.clone(),
            // don't re-use port
            node_port: None,
            bootstrap_peers: node_registry.bootstrap_peers.clone(),
            rpc_socket_addr: current_node.rpc_socket_addr,
            // set new paths
            data_dir_path: data_dir_path.clone(),
            log_dir_path: log_dir_path.clone(),
            safenode_path: safenode_path.clone(),
            service_user: current_node.user.clone(),
            env_variables: node_registry.environment_variables.clone(),
        }
        .build()?;
        service_control.install(install_ctx).map_err(|err| {
            eyre!("Error while installing node {new_service_name:?} with: {err:?}",)
        })?;

        let mut node = Node {
            genesis: current_node.genesis,
            local: current_node.local,
            service_name: new_service_name.clone(),
            user: current_node.user.clone(),
            number: new_node_number as u16,
            rpc_socket_addr: current_node.rpc_socket_addr,
            version: current_node.version.clone(),
            status: NodeStatus::Added,
            listen_addr: None,
            pid: None,
            peer_id: None,
            log_dir_path,
            data_dir_path,
            safenode_path,
            connected_peers: None,
        };
        node_control::start(
            &mut node,
            service_control,
            rpc_client,
            VerbosityLevel::Normal,
        )
        .await
        .map_err(|err| eyre!("Error while starting node {new_service_name:?} with: {err:?}",))?;
        node_registry.nodes.push(node);
    };

    node_registry
        .save()
        .map_err(|err| eyre!("Error while saving node registry with: {err:?}"))?;

    Ok(())
}
