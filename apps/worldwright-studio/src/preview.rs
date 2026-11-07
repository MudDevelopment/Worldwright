use crate::workspace::Workspace;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewLifecycle {
    Disabled,
    Idle,
    Running,
    Succeeded,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreviewStatus {
    pub state: PreviewLifecycle,
    pub artifact_digest: Option<String>,
    pub draft_revision: Option<u64>,
    pub snapshot_sha256: Option<String>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub output: String,
    pub output_sequence: u64,
    pub truncated: bool,
    pub exit_code: Option<i32>,
    pub stale: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewReceipt {
    pub schema_version: u32,
    pub artifact_digest: String,
    pub payload_digest: String,
    pub draft_revision: u64,
    pub completed_at: u64,
    pub result: String,
}

struct RunningPreview {
    child: Child,
    artifact: String,
    payload: String,
    revision: u64,
}

#[derive(Clone)]
pub struct PreviewManager {
    workspace: Workspace,
    command: Option<PathBuf>,
    state: Arc<Mutex<Inner>>,
}
struct Inner {
    status: PreviewStatus,
    running: Option<RunningPreview>,
}

impl PreviewManager {
    pub fn new(workspace: Workspace, command: Option<PathBuf>) -> Self {
        let state = if command.is_some() {
            PreviewLifecycle::Idle
        } else {
            PreviewLifecycle::Disabled
        };
        Self {
            workspace,
            command,
            state: Arc::new(Mutex::new(Inner {
                status: PreviewStatus {
                    state,
                    artifact_digest: None,
                    draft_revision: None,
                    snapshot_sha256: None,
                    started_at: None,
                    completed_at: None,
                    output: String::new(),
                    output_sequence: 0,
                    truncated: false,
                    exit_code: None,
                    stale: false,
                },
                running: None,
            })),
        }
    }
    pub fn status(&self) -> PreviewStatus {
        let Ok(mut inner) = self.state.lock() else {
            return PreviewStatus {
                state: PreviewLifecycle::Failed,
                artifact_digest: None,
                draft_revision: None,
                snapshot_sha256: None,
                started_at: None,
                completed_at: Some(now()),
                output: "preview state unavailable".into(),
                output_sequence: 0,
                truncated: false,
                exit_code: None,
                stale: false,
            };
        };
        self.refresh_locked(&mut inner);
        if let (Some(revision), Some(pinned)) = (
            inner.status.draft_revision,
            inner.status.snapshot_sha256.as_deref(),
        ) {
            if let Ok(latest) = self.workspace.latest() {
                inner.status.stale =
                    latest.revision != revision || latest.snapshot_sha256 != pinned;
            }
        }
        inner.status.clone()
    }
    pub fn start(&self, digest: &str, max_seconds: u64) -> Result<PreviewStatus, String> {
        if !(1..=3600).contains(&max_seconds) {
            return Err("max_seconds out of bounds".into());
        }
        let artifact = self
            .workspace
            .artifact_bytes(digest)
            .map_err(|_| "artifact unavailable".to_owned())?;
        if hex_digest(&artifact) != digest {
            return Err("artifact digest mismatch".into());
        }
        let artifact_record = self
            .workspace
            .artifact(digest)
            .map_err(|_| "artifact metadata unavailable".to_owned())?;
        let command = self.command.clone().ok_or("preview is disabled")?;
        let mut inner = self.state.lock().map_err(|_| "preview state unavailable")?;
        self.refresh_locked(&mut inner);
        if matches!(inner.status.state, PreviewLifecycle::Running) {
            return Err("preview is already running".into());
        }
        let mut child = Command::new(command);
        child
            .arg("preview")
            .arg(self.workspace.artifact_path(digest))
            .arg("--max-seconds")
            .arg(max_seconds.to_string())
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in ["TMP", "TEMP", "TMPDIR"] {
            if let Some(value) = std::env::var_os(key) {
                child.env(key, value);
            }
        }
        let mut child = child
            .spawn()
            .map_err(|_| "preview failed to start".to_owned())?;
        let output = Arc::clone(&self.state);
        if let Some(mut stdout) = child.stdout.take() {
            let output = Arc::clone(&output);
            thread::spawn(move || read_output(&mut stdout, &output, false));
        }
        if let Some(mut stderr) = child.stderr.take() {
            let output = Arc::clone(&output);
            thread::spawn(move || read_output(&mut stderr, &output, true));
        }
        inner.running = Some(RunningPreview {
            child,
            artifact: digest.into(),
            payload: artifact_record.payload_digest.clone(),
            revision: artifact_record.draft_revision,
        });
        inner.status = PreviewStatus {
            state: PreviewLifecycle::Running,
            artifact_digest: Some(digest.into()),
            draft_revision: Some(artifact_record.draft_revision),
            snapshot_sha256: Some(artifact_record.payload_digest),
            started_at: Some(now()),
            completed_at: None,
            output: String::new(),
            output_sequence: 0,
            truncated: false,
            exit_code: None,
            stale: false,
        };
        Ok(inner.status.clone())
    }
    pub fn input(&self, text: &str) -> Result<(), String> {
        if text.is_empty()
            || text.len() > 1024
            || text.contains(['\r', '\n'])
            || text.chars().any(char::is_control)
        {
            return Err("input is invalid".into());
        }
        let mut inner = self.state.lock().map_err(|_| "preview unavailable")?;
        let run = inner.running.as_mut().ok_or("preview is not running")?;
        let stdin = run
            .child
            .stdin
            .as_mut()
            .ok_or("preview input unavailable")?;
        stdin
            .write_all(text.as_bytes())
            .map_err(|_| "preview input failed")?;
        stdin.write_all(b"\n").map_err(|_| "preview input failed")?;
        stdin.flush().map_err(|_| "preview input failed")?;
        Ok(())
    }
    pub fn stop(&self) -> Result<PreviewStatus, String> {
        let mut inner = self.state.lock().map_err(|_| "preview unavailable")?;
        if let Some(run) = inner.running.as_mut() {
            if let Some(stdin) = run.child.stdin.as_mut() {
                let _ = stdin.write_all(b"quit\n");
                let _ = stdin.flush();
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                match run.child.try_wait() {
                    Ok(Some(status)) if status.success() => {
                        let completed = now();
                        let persisted = self.persist_receipt(
                            &run.artifact,
                            &run.payload,
                            run.revision,
                            completed,
                        );
                        inner.status.state = if persisted {
                            PreviewLifecycle::Succeeded
                        } else {
                            PreviewLifecycle::Failed
                        };
                        inner.status.exit_code = status.code();
                        inner.status.completed_at = Some(completed);
                        inner.running = None;
                        break;
                    }
                    Ok(Some(status)) => {
                        inner.status.state = PreviewLifecycle::Failed;
                        inner.status.exit_code = status.code();
                        inner.status.completed_at = Some(now());
                        inner.running = None;
                        break;
                    }
                    _ if std::time::Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    _ => {
                        let _ = run.child.kill();
                        let _ = run.child.wait();
                        inner.status.state = PreviewLifecycle::Stopped;
                        inner.status.completed_at = Some(now());
                        inner.running = None;
                        break;
                    }
                }
            }
        }
        Ok(inner.status.clone())
    }
    fn refresh_locked(&self, inner: &mut Inner) {
        let Some(run) = inner.running.as_mut() else {
            return;
        };
        if let Ok(Some(status)) = run.child.try_wait() {
            inner.status.exit_code = status.code();
            inner.status.completed_at = Some(now());
            inner.status.state = if status.success()
                && self
                    .workspace
                    .write_preview_receipt(
                        &run.artifact,
                        &PreviewReceipt {
                            schema_version: 1,
                            artifact_digest: run.artifact.clone(),
                            payload_digest: run.payload.clone(),
                            draft_revision: run.revision,
                            completed_at: inner.status.completed_at.unwrap_or_else(now),
                            result: "succeeded".into(),
                        },
                    )
                    .is_ok()
            {
                PreviewLifecycle::Succeeded
            } else {
                PreviewLifecycle::Failed
            };
            inner.running = None;
        }
    }

    fn persist_receipt(
        &self,
        artifact: &str,
        payload: &str,
        revision: u64,
        completed_at: u64,
    ) -> bool {
        self.workspace
            .write_preview_receipt(
                artifact,
                &PreviewReceipt {
                    schema_version: 1,
                    artifact_digest: artifact.to_owned(),
                    payload_digest: payload.to_owned(),
                    draft_revision: revision,
                    completed_at,
                    result: "succeeded".into(),
                },
            )
            .is_ok()
    }
}
impl Drop for PreviewManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.state) != 1 {
            return;
        }
        if let Ok(mut inner) = self.state.lock() {
            if let Some(run) = inner.running.as_mut() {
                let _ = run.child.kill();
                let _ = run.child.wait();
            }
            inner.running = None;
        }
    }
}

pub fn resolve_command(explicit: Option<&PathBuf>) -> Result<Option<PathBuf>, String> {
    if let Some(path) = explicit {
        let metadata = path
            .symlink_metadata()
            .map_err(|_| "preview command is unavailable".to_owned())?;
        if metadata.is_file() && !metadata.file_type().is_symlink() {
            return path
                .canonicalize()
                .map(Some)
                .map_err(|_| "preview command is unavailable".to_owned());
        }
        return Err("preview command must be a regular non-symlink file".into());
    }
    let current =
        std::env::current_exe().map_err(|_| "Studio executable path is unavailable".to_owned())?;
    let name = if cfg!(windows) {
        "worldwright.exe"
    } else {
        "worldwright"
    };
    let Some(parent) = current.parent() else {
        return Ok(None);
    };
    let sibling = parent.join(name);
    let Ok(metadata) = sibling.symlink_metadata() else {
        return Ok(None);
    };
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        Ok(Some(sibling))
    } else {
        Ok(None)
    }
}
fn read_output(reader: &mut impl Read, state: &Arc<Mutex<Inner>>, stderr: bool) {
    let mut buffer = [0u8; 4096];
    loop {
        let Ok(n) = reader.read(&mut buffer) else {
            break;
        };
        if n == 0 {
            break;
        }
        let Ok(mut inner) = state.lock() else { break };
        inner.status.output_sequence += 1;
        let prefix = if stderr { "[stderr] " } else { "" };
        let text = String::from_utf8_lossy(&buffer[..n]);
        let value = format!("{prefix}{text}");
        let mut truncated = inner.status.truncated;
        append_bounded(&mut inner.status.output, &value, &mut truncated);
        inner.status.truncated = truncated;
    }
}
fn append_bounded(output: &mut String, value: &str, truncated: &mut bool) {
    const MAX: usize = 64 * 1024;
    if output.len() >= MAX {
        *truncated = true;
        return;
    }
    let remaining = MAX - output.len();
    if value.len() <= remaining {
        output.push_str(value);
        return;
    }
    let mut end = remaining;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&value[..end]);
    *truncated = true;
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(1)
}
fn hex_digest(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let mut output = String::with_capacity(64);
    for byte in h.finalize() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("String formatting cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_output_preserves_utf8_boundaries() {
        let mut output = "x".repeat(64 * 1024 - 1);
        let mut truncated = false;
        append_bounded(&mut output, "é", &mut truncated);
        assert!(truncated);
        assert_eq!(output.len(), 64 * 1024 - 1);
        assert!(std::str::from_utf8(output.as_bytes()).is_ok());
    }
}
