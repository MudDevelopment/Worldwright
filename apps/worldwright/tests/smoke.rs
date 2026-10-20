use std::process::Command;
const BIN: &str = env!("CARGO_BIN_EXE_worldwright");

#[test]
fn help_and_status_describe_implemented_local_scope() {
    for flag in ["--help", "-h", "status"] {
        let output = Command::new(BIN).arg(flag).output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("content"));
        assert!(text.contains("no network service"));
        assert!(!text.contains("scaffold only"));
    }
}
#[test]
fn version_is_package_version() {
    for flag in ["--version", "-V"] {
        let output = Command::new(BIN).arg(flag).output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
    }
}
#[test]
fn invalid_usage_is_rejected() {
    for args in [
        vec![],
        vec!["serve"],
        vec!["validate"],
        vec!["status", "extra"],
        vec!["content"],
        vec!["content", "validate"],
        vec!["content", "unknown"],
        vec!["content", "validate", "a", "b"],
        vec!["content", "validate", ""],
    ] {
        let output = Command::new(BIN).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stderr.is_empty());
    }
}
#[test]
fn command_help_succeeds() {
    for args in [
        ["content", "--help"].as_slice(),
        &["content", "validate", "--help"],
        &["content", "validate", "-h"],
    ] {
        let output = Command::new(BIN).args(args).output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("content validate <path>"));
    }
}
