const BIN: &str = env!("CARGO_BIN_EXE_worldwright-console");

#[test]
fn help_is_successful_and_describes_operator_commands() {
    let output = std::process::Command::new(BIN)
        .arg("--help")
        .output()
        .expect("console should run");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("credential"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("scaffold"));
}

#[test]
fn version_comes_from_package() {
    let output = std::process::Command::new(BIN)
        .arg("--version")
        .output()
        .expect("console should run");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn missing_remote_status_configuration_fails_closed() {
    let output = std::process::Command::new(BIN)
        .arg("status")
        .output()
        .expect("console should run");
    assert_eq!(output.status.code(), Some(2));
}
