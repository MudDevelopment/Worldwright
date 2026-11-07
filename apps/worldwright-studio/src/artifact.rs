use crate::{
    preview::PreviewReceipt,
    workspace::{Workspace, WorkspaceError},
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    fs::{self, OpenOptions},
    io::Write,
};
use worldwright_release::{
    PackageOptions, ReleaseId, Sha256Digest, package_bytes, verify_bytes, write_new,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageRequest {
    pub revision: u64,
    pub snapshot_sha256: String,
    pub release_id: String,
    pub builder_identity: String,
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRecord {
    pub schema_version: u32,
    pub artifact_digest: String,
    pub payload_digest: String,
    pub draft_revision: u64,
    pub release_id: String,
    pub builder_identity: String,
    pub created_at: u64,
}

impl Workspace {
    pub fn package(&self, request: PackageRequest) -> Result<ArtifactRecord, WorkspaceError> {
        let snapshot = self.revision(request.revision)?;
        let requested_digest = Sha256Digest::parse(&request.snapshot_sha256)
            .map_err(|_| WorkspaceError::InvalidWorkspace("snapshot digest is invalid"))?;
        if requested_digest.to_string() != request.snapshot_sha256
            || snapshot.snapshot_sha256 != request.snapshot_sha256
        {
            return Err(WorkspaceError::Conflict {
                current_revision: snapshot.revision,
            });
        }
        let release_id: ReleaseId = request
            .release_id
            .parse()
            .map_err(|_| WorkspaceError::InvalidWorkspace("release ID is invalid"))?;
        let bytes = self.export(request.revision)?;
        let artifact = package_bytes(
            "Studio draft",
            &bytes,
            PackageOptions {
                release_id,
                source_revision: format!(
                    "studio-r{}-{}",
                    request.revision, request.snapshot_sha256
                ),
                builder_identity: request.builder_identity.clone(),
            },
        )
        .map_err(|_| WorkspaceError::InvalidWorkspace("draft cannot be packaged"))?;
        let verified = verify_bytes("Studio artifact", &artifact, None)
            .map_err(|_| WorkspaceError::InvalidWorkspace("artifact verification failed"))?;
        if verified.manifest().payload_sha256().to_string() != snapshot.snapshot_sha256 {
            return Err(WorkspaceError::InvalidWorkspace(
                "artifact payload digest mismatch",
            ));
        }
        let digest = verified.artifact_digest().to_string();
        let payload = verified.manifest().payload_sha256().to_string();
        if self.artifact_path(&digest).exists() || self.artifact_sidecar_path(&digest).exists() {
            let existing = self.artifact(&digest)?;
            if existing.draft_revision == request.revision
                && existing.payload_digest == payload
                && existing.release_id == verified.manifest().release_id().as_str()
                && existing.builder_identity == request.builder_identity
            {
                return Ok(existing);
            }
            return Err(WorkspaceError::InvalidWorkspace(
                "artifact identity conflicts",
            ));
        }
        let record = ArtifactRecord {
            schema_version: 1,
            artifact_digest: digest.clone(),
            payload_digest: payload,
            draft_revision: request.revision,
            release_id: verified.manifest().release_id().as_str().into(),
            builder_identity: request.builder_identity,
            created_at: now(),
        };
        let artifact_path = self.artifact_path(&digest);
        match write_new(&artifact_path, &artifact).map_err(|_| {
            WorkspaceError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "artifact write failed",
            ))
        })? {
            worldwright_release::WriteOutcome::Created
            | worldwright_release::WriteOutcome::Reused => {}
        }
        let sidecar = self.artifact_sidecar_path(&digest);
        let encoded = serde_json::to_vec(&record).map_err(|_| {
            WorkspaceError::InvalidWorkspace("artifact metadata serialization failed")
        })?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&sidecar)
        {
            Ok(mut file) => {
                file.write_all(&encoded)?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = fs::read(&sidecar)?;
                if existing != encoded {
                    return Err(WorkspaceError::InvalidWorkspace(
                        "artifact sidecar conflicts",
                    ));
                }
            }
            Err(error) => return Err(WorkspaceError::Io(error)),
        }
        Ok(record)
    }

    pub fn artifact(&self, digest: &str) -> Result<ArtifactRecord, WorkspaceError> {
        let digest = parse_digest(digest)?;
        let sidecar = self.artifact_sidecar_path(&digest);
        let sidecar_meta = sidecar.symlink_metadata()?;
        if !sidecar_meta.is_file()
            || sidecar_meta.file_type().is_symlink()
            || sidecar_meta.len() > 4096
        {
            return Err(WorkspaceError::InvalidWorkspace("artifact sidecar invalid"));
        }
        let bytes = fs::read(sidecar)?;
        let record: ArtifactRecord = serde_json::from_slice(&bytes)
            .map_err(|_| WorkspaceError::InvalidWorkspace("artifact sidecar invalid"))?;
        let artifact_path = self.artifact_path(&digest);
        let artifact_meta = artifact_path.symlink_metadata()?;
        if !artifact_meta.is_file()
            || artifact_meta.file_type().is_symlink()
            || artifact_meta.len() > worldwright_release::MAX_ARTIFACT_BYTES as u64
        {
            return Err(WorkspaceError::InvalidWorkspace("artifact invalid"));
        }
        let artifact = fs::read(artifact_path)?;
        let verified = verify_bytes("Studio artifact", &artifact, None)
            .map_err(|_| WorkspaceError::InvalidWorkspace("artifact invalid"))?;
        let snapshot = self.revision(record.draft_revision)?;
        if record.schema_version != 1
            || record.created_at == 0
            || record.artifact_digest != digest
            || record.payload_digest != verified.manifest().payload_sha256().to_string()
            || record.payload_digest != snapshot.snapshot_sha256
            || record.release_id != verified.manifest().release_id().as_str()
            || record.builder_identity != verified.manifest().builder_identity()
            || verified.artifact_digest().to_string() != digest
        {
            return Err(WorkspaceError::InvalidWorkspace(
                "artifact sidecar mismatch",
            ));
        }
        Ok(record)
    }
    pub fn artifacts(&self) -> Result<Vec<ArtifactRecord>, WorkspaceError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(self.artifacts_dir())? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() || entry.path().symlink_metadata()?.file_type().is_symlink() {
                return Err(WorkspaceError::InvalidWorkspace(
                    "artifact entry is invalid",
                ));
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let digest = if name.ends_with(".json") {
                &name[..name.len() - 5]
            } else if name.ends_with(".wwr") {
                &name[..name.len() - 4]
            } else {
                return Err(WorkspaceError::InvalidWorkspace(
                    "artifact entry is unexpected",
                ));
            };
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(WorkspaceError::InvalidWorkspace(
                    "artifact entry is unexpected",
                ));
            }
            if name.ends_with(".json") {
                records.push(self.artifact(digest)?);
                if records.len() > 100 {
                    return Err(WorkspaceError::Capacity);
                }
                if !self.artifact_path(digest).is_file() {
                    return Err(WorkspaceError::InvalidWorkspace("artifact pair missing"));
                }
            } else if !self.artifact_sidecar_path(digest).is_file() {
                return Err(WorkspaceError::InvalidWorkspace("artifact pair missing"));
            }
        }
        Ok(records)
    }
    pub fn artifact_bytes(&self, digest: &str) -> Result<Vec<u8>, WorkspaceError> {
        let digest = parse_digest(digest)?;
        let path = self.artifact_path(&digest);
        let metadata = path.symlink_metadata()?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > worldwright_release::MAX_ARTIFACT_BYTES as u64
        {
            return Err(WorkspaceError::InvalidWorkspace("artifact invalid"));
        }
        let bytes = fs::read(path)?;
        if digest_bytes(&bytes) != digest {
            return Err(WorkspaceError::InvalidWorkspace("artifact digest mismatch"));
        }
        Ok(bytes)
    }
    pub(crate) fn write_preview_receipt(
        &self,
        digest: &str,
        value: &PreviewReceipt,
    ) -> Result<(), WorkspaceError> {
        let digest = parse_digest(digest)?;
        validate_receipt_identity(self, &digest, value)?;
        let path = self.preview_receipt_path(&digest);
        let bytes = serde_json::to_vec(value)
            .map_err(|_| WorkspaceError::InvalidWorkspace("receipt serialization failed"))?;
        if bytes.len() > 4096 {
            return Err(WorkspaceError::InvalidWorkspace("receipt exceeds bounds"));
        }
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(&bytes)?;
                file.sync_all()?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = self.preview_receipt(&digest)?;
                if same_receipt_identity(&existing, value) {
                    Ok(())
                } else {
                    Err(WorkspaceError::InvalidWorkspace("receipt conflicts"))
                }
            }
            Err(error) => Err(WorkspaceError::Io(error)),
        }
    }

    pub(crate) fn preview_receipt(&self, digest: &str) -> Result<PreviewReceipt, WorkspaceError> {
        let digest = parse_digest(digest)?;
        let path = self.preview_receipt_path(&digest);
        let metadata = path.symlink_metadata()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
            return Err(WorkspaceError::InvalidWorkspace("preview receipt invalid"));
        }
        let bytes = fs::read(path)?;
        let receipt: PreviewReceipt = serde_json::from_slice(&bytes)
            .map_err(|_| WorkspaceError::InvalidWorkspace("preview receipt invalid"))?;
        validate_receipt_identity(self, &digest, &receipt)?;
        Ok(receipt)
    }
}

fn validate_receipt_identity(
    workspace: &Workspace,
    digest: &str,
    receipt: &PreviewReceipt,
) -> Result<(), WorkspaceError> {
    let artifact = workspace.artifact(digest)?;
    if receipt.schema_version != 1
        || receipt.result != "succeeded"
        || receipt.completed_at == 0
        || receipt.artifact_digest != artifact.artifact_digest
        || receipt.payload_digest != artifact.payload_digest
        || receipt.draft_revision != artifact.draft_revision
    {
        return Err(WorkspaceError::InvalidWorkspace(
            "preview receipt identity mismatch",
        ));
    }
    Ok(())
}

fn same_receipt_identity(left: &PreviewReceipt, right: &PreviewReceipt) -> bool {
    left.schema_version == right.schema_version
        && left.artifact_digest == right.artifact_digest
        && left.payload_digest == right.payload_digest
        && left.draft_revision == right.draft_revision
        && left.result == right.result
}

fn parse_digest(value: &str) -> Result<String, WorkspaceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(WorkspaceError::InvalidWorkspace(
            "artifact digest is invalid",
        ));
    }
    Ok(value.to_owned())
}
fn digest_bytes(bytes: &[u8]) -> String {
    let mut hash = sha2::Sha256::new();
    hash.update(bytes);
    let mut output = String::with_capacity(64);
    for byte in hash.finalize() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("String formatting cannot fail");
    }
    output
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn package_writes_immutable_artifact_and_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "worldwright-studio-artifact-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = Workspace::open_or_create(&root, None).unwrap();
        let snapshot = workspace.latest().unwrap();
        let record = workspace
            .package(PackageRequest {
                revision: snapshot.revision,
                snapshot_sha256: snapshot.snapshot_sha256.clone(),
                release_id: "studio-test".into(),
                builder_identity: "studio-test".into(),
            })
            .unwrap();
        let reused = workspace
            .package(PackageRequest {
                revision: snapshot.revision,
                snapshot_sha256: snapshot.snapshot_sha256,
                release_id: "studio-test".into(),
                builder_identity: "studio-test".into(),
            })
            .unwrap();
        assert_eq!(record, reused);
        assert_eq!(workspace.artifact(&record.artifact_digest).unwrap(), record);
        assert!(
            !workspace
                .artifact_bytes(&record.artifact_digest)
                .unwrap()
                .is_empty()
        );
        let receipt = PreviewReceipt {
            schema_version: 1,
            artifact_digest: record.artifact_digest.clone(),
            payload_digest: record.payload_digest.clone(),
            draft_revision: record.draft_revision,
            completed_at: 100,
            result: "succeeded".into(),
        };
        workspace
            .write_preview_receipt(&record.artifact_digest, &receipt)
            .unwrap();
        assert_eq!(
            workspace.preview_receipt(&record.artifact_digest).unwrap(),
            receipt
        );
        let _ = fs::remove_dir_all(root);
    }
}
