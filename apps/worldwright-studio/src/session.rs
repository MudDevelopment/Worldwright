//! Cooperative, fail-closed exclusion for the supported Studio serve path.
//! A crash leaves a sentinel: an operator must establish that all writers have
//! stopped before archiving/removing it. PID reuse is never treated as proof.
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::PathBuf};
use crate::workspace::Workspace;

pub(crate) struct StudioSession {
    path: PathBuf,
    file: Option<File>,
    identity: Vec<u8>,
    release_on_drop: bool,
}

impl StudioSession {
    pub(crate) fn acquire(workspace: &Workspace) -> Result<Self, String> {
        let path = workspace.session_path();
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| "Studio session randomness unavailable")?;
        let identity = format!("worldwright-studio-session-v1\npid={}\nnonce={:032x}\n", std::process::id(), u128::from_be_bytes(nonce)).into_bytes();
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let mut file = options.open(&path).map_err(|_| "Studio workspace is in use or its session sentinel needs operator recovery; see docs/R5_ACCEPTANCE.md")?;
        // On incomplete creation leave the sentinel and fail closed.
        file.write_all(&identity).and_then(|()| file.sync_all())
            .map_err(|_| "Studio session sentinel write failed; preserve workspace for recovery")?;
        #[cfg(unix)]
        File::open(path.parent().ok_or("Studio session parent unavailable")?)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| "Studio session directory sync failed; preserve workspace for recovery")?;
        Ok(Self { path, file: Some(file), identity, release_on_drop: true })
    }

    pub(crate) fn preserve(&mut self) {
        self.release_on_drop = false;
    }
}

impl Drop for StudioSession {
    fn drop(&mut self) {
        if !self.release_on_drop || std::thread::panicking() { return; }
        // Windows keeps a nonshared handle until shutdown. Unix additionally
        // checks inode identity. This is not a lock against hostile same-user code.
        let owned = self.file.as_ref().and_then(|file| file.metadata().ok());
        // Close the nonshared Windows handle before querying the path again.
        drop(self.file.take());
        let named = self.path.symlink_metadata().ok();
        let same = match (owned, named) {
            (Some(owned), Some(named)) if named.is_file() && !named.file_type().is_symlink() => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    owned.dev() == named.dev() && owned.ino() == named.ino() && named.nlink() == 1
                }
                #[cfg(not(unix))]
                { owned.len() == named.len() }
            }
            _ => false,
        };
        let mut bytes = Vec::new();
        let intact = same && File::open(&self.path)
            .and_then(|file| file.take(256).read_to_end(&mut bytes)).is_ok()
            && bytes == self.identity;
        if intact && fs::remove_file(&self.path).is_ok() {
            #[cfg(unix)]
            if let Some(parent) = self.path.parent() {
                let _ = File::open(parent).and_then(|directory| directory.sync_all());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workspace() -> (Workspace, PathBuf) {
        let root = std::env::temp_dir().join(format!("worldwright-session-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        (Workspace::open_or_create(&root, None).unwrap(), root)
    }
    #[test]
    fn r5_session_excludes_second_open_and_releases_on_drop() {
        let (workspace, root) = workspace();
        let first = StudioSession::acquire(&workspace).unwrap();
        let reopened = Workspace::open_or_create(&root, None).unwrap();
        assert!(StudioSession::acquire(&reopened).is_err());
        drop(first);
        assert!(!workspace.session_path().exists());
        drop(StudioSession::acquire(&reopened).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    // Invoked as an exact child test by the crash regression, not evidence by itself.
    #[test]
    fn session_process_helper() {
        let Some(root) = std::env::var_os("WORLDWRIGHT_R5_SESSION_TEST_ROOT") else { return; };
        let root = PathBuf::from(root);
        let workspace = Workspace::open_or_create(&root, None).unwrap();
        let session = StudioSession::acquire(&workspace).unwrap();
        fs::write(root.join("child-ready"), b"ready").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(15));
        drop(session);
    }

    #[test]
    fn r5_session_cross_process_kill_preserves_exclusion_until_operator_recovery() {
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
        }
        let (workspace, root) = workspace();
        let mut child = Child(std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "session::tests::session_process_helper", "--nocapture"])
            .env("WORLDWRIGHT_R5_SESSION_TEST_ROOT", &root)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())
            .spawn().unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !root.join("child-ready").exists() {
            assert!(child.0.try_wait().unwrap().is_none(), "session child exited before acquiring");
            assert!(std::time::Instant::now() < deadline, "session child did not become ready");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(StudioSession::acquire(&workspace).is_err());
        child.0.kill().unwrap(); child.0.wait().unwrap();
        assert!(workspace.session_path().exists());
        assert!(StudioSession::acquire(&workspace).is_err());
        // This isolated test has reaped its exact child, establishing the manual
        // recovery precondition. Production never infers it from a PID or age.
        fs::remove_file(workspace.session_path()).unwrap();
        drop(StudioSession::acquire(&workspace).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn r5_session_preserves_sentinel_when_cleanup_is_uncertain() {
        let (workspace, root) = workspace();
        let mut session = StudioSession::acquire(&workspace).unwrap();
        session.preserve(); drop(session);
        assert!(StudioSession::acquire(&workspace).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn r5_session_never_reclaims_crash_or_malformed_sentinel() {
        let (workspace, root) = workspace();
        for bytes in [b"".as_slice(), b"dead-pid-is-not-authority".as_slice()] {
            fs::write(workspace.session_path(), bytes).unwrap();
            assert!(StudioSession::acquire(&workspace).is_err());
            assert_eq!(fs::read(workspace.session_path()).unwrap(), bytes);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
