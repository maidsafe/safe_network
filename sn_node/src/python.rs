use crate::{NodeBuilder, RunningNode};
use pyo3::{prelude::*, exceptions::PyRuntimeError, exceptions::PyValueError, types::PyModule};
use std::sync::Arc;
use tokio::sync::Mutex;
use libp2p::{identity::Keypair, Multiaddr};
use sn_evm::{EvmNetwork, RewardsAddress};
use std::{net::{IpAddr, SocketAddr}, path::PathBuf};
use const_hex::FromHex;

/// Python wrapper for the Safe Network Node
#[pyclass(name = "SafeNode")]
pub struct SafeNode {
    node: Arc<Mutex<Option<RunningNode>>>,
    runtime: Arc<Mutex<Option<tokio::runtime::Runtime>>>,
}

#[pymethods]
impl SafeNode {
    #[new]
    fn new() -> Self {
        Self {
            node: Arc::new(Mutex::new(None)),
            runtime: Arc::new(Mutex::new(None)),
        }
    }

    /// Start the node with the given configuration
    #[pyo3(signature = (
        rewards_address,
        evm_network,
        ip = "0.0.0.0",
        port = 0,
        initial_peers = vec![],
        local = false,
        root_dir = None,
        home_network = false,
    ))]
    fn run(
        &self,
        rewards_address: String,
        evm_network: String,
        ip: &str,
        port: u16,
        initial_peers: Vec<String>,
        local: bool,
        root_dir: Option<String>,
        home_network: bool,
    ) -> PyResult<()> {
        let rewards_address = RewardsAddress::from_hex(&rewards_address)
            .map_err(|e| PyValueError::new_err(format!("Invalid rewards address: {e}")))?;

        let evm_network = match evm_network.as_str() {
            "arbitrum_one" => EvmNetwork::ArbitrumOne,
            "arbitrum_sepolia" => EvmNetwork::ArbitrumSepolia,
            _ => return Err(PyValueError::new_err("Invalid EVM network. Must be 'arbitrum_one' or 'arbitrum_sepolia'")),
        };

        let ip: IpAddr = ip.parse()
            .map_err(|e| PyValueError::new_err(format!("Invalid IP address: {e}")))?;
        
        let node_socket_addr = SocketAddr::new(ip, port);

        let initial_peers: Vec<Multiaddr> = initial_peers
            .into_iter()
            .map(|addr| addr.parse())
            .collect::<Result<_, _>>()
            .map_err(|e| PyValueError::new_err(format!("Invalid peer address: {e}")))?;

        let root_dir = root_dir.map(PathBuf::from);

        let keypair = Keypair::generate_ed25519();

        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {e}")))?;

        let node = rt.block_on(async {
            let mut node_builder = NodeBuilder::new(
                keypair,
                rewards_address,
                evm_network,
                node_socket_addr,
                initial_peers,
                local,
                root_dir.unwrap_or_else(|| PathBuf::from(".")),
                #[cfg(feature = "upnp")]
                false,
            );
            node_builder.is_behind_home_network = home_network;
            
            node_builder.build_and_run()
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to start node: {e}")))
        })?;

        let mut node_guard = self.node.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire node lock"))?;
        *node_guard = Some(node);

        let mut rt_guard = self.runtime.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire runtime lock"))?;
        *rt_guard = Some(rt);

        Ok(())
    }

    /// Get the node's PeerId as a string
    fn peer_id(self_: PyRef<Self>) -> PyResult<String> {
        let node_guard = self_.node.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire node lock"))?;
        
        match &*node_guard {
            Some(node) => Ok(node.peer_id().to_string()),
            None => Err(PyRuntimeError::new_err("Node not started")),
        }
    }

    /// Get all record addresses stored by the node
    fn get_all_record_addresses(self_: PyRef<Self>) -> PyResult<Vec<String>> {
        let node_guard = self_.node.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire node lock"))?;
        let rt_guard = self_.runtime.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire runtime lock"))?;

        match (&*node_guard, &*rt_guard) {
            (Some(node), Some(rt)) => {
                let addresses = rt.block_on(async {
                    node.get_all_record_addresses()
                        .await
                        .map_err(|e| PyRuntimeError::new_err(format!("Failed to get addresses: {e}")))
                })?;

                Ok(addresses.into_iter().map(|addr| addr.to_string()).collect())
            }
            _ => Err(PyRuntimeError::new_err("Node not started")),
        }
    }

    /// Get the node's kbuckets information
    fn get_kbuckets(self_: PyRef<Self>) -> PyResult<Vec<(u32, Vec<String>)>> {
        let node_guard = self_.node.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire node lock"))?;
        let rt_guard = self_.runtime.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire runtime lock"))?;

        match (&*node_guard, &*rt_guard) {
            (Some(node), Some(rt)) => {
                let kbuckets = rt.block_on(async {
                    node.get_kbuckets()
                        .await
                        .map_err(|e| PyRuntimeError::new_err(format!("Failed to get kbuckets: {e}")))
                })?;

                Ok(kbuckets
                    .into_iter()
                    .map(|(distance, peers)| {
                        (distance, peers.into_iter().map(|p| p.to_string()).collect())
                    })
                    .collect())
            }
            _ => Err(PyRuntimeError::new_err("Node not started")),
        }
    }

    /// Get the node's rewards/wallet address as a hex string
    fn get_rewards_address(self_: PyRef<Self>) -> PyResult<String> {
        let node_guard = self_.node.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire node lock"))?;
        
        match &*node_guard {
            Some(node) => Ok(format!("0x{}", hex::encode(node.reward_address()))),
            None => Err(PyRuntimeError::new_err("Node not started")),
        }
    }

    /// Set a new rewards/wallet address for the node
    /// The address should be a hex string starting with "0x"
    fn set_rewards_address(self_: PyRef<Self>, address: String) -> PyResult<()> {
        let node_guard = self_.node.try_lock()
            .map_err(|_| PyRuntimeError::new_err("Failed to acquire node lock"))?;

        // Remove "0x" prefix if present
        let address = address.strip_prefix("0x").unwrap_or(&address);
        
        // Validate the address format
        let _new_address = RewardsAddress::from_hex(address)
            .map_err(|e| PyValueError::new_err(format!("Invalid rewards address: {e}")))?;

        match &*node_guard {
            Some(_) => Err(PyRuntimeError::new_err(
                "Changing rewards address requires node restart. Please stop and start the node with the new address."
            )),
            None => Err(PyRuntimeError::new_err("Node not started")),
        }
    }
}

/// Python module initialization
#[pymodule]
#[pyo3(name = "_safenode")]
fn init_module(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<SafeNode>()?;
    Ok(())
} 