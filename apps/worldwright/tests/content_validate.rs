use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
const BIN: &str = env!("CARGO_BIN_EXE_worldwright");
const EXAMPLE: &[u8] = include_bytes!("../../../examples/two-room/world.json");
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "worldwright cli {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run(path: &Path, cwd: &Path) -> Output {
    Command::new(BIN)
        .current_dir(cwd)
        .args(["content", "validate"])
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn valid_file_with_spaces_from_outside_repository() {
    let temp = Temp::new();
    let path = temp.file("a world.json", EXAMPLE);
    for path in [path.as_path(), Path::new("a world.json")] {
        let output = run(path, &temp.0);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let text = String::from_utf8_lossy(&output.stdout);
        for expected in [
            "Schema: 1",
            "Rooms: 2",
            "Directed exits: 2",
            "Entry room: reference:rooms.atrium",
        ] {
            assert!(text.contains(expected));
        }
    }
}
#[test]
fn errors_use_stderr_and_exit_one() {
    let temp = Temp::new();
    let bad = temp.file("bad.json", b"{");
    for (path, code) in [
        (bad, "WWC_JSON"),
        (temp.0.join("absent.json"), "WWC_IO"),
        (temp.0.clone(), "WWC_IO"),
    ] {
        let output = run(&path, &temp.0);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
    }
}
#[test]
fn bad_reference_has_actionable_pointer() {
    let temp = Temp::new();
    let data = std::str::from_utf8(EXAMPLE).unwrap().replacen(
        "\"north\": \"reference:rooms.observatory\"",
        "\"north\": \"reference:rooms.missing\"",
        1,
    );
    let path = temp.file("broken.json", data.as_bytes());
    let output = run(&path, &temp.0);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("WWC_UNKNOWN_ROOM"));
    assert!(text.contains("/rooms/0/exits/north"));
}
#[test]
fn oversized_file_is_rejected_before_parsing() {
    let temp = Temp::new();
    let path = temp.file("large.json", b"");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len((worldwright_content::MAX_INPUT_BYTES + 1) as u64)
        .unwrap();
    let output = run(&path, &temp.0);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("WWC_LIMIT"));
}
#[cfg(unix)]
#[test]
fn non_utf8_path_and_terminal_controls_are_handled() {
    use std::os::unix::ffi::OsStringExt;
    let temp = Temp::new();
    let path = temp.0.join(std::ffi::OsString::from_vec(
        b"world-\xff-\x1b.json".to_vec(),
    ));
    std::fs::write(&path, EXAMPLE).unwrap();
    let output = run(&path, &temp.0);
    assert_eq!(output.status.code(), Some(0));
    assert!(!output.stdout.contains(&27));
    let output = Command::new(BIN)
        .arg("--help")
        .arg(std::ffi::OsString::from_vec(vec![255]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
