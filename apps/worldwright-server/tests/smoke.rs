const BIN: &str = env!("CARGO_BIN_EXE_worldwright-server");

fn run(arguments: &[&str]) -> std::process::Output {
    std::process::Command::new(BIN)
        .args(arguments)
        .output()
        .expect("server binary should be executable")
}

#[test]
fn help_version_and_status_are_successful() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Protocol v1"));
    }
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
    }
    let output = run(&["status"]);
    assert!(output.status.success());
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(status.contains("protocol v1/browser transport is implemented"));
    assert!(status.contains("not a readiness probe"));
}

#[test]
fn missing_command_and_unknown_options_are_usage_errors() {
    for arguments in [
        Vec::<&str>::new(),
        vec!["serve"],
        vec!["serve", "--content", "world.json"],
        vec!["serve", "--unknown"],
        vec!["status", "extra"],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stderr.is_empty());
    }
}
