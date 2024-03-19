// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod utils;

use assert_cmd::Command;
use color_eyre::Result;
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use utils::get_service_status;

const CI_USER: &str = "runner";

/// These tests need to execute as the root user.
///
/// They are intended to run on a CI-based environment with a fresh build agent because they will
/// create real services and user accounts, and will not attempt to clean themselves up.
///
/// Each test also needs to run in isolation, otherwise they will interfere with each other.
///
/// If you run them on your own dev machine, do so at your own risk!

#[tokio::test]
async fn upgrade_to_latest_version() -> Result<()> {
    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("add")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .arg("--peer")
        .arg("/ip4/127.0.0.1/udp/46091/p2p/12D3KooWAWnbQLxqspWeB3M8HB3ab3CSj6FYzsJxEG9XdVnGNCod")
        .arg("--version")
        .arg("0.98.27")
        .assert()
        .success();

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|node| node.version == "0.98.27"),
        "Services were not correctly initialised"
    );

    let release_repo = <dyn SafeReleaseRepoActions>::default_config();
    let latest_version = release_repo
        .get_latest_version(&ReleaseType::Safenode)
        .await?;
    let mut cmd = Command::cargo_bin("safenode-manager")?;
    let output = cmd
        .arg("upgrade")
        .arg("--do-not-start")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output = std::str::from_utf8(&output)?;
    println!("upgrade command output:");
    println!("{output}");

    let status = get_service_status().await?;
    assert!(
        status
            .nodes
            .iter()
            .all(|n| n.version == latest_version.to_string()),
        "Not all services were updated to the latest version"
    );

    Ok(())
}

/// This scenario may seem pointless, but forcing a change for a binary with the same version will
/// be required for the backwards compatibility test; the binary will be different, it will just
/// have the same version.
#[tokio::test]
async fn force_upgrade_when_two_binaries_have_the_same_version() -> Result<()> {
    let version = "0.98.27";

    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("add")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .arg("--peer")
        .arg("/ip4/127.0.0.1/udp/46091/p2p/12D3KooWAWnbQLxqspWeB3M8HB3ab3CSj6FYzsJxEG9XdVnGNCod")
        .arg("--version")
        .arg(version)
        .assert()
        .success();

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|n| n.version == version),
        "Services were not correctly initialised"
    );

    let mut cmd = Command::cargo_bin("safenode-manager")?;
    let output = cmd
        .arg("upgrade")
        .arg("--do-not-start")
        .arg("--force")
        .arg("--version")
        .arg(version)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output = std::str::from_utf8(&output)?;
    println!("upgrade command output:");
    println!("{output}");

    assert!(output.contains(&format!(
        "Forced safenode1 version change from {version} to {version}"
    )));
    assert!(output.contains(&format!(
        "Forced safenode2 version change from {version} to {version}"
    )));
    assert!(output.contains(&format!(
        "Forced safenode3 version change from {version} to {version}"
    )));

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|n| n.version == version),
        "Not all services were updated to the latest version"
    );

    Ok(())
}

#[tokio::test]
async fn force_downgrade_to_a_previous_version() -> Result<()> {
    let initial_version = "0.104.15";
    let downgrade_version = "0.104.10";

    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("add")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .arg("--peer")
        .arg("/ip4/127.0.0.1/udp/46091/p2p/12D3KooWAWnbQLxqspWeB3M8HB3ab3CSj6FYzsJxEG9XdVnGNCod")
        .arg("--version")
        .arg(initial_version)
        .assert()
        .success();

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|n| n.version == initial_version),
        "Services were not correctly initialised"
    );

    let mut cmd = Command::cargo_bin("safenode-manager")?;
    let output = cmd
        .arg("upgrade")
        .arg("--do-not-start")
        .arg("--force")
        .arg("--version")
        .arg(downgrade_version)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output = std::str::from_utf8(&output)?;
    println!("upgrade command output:");
    println!("{output}");

    assert!(output.contains(&format!(
        "Forced safenode1 version change from {initial_version} to {downgrade_version}"
    )));
    assert!(output.contains(&format!(
        "Forced safenode2 version change from {initial_version} to {downgrade_version}"
    )));
    assert!(output.contains(&format!(
        "Forced safenode3 version change from {initial_version} to {downgrade_version}"
    )));

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|n| n.version == downgrade_version),
        "Not all services were updated to the latest version"
    );

    Ok(())
}

#[tokio::test]
async fn upgrade_from_older_version_to_specific_version() -> Result<()> {
    let initial_version = "0.104.10";
    let upgrade_version = "0.104.14";

    let mut cmd = Command::cargo_bin("safenode-manager")?;
    cmd.arg("add")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .arg("--peer")
        .arg("/ip4/127.0.0.1/udp/46091/p2p/12D3KooWAWnbQLxqspWeB3M8HB3ab3CSj6FYzsJxEG9XdVnGNCod")
        .arg("--version")
        .arg(initial_version)
        .assert()
        .success();

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|n| n.version == initial_version),
        "Services were not correctly initialised"
    );

    let mut cmd = Command::cargo_bin("safenode-manager")?;
    let output = cmd
        .arg("upgrade")
        .arg("--do-not-start")
        .arg("--version")
        .arg(upgrade_version)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output = std::str::from_utf8(&output)?;
    println!("upgrade command output:");
    println!("{output}");

    assert!(output.contains(&format!(
        "safenode1 upgraded from {initial_version} to {upgrade_version}"
    )));
    assert!(output.contains(&format!(
        "safenode2 upgraded from {initial_version} to {upgrade_version}"
    )));
    assert!(output.contains(&format!(
        "safenode3 upgraded from {initial_version} to {upgrade_version}"
    )));

    let status = get_service_status().await?;
    assert!(
        status.nodes.iter().all(|n| n.version == upgrade_version),
        "Not all services were updated to the latest version"
    );

    Ok(())
}
