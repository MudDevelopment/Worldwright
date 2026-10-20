// Shared integration-test body, included by each app's tests/smoke.rs.
// A virtual workspace does not automatically run tests placed at its root.

use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    Command::new(BIN)
        .args(arguments)
        .output()
        .expect("scaffold binary should be executable")
}

#[test]
fn help_is_successful_and_explicit_about_scaffold_status() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("scaffold only"));
    }
}

#[test]
fn version_comes_from_the_package() {
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
    }
}

#[test]
fn status_does_not_claim_operational_readiness() {
    let output = run(&["status"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("scaffold only"));
}

#[test]
fn default_startup_fails_closed() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}

#[test]
fn unsupported_operations_and_extra_arguments_are_rejected() {
    for arguments in [vec!["serve"], vec!["validate"], vec!["status", "unexpected"]] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stderr.is_empty());
    }
}
