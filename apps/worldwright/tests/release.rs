use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_worldwright");
const CONTENT: &[u8] = include_bytes!("../../../examples/two-room/world.json");
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "worldwright release cli {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn content(&self) -> PathBuf {
        let path = self.0.join("world.json");
        std::fs::write(&path, CONTENT).unwrap();
        path
    }
    fn artifact(&self) -> PathBuf {
        self.0.join("world.wr")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn package(temp: &Temp, builder: &str) -> Output {
    Command::new(BIN)
        .args(["content", "package"])
        .arg(temp.content())
        .args([
            "--builder",
            builder,
            "--source-revision",
            "revision",
            "--release-id",
            "release-1",
            "--output",
        ])
        .arg(temp.artifact())
        .output()
        .unwrap()
}

#[test]
fn public_preview_supervisor_runs_the_exact_selected_game() {
    for (game, content) in [
        ("reference", CONTENT),
        (
            "expedition",
            include_bytes!("../../../examples/expedition/world.json").as_slice(),
        ),
    ] {
        let temp = Temp::new();
        let input = temp.0.join("selected-game.json");
        std::fs::write(&input, content).unwrap();
        let packaged = Command::new(BIN)
            .args(["content", "package"])
            .arg(&input)
            .args([
                "--builder",
                "p0-test",
                "--source-revision",
                "p0-test",
                "--release-id",
                "p0-preview",
                "--game",
                game,
                "--output",
            ])
            .arg(temp.artifact())
            .output()
            .unwrap();
        assert!(
            packaged.status.success(),
            "{}",
            String::from_utf8_lossy(&packaged.stderr)
        );
        let original = std::fs::read(temp.artifact()).unwrap();
        let inspection = Command::new(BIN)
            .args(["content", "inspect"])
            .arg(temp.artifact())
            .arg("--json")
            .output()
            .unwrap();
        assert!(inspection.status.success());
        assert!(
            String::from_utf8_lossy(&inspection.stdout).contains(&format!("worldwright-{game}"))
        );
        // Use files rather than unread pipes: large room output cannot deadlock
        // the deadline poll on either Unix or Windows anonymous pipe buffers.
        let stdout = temp.0.join("preview.stdout");
        let stderr = temp.0.join("preview.stderr");
        let mut child = Command::new(BIN)
            .arg("preview")
            .arg(temp.artifact())
            .args(["--max-seconds", "5"])
            .stdin(Stdio::piped())
            .stdout(std::fs::File::create(&stdout).unwrap())
            .stderr(std::fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        std::io::Write::write_all(
            &mut child.stdin.take().unwrap(),
            b"status\nforage\nrest\nrank\nquit\n",
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("public preview did not finish within its deadline");
            }
            thread::sleep(Duration::from_millis(20));
        };
        assert!(
            status.success(),
            "{}",
            std::fs::read_to_string(&stderr).unwrap()
        );
        let output = std::fs::read_to_string(stdout).unwrap();
        assert!(output.contains("Artifact digest: "));
        assert!(output.contains("Worldwright Isolated Preview v0.1"));
        if game == "expedition" {
            assert!(output.contains("Supplies: 3 | Threat: 0 | Victories: 0 | Rank: Scout"));
            assert!(output.contains("Foraging recovers"));
            assert!(output.contains("Recovery is scheduled"));
        } else {
            assert!(!output.contains("Supplies:"));
        }
        assert_eq!(std::fs::read(temp.artifact()).unwrap(), original);
    }
}

#[test]
fn unsupported_package_game_cannot_create_an_artifact() {
    let temp = Temp::new();
    let result = Command::new(BIN)
        .args(["content", "package"])
        .arg(temp.content())
        .args([
            "--builder",
            "p0-test",
            "--source-revision",
            "p0-test",
            "--release-id",
            "p0-invalid",
            "--game",
            "unknown",
            "--output",
        ])
        .arg(temp.artifact())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(!temp.artifact().exists());
}

#[test]
fn package_and_inspect_text_and_json_are_consistent() {
    let temp = Temp::new();
    let output = package(&temp, "builder");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Release ID: release-1"));
    assert!(text.contains("Artifact: Created"));
    let inspect = Command::new(BIN)
        .args(["content", "inspect"])
        .arg(temp.artifact())
        .output()
        .unwrap();
    assert_eq!(inspect.status.code(), Some(0));
    let inspection = String::from_utf8(inspect.stdout).unwrap();
    let artifact_digest = inspection
        .lines()
        .find_map(|line| line.strip_prefix("Artifact digest: "))
        .unwrap();
    let json = Command::new(BIN)
        .args(["content", "inspect"])
        .arg(temp.artifact())
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(json.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["artifact_digest"], artifact_digest);
    assert_eq!(value["content"]["namespace"], "reference");
}

#[test]
fn repeated_package_reuses_and_conflicting_package_preserves_bytes() {
    let temp = Temp::new();
    assert_eq!(package(&temp, "builder").status.code(), Some(0));
    let original = std::fs::read(temp.artifact()).unwrap();
    let repeated = package(&temp, "builder");
    assert_eq!(repeated.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&repeated.stdout).contains("Artifact: Reused"));
    let conflict = package(&temp, "different-builder");
    assert_eq!(conflict.status.code(), Some(3));
    assert_eq!(std::fs::read(temp.artifact()).unwrap(), original);
}

#[test]
fn tampered_artifact_and_worker_authentication_fail() {
    let temp = Temp::new();
    assert_eq!(package(&temp, "builder").status.code(), Some(0));
    let mut bytes = std::fs::read(temp.artifact()).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(temp.artifact(), bytes).unwrap();
    let inspect = Command::new(BIN)
        .args(["content", "inspect"])
        .arg(temp.artifact())
        .output()
        .unwrap();
    assert_eq!(inspect.status.code(), Some(1));

    let direct = Command::new(BIN)
        .arg("__preview-worker")
        .arg(temp.artifact())
        .arg("0000000000000000000000000000000000000000000000000000000000000000")
        .arg(
            std::env::temp_dir()
                .join("worldwright-preview-v1")
                .join("slot-0.lock"),
        )
        .arg("00000000000000000000000000000000")
        .arg("10")
        .env_remove("WORLDWRIGHT_PREVIEW")
        .output()
        .unwrap();
    assert_eq!(direct.status.code(), Some(1));
}

#[test]
fn preview_with_piped_quit_identifies_isolated_runtime() {
    let temp = Temp::new();
    assert_eq!(package(&temp, "builder").status.code(), Some(0));
    let mut child = Command::new(BIN)
        .args(["preview"])
        .arg(temp.artifact())
        .env("WORLDWRIGHT_PREVIEW_FORBIDDEN_SENTINEL", "present")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let writer = thread::spawn(move || std::io::Write::write_all(&mut input, b"quit\n"));
    let output = child.wait_with_output().unwrap();
    assert!(writer.join().unwrap().is_ok());
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Worldwright Isolated Preview v0.1"));
    assert!(stdout.contains("Artifact digest: "));
}

#[test]
fn preview_timeout_kills_worker_with_stdin_open() {
    let temp = Temp::new();
    assert_eq!(package(&temp, "builder").status.code(), Some(0));
    let mut child = Command::new(BIN)
        .arg("preview")
        .arg(temp.artifact())
        .args(["--max-seconds", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _stdin = child.stdin.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let output = loop {
        if child.try_wait().unwrap().is_some() {
            break child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("preview timeout supervisor did not finish within five seconds");
        }
        thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("maximum lifetime"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
