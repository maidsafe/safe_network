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
#[cfg(target_os = "linux")]
#[test]
fn linux_e2e_install() {
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("install")
        .arg("--user")
        .arg(CI_USER)
        .assert()
        .success();
    assert!(std::path::Path::new("/etc/systemd/system/safenode1.service").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_e2e_install() {
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("install")
        .arg("--user")
        .arg(CI_USER)
        .assert()
        .success();
    let plist_path = "/Library/LaunchDaemons/safenode1.plist";
    assert!(std::path::Path::new(plist_path).exists());
}

#[cfg(target_os = "windows")]
#[test]
fn windows_e2e_install() {
    let mut cmd = Command::cargo_bin("safenode-manager").unwrap();
    cmd.arg("install")
        .arg("--user")
        .arg(CI_USER)
        .assert()
        .success();

    let service_info = std::process::Command::new("sc.exe")
        .arg("query")
        .arg("safenode1")
        .output()
        .unwrap();
    let service_str = String::from_utf8_lossy(&service_info.stdout);
    assert!(service_str.contains("safenode1"));
}
