use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_worldwright"))
}

fn temp_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "worldwright-m8-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples")
        .join(name)
        .join("world.json")
}

fn host_target() -> String {
    let output = Command::new("rustc").arg("-vV").output().unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap()
        .to_owned()
}

#[test]
fn compatibility_and_hundred_client_workload_are_machine_readable() {
    let output = cli().args(["compatibility", "--json"]).output().unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["sdk_current"], 1);
    assert_eq!(value["minimum_rust"], "1.85.1");

    let output = cli()
        .args([
            "benchmark",
            "--clients",
            "100",
            "--commands-per-client",
            "2",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["clients"], 100);
    assert_eq!(value["commands"], 200);
    assert!(value["p95_ns"].as_u64().is_some());
}

#[test]
fn migration_dry_run_backup_and_create_new_output_are_safe() {
    let root = temp_path("migration");
    fs::create_dir(&root).unwrap();
    let output = root.join("migrated.json");
    let backup = root.join("source.backup.json");
    let input = example("two-room");

    let status = cli()
        .arg("content")
        .arg("migrate")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--backup")
        .arg(&backup)
        .arg("--dry-run")
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!output.exists());
    assert!(!backup.exists());

    let status = cli()
        .arg("content")
        .arg("migrate")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--backup")
        .arg(&backup)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read(&backup).unwrap(), fs::read(&input).unwrap());
    assert!(worldwright_content::load_file(&output, Default::default()).is_ok());
    let preserved = fs::read(&output).unwrap();

    let status = cli()
        .arg("content")
        .arg("migrate")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(3));
    assert_eq!(fs::read(&output).unwrap(), preserved);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_minimal_and_expedition_projects_build_and_test() {
    for template in ["minimal", "expedition"] {
        let parent = temp_path(template);
        fs::create_dir(&parent).unwrap();
        let name = format!("{template}-game");
        let project = parent.join(&name);
        let status = cli()
            .arg("project")
            .arg("new")
            .arg(&project)
            .arg("--name")
            .arg(&name)
            .arg("--template")
            .arg(template)
            .status()
            .unwrap();
        assert!(status.success(), "{template}");
        assert!(project.join("src/main.rs").is_file());
        assert!(project.join("tests/content.rs").is_file());
        assert!(project.join("content/world.json").is_file());
        assert!(project.join("sdk/bundle-manifest.json").is_file());
        let manifest = fs::read_to_string(project.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("sdk/crates/worldwright-sdk"));
        assert!(!manifest.contains(env!("CARGO_MANIFEST_DIR")));
        // Build only after moving the whole project, including its SDK workspace.
        let relocated = parent.join(format!("relocated-{name}"));
        fs::rename(&project, &relocated).unwrap();
        let project = relocated;

        let target = parent.join("target");
        let host = host_target();
        let status = Command::new(env!("CARGO"))
            .current_dir(&project)
            .args(["test", "--offline", "--target", &host])
            .arg("--target-dir")
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success(), "{template}");
        {
            let binary = target
                .join(&host)
                .join("debug")
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            let status = Command::new(env!("CARGO"))
                .current_dir(&project)
                .args(["build", "--locked", "--offline", "--target", &host])
                .arg("--target-dir")
                .arg(&target)
                .status()
                .unwrap();
            assert!(status.success());
            // Execute outside the generated project: content is embedded.
            let mut child = Command::new(binary)
                .current_dir(&parent)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"status\nforage\nrest\nrank\nquit\n")
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            if template == "expedition" {
                assert!(text.contains("Supplies: 3 | Threat: 0 | Victories: 0 | Rank: Scout"));
                assert!(text.contains("Foraging recovers"));
                assert!(text.contains("Recovery is scheduled"));
            } else {
                assert!(text.contains("Worldwright Reference"));
                assert!(!text.contains("Supplies:"));
            }
            assert!(text.contains("Goodbye."));
        }
        fs::remove_dir_all(parent).unwrap();
    }
}
