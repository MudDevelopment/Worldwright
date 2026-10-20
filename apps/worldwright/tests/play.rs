use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
const BIN: &str = env!("CARGO_BIN_EXE_worldwright");
const EXAMPLE: &[u8] = include_bytes!("../../../examples/two-room/world.json");
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "worldwright play {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn world(&self, source: &[u8]) -> PathBuf {
        let path = self.0.join("a world.json");
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run(path: &Path, cwd: &Path, input: &[u8]) -> Output {
    let mut child = Command::new(BIN)
        .arg("play")
        .arg(path)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let data = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&data));
    let result = child.wait_with_output().unwrap();
    let written = writer.join().unwrap();
    assert!(written.is_ok() || written.unwrap_err().kind() == std::io::ErrorKind::BrokenPipe);
    result
}
fn text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}
#[test]
fn complete_script_runs_from_outside_repository() {
    let temp = Temp::new();
    temp.world(EXAMPLE);
    let output = run(
        Path::new("a world.json"),
        &temp.0,
        b"look\ngo north\nlook\nsay Hello from the observatory\ngo south\nquit\n",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = text(&output);
    assert_eq!(text.matches("You move to the next room.").count(), 2);
    assert!(text.contains("Observatory\nA telescope points toward the stars.\nExits: south"));
    assert!(text.contains("Hello from the observatory\n"));
    assert!(text.ends_with("Atrium\nSunlight fills a quiet entry hall.\nExits: north\nGoodbye.\n"));
    assert!(!text.contains("> "));
}
#[test]
fn initial_view_does_not_dump_other_rooms_or_ids() {
    let temp = Temp::new();
    let path = temp.world(EXAMPLE);
    let output = run(&path, &temp.0, b"");
    assert_eq!(output.status.code(), Some(0));
    let text = text(&output);
    assert!(text.contains("Atrium\nSunlight fills a quiet entry hall."));
    assert!(!text.contains("Observatory"));
    assert!(!text.contains("reference:rooms"));
    assert!(text.ends_with("Goodbye.\n"));
}
#[test]
fn honors_declared_entry_room() {
    let source = std::str::from_utf8(EXAMPLE).unwrap().replacen(
        "\"entry_room\": \"reference:rooms.atrium\"",
        "\"entry_room\": \"reference:rooms.observatory\"",
        1,
    );
    let temp = Temp::new();
    let path = temp.world(source.as_bytes());
    let output = run(&path, &temp.0, b"exit");
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output).contains("Observatory\n"));
    assert!(!text(&output).contains("Atrium\n"));
}
#[test]
fn one_way_reverse_and_invalid_input_do_not_move_player() {
    let source = std::str::from_utf8(EXAMPLE)
        .unwrap()
        .replace("\"south\": \"reference:rooms.atrium\"", "");
    let temp = Temp::new();
    let path = temp.world(source.as_bytes());
    let output = run(
        &path,
        &temp.0,
        b"go north\ngo south\nxyz\ngo\nsay\nlook extra\nquit extra\n\nhelp\nl\nexit\n",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        text(&output).matches("You move to the next room.").count(),
        1
    );
    assert!(
        text(&output).ends_with(
            "Observatory\nA telescope points toward the stars.\nExits: (none)\nGoodbye.\n"
        )
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    for expected in [
        "No such exit",
        "Unknown command",
        "Usage: go",
        "Usage: say",
        "takes no arguments",
    ] {
        assert!(errors.contains(expected));
    }
}
#[test]
fn crlf_and_unterminated_unicode_say_at_eof() {
    let temp = Temp::new();
    let path = temp.world(EXAMPLE);
    let output = run(
        &path,
        &temp.0,
        "\r\nhelp\r\ngo north\r\nsay café 世界".as_bytes(),
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(text(&output).ends_with("café 世界\nGoodbye.\n"));
}
#[test]
fn malformed_missing_and_directory_content_fail_before_session() {
    let temp = Temp::new();
    let path = temp.world(b"{");
    for path in [path, temp.0.join("missing.json"), temp.0.clone()] {
        let output = run(&path, &temp.0, b"");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("WWC_"));
    }
}
#[test]
fn oversized_invalid_encoding_and_control_lines_are_discarded() {
    let temp = Temp::new();
    let path = temp.world(EXAMPLE);
    let mut input = vec![b'x'; 64 * 1024];
    input.extend_from_slice(b"go north\nsay \xff\nsay hidden\x1b[31m\nlook\nquit\n");
    let output = run(&path, &temp.0, &input);
    assert_eq!(output.status.code(), Some(0));
    assert!(!text(&output).contains("You move"));
    assert!(!text(&output).contains("hidden"));
    assert!(!output.stdout.contains(&27));
    assert!(!output.stderr.contains(&27));
    let errors = String::from_utf8_lossy(&output.stderr);
    for expected in ["4100", "UTF-8", "Control characters"] {
        assert!(errors.contains(expected));
    }
    assert_eq!(text(&output).matches("Atrium\n").count(), 2);
}
#[test]
fn full_size_say_is_not_truncated() {
    let temp = Temp::new();
    let path = temp.world(EXAMPLE);
    let body = "é".repeat(2048);
    let output = run(&path, &temp.0, format!("say {body}\r\nquit\n").as_bytes());
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(text(&output).contains(&format!("{body}\n")));
}
#[test]
fn quit_ignores_following_commands() {
    let temp = Temp::new();
    let path = temp.world(EXAMPLE);
    let output = run(&path, &temp.0, b"quit\ngo north\nsay should-not-run\n");
    assert_eq!(output.status.code(), Some(0));
    assert!(!text(&output).contains("You move"));
    assert!(!text(&output).contains("should-not-run"));
}
#[test]
fn play_help_and_usage() {
    for flag in ["--help", "-h"] {
        let output = Command::new(BIN).args(["play", flag]).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(text(&output).contains("play <path>"));
    }
    for args in [
        vec!["play"],
        vec!["play", ""],
        vec!["play", "file", "extra"],
        vec!["play", "--help", "extra"],
    ] {
        let output = Command::new(BIN).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stderr.is_empty());
    }
}
#[cfg(unix)]
#[test]
fn non_utf8_content_path_is_supported() {
    use std::os::unix::ffi::OsStringExt;
    let temp = Temp::new();
    let path = temp
        .0
        .join(std::ffi::OsString::from_vec(b"world-\xff.json".to_vec()));
    std::fs::write(&path, EXAMPLE).unwrap();
    let output = run(&path, &temp.0, b"quit\n");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
}
