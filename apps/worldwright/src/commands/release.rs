use super::play;
use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};
use worldwright_release::{
    PackageOptions, ReleaseError, ReleaseId, Sha256Digest, VerifiedArtifact, WriteOutcome,
    load_file, package_file, write_new,
};

pub const MAX_CONCURRENT_PREVIEWS: usize = 4;
const MAX_PREVIEW_SECONDS: u64 = 3600;
const PREVIEW_GRACE_SECONDS: u64 = 65;

#[derive(Debug)]
pub(crate) struct PreviewSlot {
    path: PathBuf,
    token: String,
}

impl PreviewSlot {
    fn root() -> PathBuf {
        std::env::temp_dir().join("worldwright-preview-v1")
    }

    fn acquire() -> Result<Self, &'static str> {
        Self::acquire_in(&Self::root())
    }

    fn acquire_in(root: &Path) -> Result<Self, &'static str> {
        Self::acquire_in_with(root, reclaimable)
    }

    fn acquire_in_with(
        root: &Path,
        is_reclaimable: impl Fn(&Path) -> bool,
    ) -> Result<Self, &'static str> {
        ensure_real_directory(root)?;
        for index in 0..MAX_CONCURRENT_PREVIEWS {
            let path = root.join(format!("slot-{index}.lock"));
            if let Some(slot) = create_slot(&path)? {
                return Ok(slot);
            }
            if is_reclaimable(&path) {
                let _ = fs::remove_file(&path);
                if let Some(slot) = create_slot(&path)? {
                    return Ok(slot);
                }
            }
        }
        Err("preview: preview capacity is full")
    }

    fn validate(path: &Path) -> Result<(), &'static str> {
        if !path.is_absolute() || path.parent() != Some(Self::root().as_path()) {
            return Err("preview worker: invalid preview slot");
        }
        match path.file_name().and_then(|value| value.to_str()) {
            Some("slot-0.lock" | "slot-1.lock" | "slot-2.lock" | "slot-3.lock") => Ok(()),
            _ => Err("preview worker: invalid preview slot"),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn token(&self) -> &str {
        &self.token
    }
}

impl Drop for PreviewSlot {
    fn drop(&mut self) {
        cleanup_owned(&self.path, &self.token);
    }
}

fn create_slot(path: &Path) -> Result<Option<PreviewSlot>, &'static str> {
    let token = random_token()?;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            if file.write_all(token.as_bytes()).is_err() || file.sync_all().is_err() {
                drop(file);
                let _ = fs::remove_file(path);
                return Ok(None);
            }
            Ok(Some(PreviewSlot {
                path: path.to_owned(),
                token,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(_) => Ok(None),
    }
}

fn random_token() -> Result<String, &'static str> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        if getrandom::fill(&mut bytes).is_err() {
            continue;
        }
        if bytes.iter().any(|byte| *byte != 0) {
            let mut token = String::with_capacity(32);
            for byte in bytes {
                write!(token, "{byte:02x}").expect("writing to a string cannot fail");
            }
            return Ok(token);
        }
    }
    Err("preview: preview capacity is unavailable")
}

fn cleanup_owned(path: &Path, token: &str) {
    if !slot_owned(path, token) {
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
        let _ = fs::remove_file(path);
    }
}

fn slot_owned(path: &Path, token: &str) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut bytes = Vec::new();
    if file.take(33).read_to_end(&mut bytes).is_err() || bytes != token.as_bytes() {
        return false;
    }
    true
}

fn valid_token(token: &str) -> bool {
    token.len() == 32
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn ensure_real_directory(root: &Path) -> Result<(), &'static str> {
    if !root.exists() {
        fs::create_dir_all(root).map_err(|_| "preview: preview capacity is unavailable")?;
    }
    let metadata =
        fs::symlink_metadata(root).map_err(|_| "preview: preview capacity is unavailable")?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err("preview: preview capacity is unavailable");
    }
    Ok(())
}

fn reclaimable(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > Duration::from_secs(MAX_PREVIEW_SECONDS + PREVIEW_GRACE_SECONDS))
}

pub fn package(
    content: &Path,
    output: &Path,
    release_id: ReleaseId,
    source_revision: String,
    builder_identity: String,
) -> ExitCode {
    let artifact = match package_file(
        content,
        PackageOptions {
            release_id,
            source_revision,
            builder_identity,
        },
    ) {
        Ok(artifact) => artifact,
        Err(error) => return report_error("package", error),
    };
    let outcome = match write_new(output, &artifact) {
        Ok(outcome) => outcome,
        Err(error) => return report_error("package", error),
    };
    let verified = match load_file(output, None) {
        Ok(verified) => verified,
        Err(error) => return report_error("package", error),
    };
    println!("Release ID: {}", verified.manifest().release_id());
    println!("Artifact digest: {}", verified.artifact_digest());
    println!("Payload digest: {}", verified.manifest().payload_sha256());
    println!(
        "Artifact: {}",
        match outcome {
            WriteOutcome::Created => "Created",
            WriteOutcome::Reused => "Reused",
        }
    );
    ExitCode::SUCCESS
}

pub fn inspect(path: &Path, json: bool) -> ExitCode {
    let verified = match load_file(path, None) {
        Ok(verified) => verified,
        Err(error) => return report_error("inspect", error),
    };
    if json {
        let manifest = verified.manifest();
        let content = verified.content();
        let value = serde_json::json!({
            "artifact_digest": verified.artifact_digest().to_string(),
            "manifest": manifest,
            "content": {
                "namespace": content.namespace(),
                "rooms": content.rooms().len(),
                "entry_room": content.entry_room().to_string(),
            },
        });
        match serde_json::to_string(&value) {
            Ok(value) => println!("{value}"),
            Err(_) => return report_message("inspect: unable to serialize inspection"),
        }
    } else {
        print_text_inspection(&verified);
    }
    ExitCode::SUCCESS
}

fn print_text_inspection(verified: &VerifiedArtifact) {
    let manifest = verified.manifest();
    let content = verified.content();
    println!("Manifest version: {}", manifest.manifest_version());
    println!("Release ID: {}", manifest.release_id());
    println!(
        "Content schema version: {}",
        manifest.content_schema_version()
    );
    println!("Game ID: {}", manifest.game_id());
    println!("Contract version: {}", manifest.contract_version());
    println!("Payload media type: {}", manifest.media_type());
    println!("Payload length: {}", manifest.payload_byte_length());
    println!("Payload digest: {}", manifest.payload_sha256());
    println!("Source revision: {}", manifest.source_revision());
    println!("Builder identity: {}", manifest.builder_identity());
    println!("Artifact digest: {}", verified.artifact_digest());
    println!("Namespace: {}", content.namespace());
    println!("Rooms: {}", content.rooms().len());
    println!("Entry room: {}", content.entry_room());
}

pub fn preview(path: &Path, max_seconds: u64) -> ExitCode {
    if !(1..=MAX_PREVIEW_SECONDS).contains(&max_seconds) {
        return report_message("preview: max-seconds must be 1..=3600");
    }
    let verified = match load_file(path, None) {
        Ok(verified) => verified,
        Err(error) => return report_error("preview", error),
    };
    let digest = verified.artifact_digest();
    let artifact = match std::fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) => {
            return report_message(&format!(
                "preview: unable to canonicalize artifact: {error}"
            ));
        }
    };
    let slot = match PreviewSlot::acquire() {
        Ok(slot) => slot,
        Err(message) => return report_message(message),
    };
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            return report_message(&format!("preview: unable to locate executable: {error}"));
        }
    };
    let mut child = match Command::new(executable)
        .arg("__preview-worker")
        .arg(&artifact)
        .arg(digest.to_string())
        .arg(slot.path())
        .arg(slot.token())
        .arg(max_seconds.to_string())
        .env_clear()
        .env("TMP", std::env::temp_dir())
        .env("TEMP", std::env::temp_dir())
        .env("TMPDIR", std::env::temp_dir())
        .env("WORLDWRIGHT_PREVIEW", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return report_message(&format!(
                "preview: unable to start isolated worker: {error}"
            ));
        }
    };
    let deadline = Instant::now() + Duration::from_secs(max_seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let result = if status.success() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                };
                drop(slot);
                return result;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                drop(slot);
                return report_message(&format!("preview: worker status failed: {error}"));
            }
        }
        let now = Instant::now();
        if now >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            drop(slot);
            return report_message("preview: isolated worker exceeded the maximum lifetime");
        }
        thread::sleep((deadline - now).min(Duration::from_millis(50)));
    }
}

pub fn preview_worker(
    path: &Path,
    expected_digest: Sha256Digest,
    slot_path: &Path,
    token: &str,
    max_seconds: u64,
) -> ExitCode {
    if !(1..=MAX_PREVIEW_SECONDS).contains(&max_seconds) {
        return report_message("preview worker: max-seconds must be 1..=3600");
    }
    if let Err(message) = PreviewSlot::validate(slot_path) {
        return report_message(message);
    }
    if !valid_token(token) || !slot_owned(slot_path, token) {
        return report_message("preview worker: invalid preview slot");
    }
    if std::env::var("WORLDWRIGHT_PREVIEW").ok().as_deref() != Some("1") {
        return report_message("preview worker: direct invocation is not allowed");
    }
    if std::env::var_os("WORLDWRIGHT_PREVIEW_FORBIDDEN_SENTINEL").is_some() {
        return report_message("preview worker: forbidden environment present");
    }
    let watchdog_slot = slot_path.to_owned();
    let watchdog_token = token.to_owned();
    let watchdog_seconds = max_seconds
        .checked_add(5)
        .expect("preview max lifetime is bounded");
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(watchdog_seconds));
        cleanup_owned(&watchdog_slot, &watchdog_token);
        std::process::exit(1);
    });
    let verified = match load_file(path, Some(expected_digest)) {
        Ok(verified) => verified,
        Err(error) => return report_error("preview worker", error),
    };
    play::preview(verified.content(), verified.artifact_digest())
}

fn report_error(operation: &str, error: ReleaseError) -> ExitCode {
    if matches!(error, ReleaseError::ImmutableConflict) {
        eprintln!("{operation}: immutable artifact conflict");
        ExitCode::from(3)
    } else {
        eprintln!("{operation}: {error}");
        ExitCode::from(1)
    }
}

fn report_message(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_slot_capacity_is_bounded_and_reusable() {
        let root = std::env::temp_dir().join(format!(
            "worldwright-preview-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut slots = (0..MAX_CONCURRENT_PREVIEWS)
            .map(|_| PreviewSlot::acquire_in(&root).unwrap())
            .collect::<Vec<_>>();
        assert!(PreviewSlot::acquire_in(&root).is_err());
        slots.clear();
        let slot = PreviewSlot::acquire_in(&root).unwrap();
        drop(slot);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preview_slot_validator_rejects_sibling_and_unknown_names() {
        let root = PreviewSlot::root();
        assert!(PreviewSlot::validate(&root.join("slot-0.lock")).is_ok());
        assert!(
            PreviewSlot::validate(
                &root
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("worldwright-preview-v1")
                    .join("slot-0.lock")
            )
            .is_err()
        );
        assert!(PreviewSlot::validate(&root.join("slot-9.lock")).is_err());
        assert!(PreviewSlot::validate(&root.join("slot-0.lock.link")).is_err());
    }

    #[test]
    fn stale_reclaim_retries_same_slot_without_waiting() {
        let root = std::env::temp_dir().join(format!(
            "worldwright-preview-stale-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let stale = root.join("slot-0.lock");
        fs::write(&stale, "stale").unwrap();
        let slot = PreviewSlot::acquire_in_with(&root, |_| true).unwrap();
        assert_eq!(slot.path(), stale.as_path());
        drop(slot);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn old_owner_cannot_remove_replaced_slot() {
        let root = std::env::temp_dir().join(format!(
            "worldwright-preview-owner-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let slot = PreviewSlot::acquire_in(&root).unwrap();
        let path = slot.path().to_owned();
        fs::remove_file(&path).unwrap();
        fs::write(&path, "00000000000000000000000000000000").unwrap();
        drop(slot);
        assert!(path.exists());
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
