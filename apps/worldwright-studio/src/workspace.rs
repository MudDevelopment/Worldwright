use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use worldwright_content::{
    ContentLimits, ContentReport, DraftRoomV1, DraftWorldV1, load_bytes, load_draft_bytes,
    serialize_draft,
};
use worldwright_core::ids::PublicationId;
use worldwright_operations::PublicationView;

use crate::publication::{valid_status_transition, validate_publication_view};

pub const WORKSPACE_MARKER: &[u8] = b"worldwright-studio-v1\n";
pub const MAX_REVISIONS: u64 = 10_000;

#[derive(Clone)]
pub struct Workspace {
    root: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DraftSnapshot {
    pub revision: u64,
    pub snapshot_sha256: String,
    pub draft: DraftWorldV1,
}

#[derive(Debug)]
pub enum WorkspaceError {
    InvalidWorkspace(&'static str),
    InvalidDraft(ContentReport),
    Conflict { current_revision: u64 },
    RevisionNotFound,
    Capacity,
    Io(std::io::Error),
}
impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidWorkspace(message) => message,
            Self::InvalidDraft(_) => "draft is invalid",
            Self::Conflict { .. } => "draft revision conflicts with current workspace",
            Self::RevisionNotFound => "revision was not found",
            Self::Capacity => "workspace revision capacity is exhausted",
            Self::Io(_) => "workspace I/O failed",
        })
    }
}
impl std::error::Error for WorkspaceError {}
impl From<std::io::Error> for WorkspaceError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationView {
    pub valid: bool,
    pub snapshot_sha256: String,
    pub diagnostics: Vec<DiagnosticView>,
    pub truncated: bool,
    pub namespace: Option<String>,
    pub room_count: Option<usize>,
    pub exit_count: Option<usize>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticView {
    pub severity: worldwright_content::Severity,
    pub code: &'static str,
    pub location: String,
    pub message: String,
}

impl Workspace {
    pub(crate) fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }
    pub(crate) fn artifact_path(&self, digest: &str) -> PathBuf {
        self.artifacts_dir().join(format!("{digest}.wwr"))
    }
    pub(crate) fn artifact_sidecar_path(&self, digest: &str) -> PathBuf {
        self.artifacts_dir().join(format!("{digest}.json"))
    }
    pub(crate) fn preview_receipt_path(&self, digest: &str) -> PathBuf {
        self.root
            .join("preview-receipts")
            .join(format!("{digest}.json"))
    }
    pub(crate) fn publication_records_dir(&self) -> PathBuf {
        self.root.join("publication-records")
    }
    pub(crate) fn insert_publication_record(
        &self,
        view: &PublicationView,
    ) -> Result<(), WorkspaceError> {
        validate_publication_view(view)
            .map_err(|_| WorkspaceError::InvalidWorkspace("publication record invalid"))?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| WorkspaceError::InvalidWorkspace("workspace lock is poisoned"))?;
        if self.publication_records_unlocked()?.len() >= 100 {
            return Err(WorkspaceError::Capacity);
        }
        let path = self.publication_record_path(&view.request_id)?;
        let bytes = serde_json::to_vec(view).map_err(|_| {
            WorkspaceError::InvalidWorkspace("publication record serialization failed")
        })?;
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
                    let _ = fs::remove_file(&path);
                    return Err(WorkspaceError::Io(error));
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = self.read_publication_record_path(&path)?;
                if existing == *view {
                    Ok(())
                } else {
                    Err(WorkspaceError::Conflict {
                        current_revision: 0,
                    })
                }
            }
            Err(error) => Err(WorkspaceError::Io(error)),
        }
    }

    pub(crate) fn update_publication_record(
        &self,
        next: &PublicationView,
    ) -> Result<(), WorkspaceError> {
        validate_publication_view(next)
            .map_err(|_| WorkspaceError::InvalidWorkspace("publication record invalid"))?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| WorkspaceError::InvalidWorkspace("workspace lock is poisoned"))?;
        let path = self.publication_record_path(&next.request_id)?;
        let previous = self.read_publication_record_path(&path)?;
        if !same_publication_identity(&previous, next)
            || !valid_status_transition(previous.status, next.status)
        {
            return Err(WorkspaceError::Conflict {
                current_revision: 0,
            });
        }
        if previous == *next {
            return Ok(());
        }

        let next_path = path.with_extension("json.next");
        let previous_path = path.with_extension("json.previous");
        if next_path.exists() || previous_path.exists() {
            return Err(WorkspaceError::InvalidWorkspace(
                "publication update recovery is required",
            ));
        }
        let bytes = serde_json::to_vec(next).map_err(|_| {
            WorkspaceError::InvalidWorkspace("publication record serialization failed")
        })?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&next_path)?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&next_path);
            return Err(WorkspaceError::Io(error));
        }
        fs::rename(&path, &previous_path)?;
        if let Err(error) = fs::rename(&next_path, &path) {
            let _ = fs::rename(&previous_path, &path);
            let _ = fs::remove_file(&next_path);
            return Err(WorkspaceError::Io(error));
        }
        fs::remove_file(previous_path)?;
        Ok(())
    }

    pub(crate) fn publication_record(
        &self,
        request: &str,
    ) -> Result<PublicationView, WorkspaceError> {
        let path = self.publication_record_path(request)?;
        self.read_publication_record_path(&path)
    }

    pub(crate) fn publication_records(&self) -> Result<Vec<PublicationView>, WorkspaceError> {
        let mut records = self.publication_records_unlocked()?;
        records.sort_by(|left, right| left.request_id.cmp(&right.request_id));
        Ok(records)
    }

    fn publication_records_unlocked(&self) -> Result<Vec<PublicationView>, WorkspaceError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(self.publication_records_dir())? {
            if records.len() >= 100 {
                return Err(WorkspaceError::Capacity);
            }
            let entry = entry?;
            let path = entry.path();
            let metadata = path.symlink_metadata()?;
            if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65_536 {
                return Err(WorkspaceError::InvalidWorkspace(
                    "publication record invalid",
                ));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| WorkspaceError::InvalidWorkspace("publication record unexpected"))?;
            let request = name
                .strip_suffix(".json")
                .ok_or(WorkspaceError::InvalidWorkspace(
                    "publication record unexpected",
                ))?;
            let canonical: PublicationId = request
                .parse()
                .map_err(|_| WorkspaceError::InvalidWorkspace("publication record unexpected"))?;
            if canonical.to_string() != request {
                return Err(WorkspaceError::InvalidWorkspace(
                    "publication record unexpected",
                ));
            }
            let view = self.read_publication_record_path(&path)?;
            if view.request_id != request {
                return Err(WorkspaceError::InvalidWorkspace(
                    "publication record identity mismatch",
                ));
            }
            records.push(view);
        }
        Ok(records)
    }

    fn publication_record_path(&self, request: &str) -> Result<PathBuf, WorkspaceError> {
        let canonical: PublicationId = request
            .parse()
            .map_err(|_| WorkspaceError::InvalidWorkspace("publication request ID invalid"))?;
        if canonical.to_string() != request {
            return Err(WorkspaceError::InvalidWorkspace(
                "publication request ID invalid",
            ));
        }
        Ok(self
            .publication_records_dir()
            .join(format!("{request}.json")))
    }

    fn read_publication_record_path(&self, path: &Path) -> Result<PublicationView, WorkspaceError> {
        let metadata = path.symlink_metadata()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65_536 {
            return Err(WorkspaceError::InvalidWorkspace(
                "publication record invalid",
            ));
        }
        let bytes = fs::read(path)?;
        let view: PublicationView = serde_json::from_slice(&bytes)
            .map_err(|_| WorkspaceError::InvalidWorkspace("publication record invalid"))?;
        validate_publication_view(&view)
            .map_err(|_| WorkspaceError::InvalidWorkspace("publication record invalid"))?;
        Ok(view)
    }
    pub fn open_or_create(root: &Path, initial: Option<&Path>) -> Result<Self, WorkspaceError> {
        let existing_empty = if root.exists() {
            let metadata = root.symlink_metadata()?;
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && fs::read_dir(root)?.next().is_none()
        } else {
            false
        };
        if root.exists() && !existing_empty && initial.is_some() {
            return Err(WorkspaceError::InvalidWorkspace(
                "initial content is only valid for new workspaces",
            ));
        }
        let initial_bytes = if let Some(path) = initial {
            let bytes = read_regular_input(path)?;
            let draft = load_draft_bytes(&path.to_string_lossy(), &bytes, ContentLimits::default())
                .map_err(WorkspaceError::InvalidDraft)?;
            Some(
                serialize_draft("initial", &draft, ContentLimits::default())
                    .map_err(WorkspaceError::InvalidDraft)?,
            )
        } else {
            None
        };
        if root.exists() && !existing_empty {
            if !root.is_dir()
                || root
                    .symlink_metadata()
                    .map_err(WorkspaceError::Io)?
                    .file_type()
                    .is_symlink()
            {
                return Err(WorkspaceError::InvalidWorkspace(
                    "root must be a real directory",
                ));
            }
            let root = root.canonicalize()?;
            let marker = root.join(".worldwright-studio");
            let marker_metadata = marker
                .symlink_metadata()
                .map_err(|_| WorkspaceError::InvalidWorkspace("workspace marker is invalid"))?;
            if !marker_metadata.is_file() || marker_metadata.file_type().is_symlink() {
                return Err(WorkspaceError::InvalidWorkspace(
                    "workspace marker is invalid",
                ));
            }
            if fs::read(&marker)
                .map_err(|_| WorkspaceError::InvalidWorkspace("workspace marker is invalid"))?
                != WORKSPACE_MARKER
            {
                return Err(WorkspaceError::InvalidWorkspace(
                    "workspace marker is invalid",
                ));
            }
            for name in [
                "revisions",
                "artifacts",
                "preview-receipts",
                "publication-records",
            ] {
                let p = root.join(name);
                if !p.is_dir() || p.symlink_metadata()?.file_type().is_symlink() {
                    return Err(WorkspaceError::InvalidWorkspace(
                        "workspace directory is invalid",
                    ));
                }
            }
            let workspace = Self {
                root,
                lock: Arc::new(Mutex::new(())),
            };
            workspace.enumerate()?;
            workspace.latest()?;
            return Ok(workspace);
        }
        let parent = root
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.is_dir() || parent.symlink_metadata()?.file_type().is_symlink() {
            return Err(WorkspaceError::InvalidWorkspace(
                "workspace parent must exist",
            ));
        }
        let bytes = if let Some(bytes) = initial_bytes {
            bytes
        } else {
            let draft = DraftWorldV1 {
                schema_version: 1,
                namespace: "draft".into(),
                entry_room: "draft:rooms.start".into(),
                rooms: vec![DraftRoomV1 {
                    id: "draft:rooms.start".into(),
                    title: "First Room".into(),
                    description: "An unfinished place.".into(),
                    exits: BTreeMap::new(),
                    allow_unreachable: false,
                }],
            };
            serialize_draft("initial", &draft, ContentLimits::default())
                .map_err(WorkspaceError::InvalidDraft)?
        };
        initialize_files(root, &bytes)?;
        let workspace = Self {
            root: root.canonicalize()?,
            lock: Arc::new(Mutex::new(())),
        };
        Ok(workspace)
    }
    pub fn latest(&self) -> Result<DraftSnapshot, WorkspaceError> {
        let rev = self
            .enumerate()?
            .last()
            .copied()
            .ok_or(WorkspaceError::RevisionNotFound)?;
        self.revision(rev)
    }
    pub fn revision(&self, revision: u64) -> Result<DraftSnapshot, WorkspaceError> {
        let (bytes, draft) = self.verified_revision_bytes(revision)?;
        Ok(DraftSnapshot {
            revision,
            snapshot_sha256: hash(&bytes),
            draft,
        })
    }
    pub fn save(&self, base: u64, raw: &[u8]) -> Result<DraftSnapshot, WorkspaceError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| WorkspaceError::InvalidWorkspace("workspace lock is poisoned"))?;
        let latest = self.latest()?.revision;
        if latest != base {
            return Err(WorkspaceError::Conflict {
                current_revision: latest,
            });
        }
        let draft = load_draft_bytes("save", raw, ContentLimits::default())
            .map_err(WorkspaceError::InvalidDraft)?;
        let bytes = serialize_draft("save", &draft, ContentLimits::default())
            .map_err(WorkspaceError::InvalidDraft)?;
        let next = latest.checked_add(1).ok_or(WorkspaceError::Capacity)?;
        if next > MAX_REVISIONS {
            return Err(WorkspaceError::Capacity);
        }
        if let Err(error) = self.write_revision(next, &bytes) {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                let current_revision = self.latest()?.revision;
                return Err(WorkspaceError::Conflict { current_revision });
            }
            return Err(WorkspaceError::Io(error));
        }
        self.revision(next)
    }
    pub fn inspect_import(raw: &[u8]) -> Result<(DraftWorldV1, ValidationView), WorkspaceError> {
        let draft = load_draft_bytes("import", raw, ContentLimits::default())
            .map_err(WorkspaceError::InvalidDraft)?;
        let canonical = serialize_draft("import", &draft, ContentLimits::default())
            .map_err(WorkspaceError::InvalidDraft)?;
        let validation = validation_for("import", &canonical);
        Ok((draft, validation))
    }
    pub fn validate_revision(&self, revision: u64) -> Result<ValidationView, WorkspaceError> {
        let (bytes, _) = self.verified_revision_bytes(revision)?;
        Ok(validation_for("workspace", &bytes))
    }
    pub fn export(&self, revision: u64) -> Result<Vec<u8>, WorkspaceError> {
        Ok(self.verified_revision_bytes(revision)?.0)
    }
    fn enumerate(&self) -> Result<Vec<u64>, WorkspaceError> {
        let dir = self.root.join("revisions");
        let mut values = Vec::new();
        for entry in fs::read_dir(dir)? {
            let e = entry?;
            let m = e.metadata()?;
            if !m.is_file() || e.path().symlink_metadata()?.file_type().is_symlink() {
                return Err(WorkspaceError::InvalidWorkspace(
                    "revision entry is invalid",
                ));
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if name.len() != 25
                || !name.ends_with(".json")
                || !name[..20].bytes().all(|b| b.is_ascii_digit())
            {
                return Err(WorkspaceError::InvalidWorkspace(
                    "revision entry is unexpected",
                ));
            }
            let n = name[..20]
                .parse::<u64>()
                .map_err(|_| WorkspaceError::InvalidWorkspace("revision identity is invalid"))?;
            if n == 0 || n > MAX_REVISIONS {
                return Err(WorkspaceError::InvalidWorkspace(
                    "revision identity is invalid",
                ));
            }
            values.push(n)
        }
        values.sort_unstable();
        if values.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WorkspaceError::InvalidWorkspace(
                "revision identity is duplicated",
            ));
        }
        for (i, n) in values.iter().enumerate() {
            if *n != (i as u64 + 1) {
                return Err(WorkspaceError::InvalidWorkspace(
                    "revision sequence has a gap",
                ));
            }
        }
        if values.len() > MAX_REVISIONS as usize {
            return Err(WorkspaceError::Capacity);
        }
        Ok(values)
    }
    fn read_revision(&self, revision: u64) -> Result<Vec<u8>, WorkspaceError> {
        if !(1..=MAX_REVISIONS).contains(&revision) {
            return Err(WorkspaceError::RevisionNotFound);
        }
        let p = self
            .root
            .join("revisions")
            .join(format!("{revision:020}.json"));
        let m = fs::metadata(&p).map_err(|_| WorkspaceError::RevisionNotFound)?;
        if !m.is_file() || p.symlink_metadata()?.file_type().is_symlink() {
            return Err(WorkspaceError::InvalidWorkspace("revision file is invalid"));
        }
        read_regular(&p)
    }
    fn verified_revision_bytes(
        &self,
        revision: u64,
    ) -> Result<(Vec<u8>, DraftWorldV1), WorkspaceError> {
        let bytes = self.read_revision(revision)?;
        let draft = load_draft_bytes("workspace", &bytes, ContentLimits::default())
            .map_err(|_| WorkspaceError::InvalidWorkspace("revision content is invalid"))?;
        let canonical = serialize_draft("workspace", &draft, ContentLimits::default())
            .map_err(|_| WorkspaceError::InvalidWorkspace("revision content is noncanonical"))?;
        if canonical != bytes {
            return Err(WorkspaceError::InvalidWorkspace("revision is noncanonical"));
        }
        Ok((bytes, draft))
    }
    fn write_revision(&self, revision: u64, bytes: &[u8]) -> Result<(), std::io::Error> {
        let p = self
            .root
            .join("revisions")
            .join(format!("{revision:020}.json"));
        let mut f = OpenOptions::new().write(true).create_new(true).open(&p)?;
        if let Err(error) = f.write_all(bytes).and_then(|_| f.sync_all()) {
            let _ = fs::remove_file(&p);
            return Err(error);
        }
        #[cfg(unix)]
        {
            // A directory-sync failure leaves a fully synced revision file; its
            // durable outcome is uncertain and must not be deleted.
            let dir = fs::File::open(self.root.join("revisions"))?;
            dir.sync_all()?;
        }
        Ok(())
    }
}

fn same_publication_identity(previous: &PublicationView, next: &PublicationView) -> bool {
    previous.request_id == next.request_id
        && previous.world_id == next.world_id
        && previous.release_id == next.release_id
        && previous.artifact_digest == next.artifact_digest
        && previous.kind == next.kind
        && previous.target == next.target
        && previous.expected_current == next.expected_current
        && previous.requested_by == next.requested_by
        && previous.review_context == next.review_context
}

fn initialize_files(root: &Path, bytes: &[u8]) -> Result<(), WorkspaceError> {
    let root_preexisting = root.exists();
    if !root_preexisting {
        fs::create_dir(root)?;
    }
    let mut created = Vec::<PathBuf>::new();
    let result = (|| -> Result<(), WorkspaceError> {
        for name in [
            "revisions",
            "artifacts",
            "preview-receipts",
            "publication-records",
        ] {
            let path = root.join(name);
            fs::create_dir(&path)?;
            created.push(path);
        }
        let marker_path = root.join(".worldwright-studio");
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker_path)?;
        marker.write_all(WORKSPACE_MARKER)?;
        marker.sync_all()?;
        created.push(marker_path);
        let workspace = Workspace {
            root: root.canonicalize()?,
            lock: Arc::new(Mutex::new(())),
        };
        workspace.write_revision(1, bytes)?;
        created.push(
            workspace
                .root
                .join("revisions")
                .join("00000000000000000001.json"),
        );
        Ok(())
    })();
    if result.is_err() {
        for path in created.into_iter().rev() {
            let _ = if path.is_dir() {
                fs::remove_dir(path)
            } else {
                fs::remove_file(path)
            };
        }
        if !root_preexisting {
            let _ = fs::remove_dir(root);
        }
    }
    result
}
fn read_regular(path: &Path) -> Result<Vec<u8>, WorkspaceError> {
    let m = fs::metadata(path)?;
    if !m.is_file() || path.symlink_metadata()?.file_type().is_symlink() {
        return Err(WorkspaceError::InvalidWorkspace(
            "expected regular non-symlink file",
        ));
    }
    let f = fs::File::open(path)?;
    let mut b = Vec::new();
    f.take((ContentLimits::default().max_bytes + 1) as u64)
        .read_to_end(&mut b)?;
    if b.is_empty() || b.len() > ContentLimits::default().max_bytes {
        return Err(WorkspaceError::InvalidWorkspace("file exceeds bounds"));
    }
    Ok(b)
}
fn read_regular_input(path: &Path) -> Result<Vec<u8>, WorkspaceError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || path.symlink_metadata()?.file_type().is_symlink() {
        return Err(WorkspaceError::InvalidWorkspace(
            "expected regular non-symlink file",
        ));
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take((ContentLimits::default().max_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > ContentLimits::default().max_bytes {
        return load_draft_bytes(&path.to_string_lossy(), &bytes, ContentLimits::default())
            .map(|_| bytes)
            .map_err(WorkspaceError::InvalidDraft);
    }
    Ok(bytes)
}
fn hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let mut output = String::with_capacity(64);
    for byte in h.finalize() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("String formatting cannot fail");
    }
    output
}
fn validation_for(source: &str, bytes: &[u8]) -> ValidationView {
    match load_bytes(source, bytes, ContentLimits::default()) {
        Ok(content) => ValidationView {
            valid: true,
            snapshot_sha256: hash(bytes),
            diagnostics: Vec::new(),
            truncated: false,
            namespace: Some(content.namespace().into()),
            room_count: Some(content.rooms().len()),
            exit_count: Some(content.exit_count()),
        },
        Err(report) => ValidationView {
            valid: false,
            snapshot_sha256: hash(bytes),
            diagnostics: report
                .diagnostics()
                .iter()
                .map(|d| DiagnosticView {
                    severity: d.severity,
                    code: d.code,
                    location: d.location.clone(),
                    message: d.message.clone(),
                })
                .collect(),
            truncated: report.truncated(),
            namespace: None,
            room_count: None,
            exit_count: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "worldwright-studio-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn initialize_save_conflict_and_reopen() {
        let path = root("workspace");
        let workspace = Workspace::open_or_create(&path, None).unwrap();
        let first = workspace.latest().unwrap();
        assert_eq!(first.revision, 1);
        assert!(workspace.validate_revision(1).unwrap().valid);
        let bytes = serialize_draft("test", &first.draft, ContentLimits::default()).unwrap();
        assert_eq!(workspace.save(1, &bytes).unwrap().revision, 2);
        assert!(matches!(
            workspace.save(1, &bytes),
            Err(WorkspaceError::Conflict {
                current_revision: 2
            })
        ));
        assert_eq!(
            Workspace::open_or_create(&path, None)
                .unwrap()
                .latest()
                .unwrap()
                .revision,
            2
        );
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn invalid_semantics_are_stored_but_reported() {
        let path = root("invalid");
        let workspace = Workspace::open_or_create(&path, None).unwrap();
        let raw = br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[{"id":"draft:rooms.start","title":"","description":"","exits":{}}]}"#;
        let snapshot = workspace.save(1, raw).unwrap();
        let view = workspace.validate_revision(snapshot.revision).unwrap();
        assert!(!view.valid);
        assert!(!view.diagnostics.is_empty());
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn unknown_fields_and_empty_revision_are_rejected() {
        let path = root("strict");
        let workspace = Workspace::open_or_create(&path, None).unwrap();
        assert!(workspace.save(1, br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[],"extra":true}"#).is_err());
        assert!(workspace.save(1, b"").is_err());
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn existing_empty_root_initializes_and_unmarked_nonempty_rejects() {
        let empty = root("empty");
        fs::create_dir(&empty).unwrap();
        let workspace = Workspace::open_or_create(&empty, None).unwrap();
        assert_eq!(workspace.latest().unwrap().revision, 1);
        let marked = root("nonempty");
        fs::create_dir(&marked).unwrap();
        fs::write(marked.join("unrelated"), b"x").unwrap();
        assert!(matches!(
            Workspace::open_or_create(&marked, None),
            Err(WorkspaceError::InvalidWorkspace(_))
        ));
        let _ = fs::remove_dir_all(empty);
        let _ = fs::remove_dir_all(marked);
    }

    #[test]
    fn invalid_initial_content_does_not_create_root() {
        let path = root("invalid-initial");
        let input = path.with_extension("json");
        fs::write(&input, br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[],"unknown":true}"#).unwrap();
        assert!(Workspace::open_or_create(&path, Some(&input)).is_err());
        assert!(!path.exists());
        let _ = fs::remove_file(input);
    }

    #[test]
    fn corrupt_noncanonical_and_gap_revisions_reject_on_open() {
        let path = root("corrupt");
        let _workspace = Workspace::open_or_create(&path, None).unwrap();
        fs::write(
            path.join("revisions").join("00000000000000000002.json"),
            b"{}",
        )
        .unwrap();
        assert!(Workspace::open_or_create(&path, None).is_err());
        let _ = fs::remove_dir_all(path);
        let path = root("noncanonical");
        let workspace = Workspace::open_or_create(&path, None).unwrap();
        let revision = path.join("revisions").join("00000000000000000001.json");
        fs::write(&revision, br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[{"id":"draft:rooms.start","title":"First Room","description":"An unfinished place.","exits":{},"allow_unreachable":false}]} "#).unwrap();
        assert!(workspace.latest().is_err());
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn sequential_cross_instance_save_conflicts() {
        let path = root("instances");
        let first = Workspace::open_or_create(&path, None).unwrap();
        let second = Workspace::open_or_create(&path, None).unwrap();
        let bytes = serialize_draft(
            "test",
            &first.latest().unwrap().draft,
            ContentLimits::default(),
        )
        .unwrap();
        assert_eq!(first.save(1, &bytes).unwrap().revision, 2);
        assert!(matches!(
            second.save(1, &bytes),
            Err(WorkspaceError::Conflict {
                current_revision: 2
            })
        ));
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn revision_filename_capacity_rules_are_bounded() {
        let path = root("entries");
        let workspace = Workspace::open_or_create(&path, None).unwrap();
        fs::write(
            path.join("revisions").join("00000000000000000000.json"),
            b"x",
        )
        .unwrap();
        assert!(workspace.latest().is_err());
        let _ = fs::remove_dir_all(path);
    }
}
