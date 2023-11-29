// Copyright (C) 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use assert_cmd::Command;
use libp2p_identity::PeerId;
use std::str::FromStr;

/// These tests need to execute as the root user.
///
/// They are intended to run on a CI-based environment with a fresh build agent because they will
/// create real services and user accounts, and will not attempt to clean themselves up.
///
/// If you run them on your own dev machine, do so at your own risk!

const CI_USER: &str = "runner";

#[derive(Debug)]
struct ServiceStatus {
    name: String,
    peer_id: String,
    status: String,
}

/// The default behaviour is for the service to run as the `safe` user, which gets created during
/// the process. However, there seems to be some sort of issue with adding user accounts on the GHA
/// build agent, so we will just tell it to use the `runner` user, which is the account for the
/// build agent.
#[test]
fn cross_platform_service_install_and_control() {
    // An explicit version of `safenode` will be used to avoid any rate limiting from Github when
    // retrieving the latest version number.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("install")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .arg("--version")
        .arg("0.98.27")
        .assert()
        .success();

    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let service_status = parse_service_status(&output.stdout);

    assert_eq!(service_status[0].name, "safenode1");
    assert_eq!(service_status[0].peer_id, "-");
    assert_eq!(service_status[0].status, "INSTALLED");
    assert_eq!(service_status[1].name, "safenode2");
    assert_eq!(service_status[1].peer_id, "-");
    assert_eq!(service_status[1].status, "INSTALLED");
    assert_eq!(service_status[2].name, "safenode3");
    assert_eq!(service_status[2].peer_id, "-");
    assert_eq!(service_status[2].status, "INSTALLED");

    // Start each of the three installed services.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("start").assert().success();
    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let start_status = parse_service_status(&output.stdout);

    // After `start`, all services should be running with valid peer IDs assigned.
    assert_eq!(start_status[0].name, "safenode1");
    assert_eq!(start_status[0].status, "RUNNING");
    assert_eq!(start_status[1].name, "safenode2");
    assert_eq!(start_status[1].status, "RUNNING");
    assert_eq!(start_status[2].name, "safenode3");
    assert_eq!(start_status[2].status, "RUNNING");
    for status in start_status.iter() {
        assert!(PeerId::from_str(&status.peer_id).is_ok());
    }

    // The three peer IDs should persist throughout the rest of the test.
    let peer_ids = start_status
        .iter()
        .map(|s| s.peer_id.clone())
        .collect::<Vec<String>>();

    // Stop each of the three installed services.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("stop").assert().success();
    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let stop_status = parse_service_status(&output.stdout);

    // After `stop`, all services should be stopped with peer IDs retained.
    assert_eq!(stop_status[0].name, "safenode1");
    assert_eq!(stop_status[0].status, "STOPPED");
    assert_eq!(stop_status[0].peer_id, peer_ids[0]);
    assert_eq!(stop_status[1].name, "safenode2");
    assert_eq!(stop_status[1].status, "STOPPED");
    assert_eq!(stop_status[1].peer_id, peer_ids[1]);
    assert_eq!(stop_status[2].name, "safenode3");
    assert_eq!(stop_status[2].status, "STOPPED");
    assert_eq!(stop_status[2].peer_id, peer_ids[2]);

    // Start each of the three installed services again.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("start").assert().success();
    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let start_status = parse_service_status(&output.stdout);

    // Peer IDs again should be retained after restart.
    assert_eq!(start_status[0].name, "safenode1");
    assert_eq!(start_status[0].status, "RUNNING");
    assert_eq!(start_status[0].peer_id, peer_ids[0]);
    assert_eq!(start_status[1].name, "safenode2");
    assert_eq!(start_status[1].status, "RUNNING");
    assert_eq!(start_status[1].peer_id, peer_ids[1]);
    assert_eq!(start_status[2].name, "safenode3");
    assert_eq!(start_status[2].status, "RUNNING");
    assert_eq!(start_status[2].peer_id, peer_ids[2]);

    // Stop one node by peer ID.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("stop")
        .arg("--peer-id")
        .arg(start_status[1].peer_id.clone())
        .assert()
        .success();
    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let single_node_stop_status = parse_service_status(&output.stdout);

    // Peer IDs again should be retained after restart.
    assert_eq!(single_node_stop_status[0].name, "safenode1");
    assert_eq!(single_node_stop_status[0].status, "RUNNING");
    assert_eq!(single_node_stop_status[0].peer_id, peer_ids[0]);
    assert_eq!(single_node_stop_status[1].name, "safenode2");
    assert_eq!(single_node_stop_status[1].status, "STOPPED");
    assert_eq!(single_node_stop_status[1].peer_id, peer_ids[1]);
    assert_eq!(single_node_stop_status[2].name, "safenode3");
    assert_eq!(single_node_stop_status[2].status, "RUNNING");
    assert_eq!(single_node_stop_status[2].peer_id, peer_ids[2]);

    // Now restart the single stopped node.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("start")
        .arg("--peer-id")
        .arg(single_node_stop_status[1].peer_id.clone())
        .assert()
        .success();
    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let single_node_start_status = parse_service_status(&output.stdout);

    // The individually stopped node should now be running again.
    assert_eq!(single_node_start_status[0].name, "safenode1");
    assert_eq!(single_node_start_status[0].status, "RUNNING");
    assert_eq!(single_node_start_status[0].peer_id, peer_ids[0]);
    assert_eq!(single_node_start_status[1].name, "safenode2");
    assert_eq!(single_node_start_status[1].status, "RUNNING");
    assert_eq!(single_node_start_status[1].peer_id, peer_ids[1]);
    assert_eq!(single_node_start_status[2].name, "safenode3");
    assert_eq!(single_node_start_status[2].status, "RUNNING");
    assert_eq!(single_node_start_status[2].peer_id, peer_ids[2]);

    // Finally, stop each of the three installed services.
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("stop").assert().success();
    let output = Command::cargo_bin("safenode-manager")
        .unwrap()
        .arg("status")
        .output()
        .expect("Could not retrieve service status");
    let stop_status = parse_service_status(&output.stdout);

    // After `stop`, all services should be stopped with peer IDs retained.
    assert_eq!(stop_status[0].name, "safenode1");
    assert_eq!(stop_status[0].status, "STOPPED");
    assert_eq!(stop_status[0].peer_id, peer_ids[0]);
    assert_eq!(stop_status[1].name, "safenode2");
    assert_eq!(stop_status[1].status, "STOPPED");
    assert_eq!(stop_status[1].peer_id, peer_ids[1]);
    assert_eq!(stop_status[2].name, "safenode3");
    assert_eq!(stop_status[2].status, "STOPPED");
    assert_eq!(stop_status[2].peer_id, peer_ids[2]);
}

fn parse_service_status(output: &[u8]) -> Vec<ServiceStatus> {
    let output_str = String::from_utf8_lossy(output);
    output_str
        .split('\n')
        .skip(4) // Skip header lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            ServiceStatus {
                name: columns[0].to_string(),
                peer_id: columns[1].to_string(),
                status: columns[2].to_string(),
            }
        })
        .collect()
}
