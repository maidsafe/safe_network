use assert_cmd::Command;

/// These tests need to execute as the root user.
///
/// They are intended to run on a CI-based environment with a fresh build agent because they will
/// create real services and user accounts, and will not attempt to clean themselves up.
///
/// If you run them on your own dev machine, do so at your own risk!

const CI_USER: &str = "runner";

/// The default behaviour is for the service to run as the `safe` user, which gets created during
/// the process. However, there seems to be some sort of issue with adding user accounts on the GHA
/// build agent, so we will just tell it to use the `runner` user, which is the account for the
/// build agent.
#[test]
fn cross_platform_service_install_and_control() {
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("install")
        .arg("--user")
        .arg(CI_USER)
        .arg("--count")
        .arg("3")
        .assert()
        .success();
    cmd.arg("start").assert().success();
}
