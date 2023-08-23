// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod check_testnet;

use color_eyre::{eyre::eyre, Help, Result};
use libp2p::identity::PeerId;
#[cfg(test)]
use mockall::automock;
use std::future::Future;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Command, Stdio};
use tracing::{debug, info};

pub const DEFAULT_NODE_LAUNCH_INTERVAL: u64 = 1000;
#[cfg(not(target_os = "windows"))]
pub const SAFENODE_BIN_NAME: &str = "safenode";
#[cfg(target_os = "windows")]
pub const SAFENODE_BIN_NAME: &str = "safenode.exe";

#[cfg(not(target_os = "windows"))]
pub const FAUCET_BIN_NAME: &str = "faucet";
#[cfg(target_os = "windows")]
pub const FAUCET_BIN_NAME: &str = "faucet.exe";

/// This trait exists for unit testing.
///
/// It enables us to test that nodes are launched with the correct arguments without actually
/// launching processes.
#[cfg_attr(test, automock)]
pub trait NodeLauncher {
    fn launch(&self, node_bin_path: &Path, args: Vec<String>) -> Result<()>;
}

/// This trait exists for unit testing.
///
/// It allows us to return a dummy PeerId during a test without making a real RPC call.
#[cfg_attr(test, automock)]
pub trait RpcClient {
    fn obtain_peer_id(
        &self,
        rpc_address: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Result<PeerId>> + 'static>>;
}

#[derive(Default)]
pub struct SafeNodeLauncher {}
impl NodeLauncher for SafeNodeLauncher {
    fn launch(&self, node_bin_path: &Path, args: Vec<String>) -> Result<()> {
        debug!("Running {:#?} with args: {:#?}", node_bin_path, args);
        Command::new(node_bin_path)
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(())
    }
}

#[derive(Default)]
pub struct SafeRpcClient {}
impl RpcClient for SafeRpcClient {
    fn obtain_peer_id(
        &self,
        rpc_address: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Result<PeerId>> + 'static>> {
        Box::pin(async move {
            let peer_id = crate::check_testnet::obtain_peer_id(rpc_address).await?;
            info!("Obtained peer ID {}", peer_id.to_string());
            Ok(peer_id)
        })
    }
}

#[derive(Default)]
pub struct TestnetBuilder {
    node_bin_path: Option<PathBuf>,
    node_launch_interval: Option<u64>,
    nodes_dir_path: Option<PathBuf>,
    clear_nodes_dir: bool,
    flamegraph_mode: bool,
}

impl TestnetBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the path of the `safenode` binary.
    ///
    /// If not set, we will just use `safenode` and assume it is on `PATH`.
    pub fn node_bin_path(&mut self, node_bin_path: PathBuf) -> &mut Self {
        self.node_bin_path = Some(node_bin_path);
        self
    }

    /// Set the number of milliseconds to wait between launching each node.
    pub fn node_launch_interval(&mut self, node_launch_interval: u64) -> &mut Self {
        self.node_launch_interval = Some(node_launch_interval);
        self
    }

    /// Set the directory under which to output the data and logs for the nodes.
    ///
    /// A directory called 'local-test-network' will be created under here, and under this, there
    /// will be a directory for each node.
    pub fn nodes_dir_path(&mut self, nodes_dir_path: PathBuf) -> &mut Self {
        self.nodes_dir_path = Some(nodes_dir_path);
        self
    }

    /// Set this to use `flamegraph` to profile the network.
    ///
    /// Requires installations of `cargo flamegraph` and `perf`. This mode is not supported on
    /// Windows.
    pub fn flamegraph_mode(&mut self, flamegraph_mode: bool) -> &mut Self {
        self.flamegraph_mode = flamegraph_mode;
        self
    }

    /// Construct a `Testnet` instance using the options specified.
    ///
    /// The testnet instance and the path to the network contacts will be returned.
    pub fn build(&self) -> Result<Testnet> {
        let default_node_dir_path = dirs_next::data_dir()
            .ok_or_else(|| eyre!("Failed to obtain data directory path"))?
            .join("safe")
            .join("node");
        let nodes_dir_path = self
            .nodes_dir_path
            .as_ref()
            .unwrap_or(&default_node_dir_path);
        if self.clear_nodes_dir && nodes_dir_path.exists() {
            info!("Clearing {:#?} for new network", nodes_dir_path);
            std::fs::remove_dir_all(nodes_dir_path.clone())?;
        }

        let testnet = Testnet::new(
            self.node_bin_path
                .as_ref()
                .unwrap_or(&PathBuf::from(SAFENODE_BIN_NAME))
                .clone(),
            self.node_launch_interval
                .unwrap_or(DEFAULT_NODE_LAUNCH_INTERVAL),
            nodes_dir_path.clone(),
            self.flamegraph_mode,
            Box::default() as Box<SafeNodeLauncher>,
            Box::default() as Box<SafeRpcClient>,
        )?;
        Ok(testnet)
    }
}

pub struct Testnet {
    pub node_bin_path: PathBuf,
    pub node_launch_interval: u64,
    pub nodes_dir_path: PathBuf,
    pub flamegraph_mode: bool,
    pub node_count: usize,
    pub launcher: Box<dyn NodeLauncher>,
    pub rpc_client: Box<dyn RpcClient>,
}

impl Testnet {
    /// Create a new `Testnet` instance.
    ///
    /// The `node_data_dir` path will be inspected to see if it already exists, and if so, to
    /// obtain the number of nodes. This is used for having nodes join an existing network.
    pub fn new(
        node_bin_path: PathBuf,
        node_launch_interval: u64,
        nodes_dir_path: PathBuf,
        flamegraph_mode: bool,
        launcher: Box<dyn NodeLauncher>,
        rpc_client: Box<dyn RpcClient>,
    ) -> Result<Self> {
        let mut node_count = 0;
        if nodes_dir_path.exists() {
            let entries = std::fs::read_dir(&nodes_dir_path)?;
            for entry in entries {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    node_count += 1;
                }
            }
        }

        Ok(Self {
            node_bin_path,
            node_launch_interval,
            nodes_dir_path,
            flamegraph_mode,
            node_count,
            launcher,
            rpc_client,
        })
    }

    /// Use this function to create a `Testnet` with a fluent interface.
    pub fn configure() -> TestnetBuilder {
        TestnetBuilder::default()
    }

    /// Launches a genesis node at 127.0.0.1:11101.
    ///
    /// The RPC service is launched along with the node, on port 12001.
    ///
    /// Returns the MultiAdrr of the genesis node, including the peer ID.
    ///
    /// # Arguments
    ///
    /// * `node_args` - Additional arguments to pass to the node process, e.g., --json-log-output.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * The node data directory is already populated with previous node root directories
    /// * The node process fails
    pub async fn launch_genesis(&self, node_args: Vec<String>) -> Result<String> {
        if self.node_count != 0 {
            return Err(eyre!(
                "A new testnet cannot be launched until the data directory is cleared"
            )
            .suggestion("Try again using the `--clean` argument"));
        }

        let rpc_address = "127.0.0.1:12001".parse()?;
        let mut launch_args =
            self.get_launch_args("safenode-1".to_string(), Some(rpc_address), node_args)?;

        let genesis_port: u16 = 11101;
        launch_args.push("--port".to_string());
        launch_args.push(genesis_port.to_string());

        let launch_bin = self.get_launch_bin();
        self.launcher.launch(&launch_bin, launch_args)?;
        info!(
            "Delaying for {} seconds before launching other nodes",
            self.node_launch_interval
        );
        std::thread::sleep(std::time::Duration::from_millis(self.node_launch_interval));

        let peer_id = self.rpc_client.obtain_peer_id(rpc_address).await?;
        let genesis_multi_addr = format!("/ip4/127.0.0.1/tcp/{:?}/p2p/{}", genesis_port, peer_id);
        Ok(genesis_multi_addr)
    }

    /// Launches a number of new nodes, either for a new network or an existing network.
    ///
    /// # Arguments
    ///
    /// * `number_of_nodes` - The number of nodes to launch.
    /// * `node_args` - Additional arguments to pass to the node process, e.g., --json-logs.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * The node data directories cannot be created
    /// * The node process fails
    pub fn launch_nodes(&mut self, number_of_nodes: usize, node_args: Vec<String>) -> Result<()> {
        let start = if self.node_count == 0 {
            self.node_count + 2
        } else {
            self.node_count + 1
        };
        let end = self.node_count + number_of_nodes;
        for i in start..=end {
            info!("Launching node {i} of {end}...");
            let rpc_address = format!("127.0.0.1:{}", 12000 + i).parse()?;
            let launch_args = self.get_launch_args(
                format!("safenode-{i}"),
                Some(rpc_address),
                node_args.clone(),
            )?;
            let launch_bin = self.get_launch_bin();
            self.launcher.launch(&launch_bin, launch_args)?;

            if i < end {
                info!(
                    "Delaying for {} seconds before launching the next node",
                    self.node_launch_interval / 1000
                );
                std::thread::sleep(std::time::Duration::from_millis(self.node_launch_interval));
            }
        }
        self.node_count += number_of_nodes;
        Ok(())
    }

    fn get_launch_args(
        &self,
        node_name: String,
        rpc_address: Option<SocketAddr>,
        node_args: Vec<String>,
    ) -> Result<Vec<String>> {
        let mut launch_args = Vec::new();
        if self.flamegraph_mode {
            launch_args.push("flamegraph".to_string());
            launch_args.push("--output".to_string());
            launch_args.push(
                self.nodes_dir_path
                    .join(format!("{node_name}-flame.svg"))
                    .to_str()
                    .ok_or_else(|| eyre!("Unable to obtain path"))?
                    .to_string(),
            );
            launch_args.push("--root".to_string());
            launch_args.push("--bin".to_string());
            launch_args.push("safenode".to_string());
            launch_args.push("--".to_string());
        }
        launch_args.push("--log-output-dest".to_string());
        launch_args.push("data-dir".to_string());
        launch_args.push("--local".to_string());
        if let Some(addr) = rpc_address {
            launch_args.push("--rpc".to_string());
            launch_args.push(addr.to_string());
        }
        launch_args.extend(node_args);

        Ok(launch_args)
    }

    fn get_launch_bin(&self) -> PathBuf {
        if self.flamegraph_mode {
            PathBuf::from("cargo")
        } else {
            self.node_bin_path.clone()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use assert_fs::prelude::*;
    use color_eyre::Result;
    use libp2p::identity::Keypair;
    use mockall::predicate::*;

    const GENESIS_NODE_NAME: &str = "safenode-1";
    const NODE_LAUNCH_INTERVAL: u64 = 0;
    const TESTNET_DIR_NAME: &str = "local-test-network";

    fn setup_default_mocks() -> (MockNodeLauncher, MockRpcClient) {
        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));
        let rpc_client = setup_default_rpc_client_mock();
        (node_launcher, rpc_client)
    }

    fn setup_default_rpc_client_mock() -> MockRpcClient {
        let mut rpc_client = MockRpcClient::new();
        rpc_client.expect_obtain_peer_id().returning(move |_| {
            let peer_id = PeerId::from_public_key(&Keypair::generate_ed25519().public());
            Box::pin(async move { Ok(peer_id) })
        });
        rpc_client
    }

    #[test]
    fn new_should_create_a_testnet_with_zero_nodes_when_no_previous_network_exists() -> Result<()> {
        let (node_launcher, rpc_client) = setup_default_mocks();
        let testnet = Testnet::new(
            PathBuf::from(SAFENODE_BIN_NAME),
            30000,
            PathBuf::from(TESTNET_DIR_NAME),
            false,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;

        assert_eq!(testnet.node_bin_path, PathBuf::from(SAFENODE_BIN_NAME));
        assert_eq!(testnet.node_launch_interval, 30000);
        assert_eq!(testnet.nodes_dir_path, PathBuf::from(TESTNET_DIR_NAME));
        assert!(!testnet.flamegraph_mode);
        assert_eq!(testnet.node_count, 0);

        Ok(())
    }

    #[test]
    fn new_should_create_a_testnet_with_twenty_nodes_when_a_previous_network_exists() -> Result<()>
    {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_data_dir = nodes_dir.child("safenode-1");
        genesis_data_dir.create_dir_all()?;
        for i in 1..=20 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.create_dir_all()?;
        }

        let (node_launcher, rpc_client) = setup_default_mocks();
        let testnet = Testnet::new(
            PathBuf::from(SAFENODE_BIN_NAME),
            30000,
            nodes_dir.to_path_buf(),
            false,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;

        assert_eq!(testnet.node_bin_path, PathBuf::from(SAFENODE_BIN_NAME));
        assert_eq!(testnet.node_launch_interval, 30000);
        assert_eq!(testnet.nodes_dir_path, nodes_dir.to_path_buf());
        assert!(!testnet.flamegraph_mode);
        assert_eq!(testnet.node_count, 20);

        Ok(())
    }

    #[tokio::test]
    async fn launch_genesis_should_launch_the_genesis_node() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;

        let rpc_address: SocketAddr = "127.0.0.1:12001".parse()?;
        let mut node_launcher = MockNodeLauncher::new();
        node_launcher
            .expect_launch()
            .times(1)
            .with(
                eq(node_bin_path.path().to_path_buf()),
                eq(vec![
                    "--log-output-dest".to_string(),
                    "data-dir".to_string(),
                    "--local".to_string(),
                    "--rpc".to_string(),
                    rpc_address.to_string(),
                    "--log-format".to_string(),
                    "json".to_string(),
                    "--port".to_string(),
                    "11101".to_string(),
                ]),
            )
            .returning(|_, _| Ok(()));
        let peer_id = PeerId::from_public_key(&Keypair::generate_ed25519().public());
        let mut rpc_client = MockRpcClient::new();
        rpc_client
            .expect_obtain_peer_id()
            .times(1)
            .with(eq(rpc_address))
            .returning(move |_| Box::pin(async move { Ok(peer_id) }));
        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;

        let multiaddr = testnet
            .launch_genesis(vec!["--log-format".to_string(), "json".to_string()])
            .await?;

        assert_eq!(
            format!("/ip4/127.0.0.1/tcp/11101/p2p/{}", peer_id),
            multiaddr
        );
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn launch_genesis_with_flamegraph_mode_should_launch_the_genesis_node() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let graph_output_file = nodes_dir.child(format!("{GENESIS_NODE_NAME}-flame.svg"));

        let rpc_address: SocketAddr = "127.0.0.1:12001".parse()?;
        let mut node_launcher = MockNodeLauncher::new();
        node_launcher
            .expect_launch()
            .times(1)
            .with(
                eq(PathBuf::from("cargo")),
                eq(vec![
                    "flamegraph".to_string(),
                    "--output".to_string(),
                    graph_output_file
                        .path()
                        .to_str()
                        .ok_or_else(|| eyre!("Unable to obtain path"))?
                        .to_string(),
                    "--root".to_string(),
                    "--bin".to_string(),
                    SAFENODE_BIN_NAME.to_string(),
                    "--".to_string(),
                    "--log-output-dest".to_string(),
                    "data-dir".to_string(),
                    "--local".to_string(),
                    "--rpc".to_string(),
                    rpc_address.to_string(),
                    "--log-format".to_string(),
                    "json".to_string(),
                    "--port".to_string(),
                    "11101".to_string(),
                ]),
            )
            .returning(|_, _| Ok(()));
        let peer_id = PeerId::from_public_key(&Keypair::generate_ed25519().public());
        let mut rpc_client = MockRpcClient::new();
        rpc_client
            .expect_obtain_peer_id()
            .times(1)
            .with(eq(rpc_address))
            .returning(move |_| Box::pin(async move { Ok(peer_id) }));

        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            true,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;
        let multiaddr = testnet
            .launch_genesis(vec!["--log-format".to_string(), "json".to_string()])
            .await?;

        assert_eq!(
            format!("/ip4/127.0.0.1/tcp/11101/p2p/{}", peer_id),
            multiaddr
        );
        Ok(())
    }

    #[tokio::test]
    async fn launch_genesis_should_return_error_if_we_are_using_an_existing_network() -> Result<()>
    {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_data_dir = nodes_dir.child(GENESIS_NODE_NAME);
        genesis_data_dir.create_dir_all()?;
        for i in 1..=20 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.create_dir_all()?;
        }

        let (node_launcher, rpc_client) = setup_default_mocks();
        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;
        let result = testnet
            .launch_genesis(vec!["--log-format".to_string(), "json".to_string()])
            .await;

        match result {
            Ok(_) => Err(eyre!("This test should return an error")),
            Err(e) => {
                assert_eq!(
                    e.to_string(),
                    "A new testnet cannot be launched until the data directory is cleared"
                );
                Ok(())
            }
        }
    }

    #[test]
    fn launch_nodes_should_launch_the_specified_number_of_nodes() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_node_dir = tmp_data_dir.child("safenode-1");
        genesis_node_dir.create_dir_all()?;

        let mut node_launcher = MockNodeLauncher::new();
        for i in 2..=20 {
            let rpc_port = 12000 + i;
            node_launcher
                .expect_launch()
                .times(1)
                .with(
                    eq(node_bin_path.path().to_path_buf()),
                    eq(vec![
                        "--log-output-dest".to_string(),
                        "data-dir".to_string(),
                        "--local".to_string(),
                        "--rpc".to_string(),
                        format!("127.0.0.1:{}", rpc_port),
                        "--log-format".to_string(),
                        "json".to_string(),
                    ]),
                )
                .returning(|_, _| Ok(()));
        }
        let rpc_client = setup_default_rpc_client_mock();

        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;
        let result = testnet.launch_nodes(20, vec!["--log-format".to_string(), "json".to_string()]);

        assert!(result.is_ok());
        assert_eq!(testnet.node_count, 20);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn launch_nodes_with_flamegraph_should_launch_the_specified_number_of_nodes() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_node_dir = tmp_data_dir.child("safenode-1");
        genesis_node_dir.create_dir_all()?;

        let mut node_launcher = MockNodeLauncher::new();
        for i in 2..=20 {
            let rpc_port = 12000 + i;
            let graph_output_file_path = nodes_dir
                .join(format!("safenode-{i}-flame.svg"))
                .to_str()
                .ok_or_else(|| eyre!("Unable to obtain path"))?
                .to_string();
            node_launcher
                .expect_launch()
                .times(1)
                .with(
                    eq(PathBuf::from("cargo")),
                    eq(vec![
                        "flamegraph".to_string(),
                        "--output".to_string(),
                        graph_output_file_path,
                        "--root".to_string(),
                        "--bin".to_string(),
                        SAFENODE_BIN_NAME.to_string(),
                        "--".to_string(),
                        "--log-output-dest".to_string(),
                        "data-dir".to_string(),
                        "--local".to_string(),
                        "--rpc".to_string(),
                        format!("127.0.0.1:{}", rpc_port),
                        "--log-format".to_string(),
                        "json".to_string(),
                    ]),
                )
                .returning(|_, _| Ok(()));
        }
        let rpc_client = setup_default_rpc_client_mock();

        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            true,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;
        let result = testnet.launch_nodes(20, vec!["--log-format".to_string(), "json".to_string()]);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn launch_nodes_should_launch_the_specified_number_of_additional_nodes() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_node_dir = tmp_data_dir.child("safenode-1");
        genesis_node_dir.create_dir_all()?;

        let mut node_launcher = MockNodeLauncher::new();
        for i in 2..=30 {
            let rpc_port = 12000 + i;
            node_launcher
                .expect_launch()
                .times(1)
                .with(
                    eq(node_bin_path.path().to_path_buf()),
                    eq(vec![
                        "--log-output-dest".to_string(),
                        "data-dir".to_string(),
                        "--local".to_string(),
                        "--rpc".to_string(),
                        format!("127.0.0.1:{}", rpc_port),
                        "--log-format".to_string(),
                        "json".to_string(),
                    ]),
                )
                .returning(|_, _| Ok(()));
        }
        let rpc_client = setup_default_rpc_client_mock();

        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
            Box::new(rpc_client),
        )?;
        let result = testnet.launch_nodes(20, vec!["--log-format".to_string(), "json".to_string()]);
        assert!(result.is_ok());
        assert_eq!(testnet.node_count, 20);

        let result = testnet.launch_nodes(10, vec!["--log-format".to_string(), "json".to_string()]);
        assert!(result.is_ok());
        assert_eq!(testnet.node_count, 30);
        Ok(())
    }
}
