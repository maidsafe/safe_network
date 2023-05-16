// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
use color_eyre::{eyre::eyre, Result};
#[cfg(test)]
use mockall::automock;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing::{debug, info};

pub const DEFAULT_NODE_LAUNCH_INTERVAL: u64 = 1000;
#[cfg(not(target_os = "windows"))]
pub const SAFENODE_BIN_NAME: &str = "safenode";
#[cfg(target_os = "windows")]
pub const SAFENODE_BIN_NAME: &str = "safenode.exe";
const GENESIS_NODE_DIR_NAME: &str = "safenode-1";
const TESTNET_DIR_NAME: &str = "local-test-network";

/// This trait exists for unit testing.
///
/// It enables us to test that nodes are launched with the correct arguments without actually
/// launching processes.
#[cfg_attr(test, automock)]
pub trait NodeLauncher {
    fn launch(&self, node_bin_path: &Path, args: Vec<String>) -> Result<()>;
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

    /// Set this to clear out the existing node data directory for a new network.
    pub fn clear_nodes_dir(&mut self) -> &mut Self {
        self.clear_nodes_dir = true;
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
    pub fn build(&self) -> Result<(Testnet, PathBuf)> {
        let default_node_dir_path = dirs_next::home_dir()
            .ok_or_else(|| eyre!("Failed to obtain user's home path"))?
            .join(".safe")
            .join("node")
            .join(TESTNET_DIR_NAME);
        let nodes_dir_path = self
            .nodes_dir_path
            .as_ref()
            .unwrap_or(&default_node_dir_path);
        if self.clear_nodes_dir && nodes_dir_path.exists() {
            info!("Clearing {:#?} for new network", nodes_dir_path);
            std::fs::remove_dir_all(nodes_dir_path.clone())?;
        }

        let node_launcher = SafeNodeLauncher::default();
        let testnet = Testnet::new(
            self.node_bin_path
                .as_ref()
                .unwrap_or(&PathBuf::from(SAFENODE_BIN_NAME))
                .clone(),
            self.node_launch_interval
                .unwrap_or(DEFAULT_NODE_LAUNCH_INTERVAL),
            nodes_dir_path.clone(),
            self.flamegraph_mode,
            Box::new(node_launcher),
        )?;
        let network_contacts_path = nodes_dir_path
            .join(GENESIS_NODE_DIR_NAME)
            .join("section_tree");
        Ok((testnet, network_contacts_path))
    }
}

pub struct Testnet {
    pub node_bin_path: PathBuf,
    pub node_launch_interval: u64,
    pub nodes_dir_path: PathBuf,
    pub flamegraph_mode: bool,
    pub node_count: usize,
    pub launcher: Box<dyn NodeLauncher>,
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
    ) -> Result<Self> {
        let mut node_count = 0;
        if nodes_dir_path.exists() {
            let entries = std::fs::read_dir(&nodes_dir_path)?;
            for entry in entries {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let dir_name = entry.file_name().clone();
                    let dir_name = dir_name
                        .to_str()
                        .ok_or_else(|| eyre!("Failed to obtain dir name"))?;
                    // This excludes any directories the user may have created under the network
                    // data directory path, either intentionally or unintentionally.
                    if dir_name.starts_with("safenode-") && dir_name != GENESIS_NODE_DIR_NAME {
                        node_count += 1;
                    }
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
        })
    }

    /// Use this function to create a `Testnet` with a fluent interface.
    pub fn configure() -> TestnetBuilder {
        TestnetBuilder::default()
    }

    /// Launches a genesis node at the specified address.
    ///
    /// Returns the multiaddress of the genesis node.
    ///
    /// # Arguments
    ///
    /// * `address` - Optional address for where the genesis node will listen for connections. If
    /// not specified, the 127.0.0.1:12000 local address will be used.
    /// * `node_args` - Additional arguments to pass to the node process, e.g., --json-logs.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * The node data directory cannot be created
    /// * The node process fails
    /// * The network has already been launched previously
    pub fn launch_genesis(
        &self,
        _address: Option<SocketAddr>,
        node_args: Vec<String>,
    ) -> Result<String> {
        if self.node_count != 0 {
            return Err(eyre!(
                "A genesis node cannot be launched for an existing network"
            ));
        }

        // info!("Launching genesis node using address {address}...");

        let rpc_address = "127.0.0.1:12001".parse()?;
        let mut launch_args =
            self.get_launch_args("safenode-1".to_string(), Some(rpc_address), None, node_args)?;

        let genesis_port: u16 = 11101;
        // Let's start gen on a different port
        launch_args.push("--port".to_string());
        launch_args.push(genesis_port.to_string());

        let node_data_dir_path = self.nodes_dir_path.join("safenode-1");
        std::fs::create_dir_all(node_data_dir_path)?;

        let launch_bin = self.get_launch_bin();

        self.launcher.launch(&launch_bin, launch_args)?;
        info!(
            "Delaying for {} seconds before launching other nodes",
            self.node_launch_interval / 1000
        );
        std::thread::sleep(std::time::Duration::from_millis(self.node_launch_interval));

        // Now lets grab the genesis peer id.
        let gen_id_query_args = vec![
            "run",
            "--release",
            "--example",
            "safenode_rpc_client",
            "--",
            "127.0.0.1:12001",
            "info",
        ];

        let result = Command::new("cargo").args(gen_id_query_args).output()?;

        use regex::Regex;
        let re = Regex::new(r"Peer Id: ([^\n]+)").unwrap();
        let stdout = String::from_utf8(result.stdout).unwrap();
        if let Some(captures) = re.captures(&stdout) {
            let peer_id = captures.get(1).unwrap().as_str();
            info!("Peer Id: {}", peer_id);

            let genesis_multi_addr =
                format!("/ip4/127.0.0.1/tcp/{:?}/p2p/{}", genesis_port, peer_id);

            Ok(genesis_multi_addr)
        } else {
            let err = String::from_utf8(result.stderr)?;
            Err(eyre!(
                "Genesis node PeerId could not be determined: {err:?}"
            ))
        }
    }

    /// Launches a number of new nodes, either for a new network or an existing network.
    ///
    /// # Arguments
    ///
    /// * `number_of_nodes` - The number of nodes to launch.
    /// * `network_contacts_path` - The path to the network contacts file.
    /// * `node_args` - Additional arguments to pass to the node process, e.g., --json-logs.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * The node data directories cannot be created
    /// * The node process fails
    pub fn launch_nodes(
        &mut self,
        number_of_nodes: usize,
        network_contacts_path: &Path,
        node_args: Vec<String>,
    ) -> Result<()> {
        let start = self.node_count + 2;
        let end = self.node_count + number_of_nodes;
        for i in start..=end {
            info!("Launching node {i} of {end}...");
            let node_data_dir_path = self
                .nodes_dir_path
                .join(format!("safenode-{i}"))
                .to_str()
                .ok_or_else(|| eyre!("Unable to obtain node data directory path"))?
                .to_string();
            std::fs::create_dir_all(&node_data_dir_path)?;

            let rpc_address = format!("127.0.0.1:{}", 12000 + i).parse()?;

            let launch_args = self.get_launch_args(
                format!("safenode-{i}"),
                Some(rpc_address),
                Some(network_contacts_path),
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
        _network_contacts_path: Option<&Path>,
        node_args: Vec<String>,
    ) -> Result<Vec<String>> {
        let node_data_dir_path = self.nodes_dir_path.join(node_name.clone());
        let mut launch_args = Vec::new();
        if self.flamegraph_mode {
            launch_args.push("flamegraph".to_string());
            launch_args.push("--output".to_string());
            launch_args.push(
                node_data_dir_path
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

        let node_data_dir_path = node_data_dir_path
            .to_str()
            .ok_or_else(|| eyre!("Unable to obtain node data directory path"))?
            .to_string();
        launch_args.push("--log-dir".to_string());
        launch_args.push(node_data_dir_path.clone());
        launch_args.push("--root-dir".to_string());
        launch_args.push(node_data_dir_path);
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
    use mockall::predicate::*;

    const NODE_LAUNCH_INTERVAL: u64 = 0;

    #[test]
    fn new_should_create_a_testnet_with_zero_nodes_when_no_previous_network_exists() -> Result<()> {
        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));

        let testnet = Testnet::new(
            PathBuf::from(SAFENODE_BIN_NAME),
            30000,
            PathBuf::from(TESTNET_DIR_NAME),
            false,
            Box::new(node_launcher),
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

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));
        let testnet = Testnet::new(
            PathBuf::from(SAFENODE_BIN_NAME),
            30000,
            nodes_dir.to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;

        assert_eq!(testnet.node_bin_path, PathBuf::from(SAFENODE_BIN_NAME));
        assert_eq!(testnet.node_launch_interval, 30000);
        assert_eq!(testnet.nodes_dir_path, nodes_dir.to_path_buf());
        assert!(!testnet.flamegraph_mode);
        assert_eq!(testnet.node_count, 20);

        Ok(())
    }

    #[test]
    fn new_should_create_a_testnet_ignoring_random_directories_in_the_node_data_dir() -> Result<()>
    {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_data_dir = nodes_dir.child("safenode-1");
        genesis_data_dir.create_dir_all()?;
        for i in 1..=20 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.create_dir_all()?;
        }
        let random_dir = nodes_dir.child("user-created-random-dir");
        random_dir.create_dir_all()?;

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));

        let testnet = Testnet::new(
            PathBuf::from(SAFENODE_BIN_NAME),
            30000,
            nodes_dir.to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;

        assert_eq!(testnet.node_bin_path, PathBuf::from(SAFENODE_BIN_NAME));
        assert_eq!(testnet.node_launch_interval, 30000);
        assert_eq!(testnet.nodes_dir_path, nodes_dir.to_path_buf());
        assert!(!testnet.flamegraph_mode);
        assert_eq!(testnet.node_count, 20);

        Ok(())
    }

    #[test]
    fn launch_genesis_should_launch_the_genesis_node() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let genesis_data_dir = nodes_dir
            .child(GENESIS_NODE_DIR_NAME)
            .to_str()
            .ok_or_else(|| eyre!("Unable to obtain path"))?
            .to_string();

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher
            .expect_launch()
            .times(1)
            .with(
                eq(node_bin_path.path().to_path_buf()),
                eq(vec![
                    "--first".to_string(),
                    "10.0.0.1:12000".to_string(),
                    "--local-addr".to_string(),
                    "0.0.0.0:12000".to_string(),
                    "--root-dir".to_string(),
                    genesis_data_dir.clone(),
                    "--log-dir".to_string(),
                    genesis_data_dir,
                    "--json-logs".to_string(),
                ]),
            )
            .returning(|_, _| Ok(()));

        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_genesis(
            Some("10.0.0.1:12000".parse()?),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn launch_genesis_should_launch_the_genesis_node_as_a_local_network() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let genesis_data_dir = nodes_dir
            .child(GENESIS_NODE_DIR_NAME)
            .to_str()
            .ok_or_else(|| eyre!("Unable to obtain path"))?
            .to_string();

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher
            .expect_launch()
            .times(1)
            .with(
                eq(node_bin_path.path().to_path_buf()),
                eq(vec![
                    "--first".to_string(),
                    "127.0.0.1:12000".to_string(),
                    "--local-addr".to_string(),
                    "0.0.0.0:12000".to_string(),
                    "--root-dir".to_string(),
                    genesis_data_dir.clone(),
                    "--log-dir".to_string(),
                    genesis_data_dir,
                    "--json-logs".to_string(),
                ]),
            )
            .returning(|_, _| Ok(()));

        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_genesis(None, vec!["--json-logs".to_string()]);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn launch_genesis_should_create_the_genesis_data_directory() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));
        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_genesis(
            Some("10.0.0.1:12000".parse()?),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        let genesis_data_dir = nodes_dir.child(GENESIS_NODE_DIR_NAME);
        genesis_data_dir.assert(predicates::path::is_dir());
        Ok(())
    }

    #[test]
    fn launch_genesis_should_create_the_genesis_data_directory_when_parents_are_missing(
    ) -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));
        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_genesis(
            Some("10.0.0.1:12000".parse()?),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        let genesis_data_dir = nodes_dir.child(GENESIS_NODE_DIR_NAME);
        genesis_data_dir.assert(predicates::path::is_dir());
        Ok(())
    }

    #[test]
    fn launch_genesis_with_flamegraph_mode_should_launch_the_genesis_node() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let genesis_data_dir = nodes_dir.child(GENESIS_NODE_DIR_NAME);
        let graph_output_file =
            genesis_data_dir.child(format!("{GENESIS_NODE_DIR_NAME}-flame.svg"));
        let genesis_data_dir_str = nodes_dir
            .child(GENESIS_NODE_DIR_NAME)
            .to_str()
            .ok_or_else(|| eyre!("Unable to obtain path"))?
            .to_string();

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
                    "--first".to_string(),
                    "10.0.0.1:12000".to_string(),
                    "--local-addr".to_string(),
                    "0.0.0.0:12000".to_string(),
                    "--root-dir".to_string(),
                    genesis_data_dir_str.clone(),
                    "--log-dir".to_string(),
                    genesis_data_dir_str,
                    "--json-logs".to_string(),
                ]),
            )
            .returning(|_, _| Ok(()));

        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            true,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_genesis(
            Some("10.0.0.1:12000".parse()?),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn launch_genesis_should_return_error_if_we_are_using_an_existing_network() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let genesis_data_dir = nodes_dir.child(GENESIS_NODE_DIR_NAME);
        genesis_data_dir.create_dir_all()?;
        for i in 1..=20 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.create_dir_all()?;
        }

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));

        let testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_genesis(
            Some("10.0.0.1:12000".parse()?),
            vec!["--json-logs".to_string()],
        );

        match result {
            Ok(_) => Err(eyre!("This test should return an error")),
            Err(e) => {
                assert_eq!(
                    e.to_string(),
                    "A genesis node cannot be launched for an existing network"
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
        nodes_dir.create_dir_all()?;
        let network_contacts_file = tmp_data_dir.child("network-contacts");
        network_contacts_file.write_str("section tree content")?;

        let mut node_launcher = MockNodeLauncher::new();
        for i in 1..=20 {
            let node_data_dir = nodes_dir
                .join(&format!("safenode-{i}"))
                .to_str()
                .ok_or_else(|| eyre!("Unable to obtain path"))?
                .to_string();
            node_launcher
                .expect_launch()
                .times(1)
                .with(
                    eq(node_bin_path.path().to_path_buf()),
                    eq(vec![
                        "--network-contacts-file".to_string(),
                        network_contacts_file.path().to_str().unwrap().to_string(),
                        "--root-dir".to_string(),
                        node_data_dir.clone(),
                        "--log-dir".to_string(),
                        node_data_dir,
                        "--json-logs".to_string(),
                    ]),
                )
                .returning(|_, _| Ok(()));
        }

        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_nodes(
            20,
            network_contacts_file.path(),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        assert_eq!(testnet.node_count, 20);
        Ok(())
    }

    #[test]
    fn launch_nodes_should_create_directories_for_each_node() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let network_contacts_file = tmp_data_dir.child("network-contacts");
        network_contacts_file.write_str("section tree content")?;

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));
        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_nodes(
            20,
            network_contacts_file.path(),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        for i in 1..=20 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.assert(predicates::path::is_dir());
        }

        Ok(())
    }

    #[test]
    fn launch_nodes_should_create_directories_when_parents_are_missing() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        let network_contacts_file = tmp_data_dir.child("network-contacts");
        network_contacts_file.write_str("section tree content")?;

        let mut node_launcher = MockNodeLauncher::new();
        node_launcher.expect_launch().returning(|_, _| Ok(()));
        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_nodes(
            20,
            network_contacts_file.path(),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        for i in 2..=20 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.assert(predicates::path::is_dir());
        }

        Ok(())
    }

    #[test]
    fn launch_nodes_with_flamegraph_should_launch_the_specified_number_of_nodes() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let network_contacts_file = tmp_data_dir.child("network-contacts");
        network_contacts_file.write_str("section tree content")?;

        let mut node_launcher = MockNodeLauncher::new();
        for i in 1..=20 {
            let node_data_dir = nodes_dir.join(&format!("safenode-{i}"));
            let graph_output_file_path = node_data_dir
                .join(format!("safenode-{i}-flame.svg"))
                .to_str()
                .ok_or_else(|| eyre!("Unable to obtain path"))?
                .to_string();
            let node_data_dir = node_data_dir
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
                        "--network-contacts-file".to_string(),
                        network_contacts_file.path().to_str().unwrap().to_string(),
                        "--root-dir".to_string(),
                        node_data_dir.clone(),
                        "--log-dir".to_string(),
                        node_data_dir,
                        "--json-logs".to_string(),
                    ]),
                )
                .returning(|_, _| Ok(()));
        }

        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            true,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_nodes(
            20,
            network_contacts_file.path(),
            vec!["--json-logs".to_string()],
        );

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn launch_nodes_should_launch_the_specified_number_of_additional_nodes() -> Result<()> {
        let tmp_data_dir = assert_fs::TempDir::new()?;
        let node_bin_path = tmp_data_dir.child(SAFENODE_BIN_NAME);
        node_bin_path.write_binary(b"fake safenode code")?;
        let nodes_dir = tmp_data_dir.child(TESTNET_DIR_NAME);
        nodes_dir.create_dir_all()?;
        let network_contacts_file = tmp_data_dir.child("network-contacts");
        network_contacts_file.write_str("section tree content")?;

        let mut node_launcher = MockNodeLauncher::new();
        for i in 1..=30 {
            let node_data_dir = nodes_dir
                .join(&format!("safenode-{i}"))
                .to_str()
                .ok_or_else(|| eyre!("Unable to obtain path"))?
                .to_string();
            node_launcher
                .expect_launch()
                .times(1)
                .with(
                    eq(node_bin_path.path().to_path_buf()),
                    eq(vec![
                        "--network-contacts-file".to_string(),
                        network_contacts_file.path().to_str().unwrap().to_string(),
                        "--root-dir".to_string(),
                        node_data_dir.clone(),
                        "--log-dir".to_string(),
                        node_data_dir,
                        "--json-logs".to_string(),
                    ]),
                )
                .returning(|_, _| Ok(()));
        }

        let mut testnet = Testnet::new(
            node_bin_path.path().to_path_buf(),
            NODE_LAUNCH_INTERVAL,
            nodes_dir.path().to_path_buf(),
            false,
            Box::new(node_launcher),
        )?;
        let result = testnet.launch_nodes(
            20,
            network_contacts_file.path(),
            vec!["--json-logs".to_string()],
        );
        assert!(result.is_ok());
        assert_eq!(testnet.node_count, 20);

        let result = testnet.launch_nodes(
            10,
            network_contacts_file.path(),
            vec!["--json-logs".to_string()],
        );
        assert!(result.is_ok());
        assert_eq!(testnet.node_count, 30);
        for i in 1..=30 {
            let node_dir = nodes_dir.child(format!("safenode-{i}"));
            node_dir.assert(predicates::path::is_dir());
        }
        Ok(())
    }
}
