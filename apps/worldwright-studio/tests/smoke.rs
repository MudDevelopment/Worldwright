const BIN: &str = env!("CARGO_BIN_EXE_worldwright-studio");

#[test]
fn help_version_status_are_implemented() {
    let help = std::process::Command::new(BIN)
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("Worldwright Studio"));
    assert!(help_text.contains("--preview-command"));
    assert!(help_text.contains("--release-endpoint"));
    assert!(!help_text.contains("scaffold"));
    let version = std::process::Command::new(BIN)
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    let status = std::process::Command::new(BIN)
        .arg("status")
        .output()
        .unwrap();
    assert!(status.status.success());
    let status_text = String::from_utf8_lossy(&status.stdout);
    assert!(status_text.contains("draft editing"));
    assert!(status_text.contains("controlled publication"));
}

#[test]
fn invalid_invocations_fail_closed() {
    for args in [
        vec![],
        vec!["wat"],
        vec!["serve"],
        vec!["serve", "--workspace"],
    ] {
        let output = std::process::Command::new(BIN).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}
