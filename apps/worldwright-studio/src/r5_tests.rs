//! R5 local file/contract regressions, not live operator/publication acceptance.
use crate::{artifact::PackageRequest, cli, workspace::Workspace};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use worldwright_operations::PublicationView;
use worldwright_release::{DeploymentTarget, PublicationKind, PublicationStatus};

fn workspace() -> (Workspace, PathBuf) {
    let root = std::env::temp_dir().join(format!("worldwright-r5-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    (Workspace::open_or_create(&root, None).unwrap(), root)
}
fn view(status: PublicationStatus) -> PublicationView {
    PublicationView {
        request_id: format!("{:032x}", 22), world_id: format!("{:032x}", 1),
        release_id: "r5-test".into(), artifact_digest: "a".repeat(64),
        kind: PublicationKind::Activate, target: DeploymentTarget::Staging,
        expected_current: None, requested_by: "builder".into(), status,
        reviewed_by: if status == PublicationStatus::Requested { None } else { Some("independent-reviewer".into()) },
        review_reason: None, activation_executor: None, review_context: None,
    }
}
fn write_view(path: &std::path::Path, value: &PublicationView) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
#[test]
fn r5_recovery_reconciles_every_valid_interruption_layout() {
    for layout in 0..5 {
        let (ws, root) = workspace();
        let old = view(PublicationStatus::Requested);
        let next = view(PublicationStatus::Approved);
        let path = ws.publication_records_dir().join(format!("{}.json", old.request_id));
        let previous = path.with_extension("json.previous");
        let pending = path.with_extension("json.next");
        let revision = ws.latest().unwrap();
        let before = ws.export(revision.revision).unwrap();
        match layout {
            0 => write_view(&path, &old),
            1 => { write_view(&path, &old); write_view(&pending, &next); }
            2 => { write_view(&previous, &old); write_view(&pending, &next); }
            3 => { write_view(&previous, &old); write_view(&path, &next); }
            4 => write_view(&previous, &old),
            _ => unreachable!(),
        }
        assert_eq!(ws.publication_recovery().unwrap().len(), if layout != 0 { 1 } else { 0 });
        let expected = if layout == 0 || layout == 4 { old } else { next };
        let result = ws.recover_publication_record(&expected.request_id).unwrap();
        assert_eq!(result, expected);
        assert_eq!(ws.recover_publication_record(&expected.request_id).unwrap(), expected);
        assert!(!pending.exists()); assert!(!previous.exists());
        assert!(ws.publication_recovery().unwrap().is_empty());
        assert_eq!(ws.publication_records().unwrap(), vec![expected]);
        assert_eq!(ws.export(revision.revision).unwrap(), before);
        assert_eq!(fs::read(root.join(".worldwright-studio")).unwrap(), crate::workspace::WORKSPACE_MARKER);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn r5_recovery_refuses_ambiguous_corrupt_and_conflicting_records() {
    for case in 0..7 {
        let (ws, root) = workspace();
        let old = view(PublicationStatus::Requested);
        let mut next = view(PublicationStatus::Approved);
        let path = ws.publication_records_dir().join(format!("{}.json", old.request_id));
        let previous = path.with_extension("json.previous");
        let pending = path.with_extension("json.next");
        write_view(&path, &old);
        match case {
            0 => { write_view(&previous, &old); write_view(&pending, &next); }
            1 => { next.artifact_digest = "b".repeat(64); write_view(&pending, &next); }
            2 => { next.request_id = format!("{:032x}", 23); write_view(&pending, &next); }
            3 => { write_view(&path, &next); write_view(&pending, &old); }
            4 => { fs::write(&pending, b"{partial").unwrap(); }
            5 => { fs::remove_file(&path).unwrap(); write_view(&pending, &next); }
            6 => { next.review_context = Some(worldwright_operations::StudioReviewContext {
                schema_version: 1, draft_revision: 1, snapshot_sha256: "c".repeat(64),
                validation_diagnostic_count: 0, preview_artifact_sha256: "a".repeat(64),
                preview_completed_at: 1, change_summary: "changed review context".into(),
            }); write_view(&pending, &next); }
            _ => unreachable!(),
        }
        let before: Vec<_> = [&path, &previous, &pending].iter().map(|p| fs::read(p).ok()).collect();
        assert!(ws.recover_publication_record(&old.request_id).is_err());
        let after: Vec<_> = [&path, &previous, &pending].iter().map(|p| fs::read(p).ok()).collect();
        assert_eq!(before, after);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn r5_recovery_inventory_and_identifier_are_bounded() {
    let (ws, root) = workspace();
    for id in ["../escape", "0", "", "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"] {
        assert!(ws.recover_publication_record(id).is_err());
    }
    fs::write(ws.publication_records_dir().join("unexpected.txt"), b"keep").unwrap();
    assert!(ws.publication_recovery().is_err());
    assert_eq!(fs::read(ws.publication_records_dir().join("unexpected.txt")).unwrap(), b"keep");
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn r5_recovery_refuses_symlink_without_touching_target() {
    let (ws, root) = workspace();
    let old = view(PublicationStatus::Requested);
    let path = ws.publication_records_dir().join(format!("{}.json", old.request_id));
    write_view(&path, &old);
    let external = root.join("external.json"); write_view(&external, &view(PublicationStatus::Approved));
    let before = fs::read(&external).unwrap();
    std::os::unix::fs::symlink(&external, path.with_extension("json.next")).unwrap();
    assert!(ws.publication_recovery().is_err());
    assert!(ws.recover_publication_record(&old.request_id).is_err());
    assert_eq!(fs::read(&external).unwrap(), before);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn r5_artifact_identity_comes_from_bytes_without_sidecar_migration() {
    let (ws, root) = workspace();
    let snapshot = ws.latest().unwrap();
    for game in ["reference", "expedition"] {
        let record = ws.package(PackageRequest {
            revision: snapshot.revision, snapshot_sha256: snapshot.snapshot_sha256.clone(),
            release_id: format!("r5-{game}"), builder_identity: "r5-test".into(), game: Some(game.into()),
        }).unwrap();
        let path = root.join("artifacts").join(format!("{}.json", record.artifact_digest));
        let before = fs::read(&path).unwrap();
        let identity = ws.artifact_identity(&record.artifact_digest).unwrap();
        assert_eq!(identity.game, game); assert_eq!(identity.artifact_digest, record.artifact_digest);
        assert_eq!(identity.payload_digest, snapshot.snapshot_sha256);
        assert_eq!(identity.draft_revision, snapshot.revision);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(serde_json::from_slice::<serde_json::Value>(&before).unwrap().get("game").is_none());
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn r5_token_source_cli_is_exclusive_and_explicit() {
    let path = std::env::temp_dir().join("worldwright-r5-token");
    let file = path.to_str().unwrap();
    let args = |parts: &[&str]| parts.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    assert!(matches!(cli::parse(args(&["serve", "--workspace", "draft", "--release-endpoint", "http://127.0.0.1:1234", "--operator-token-file", file])), Ok(cli::Command::Serve(_))));
    for extra in [vec!["--operator-token-env", "TOKEN", "--operator-token-file", file], vec!["--operator-token-file", "relative"], vec!["--operator-token-file", file, "--operator-token-file", file]] {
        let mut all = vec!["serve", "--workspace", "draft", "--release-endpoint", "http://127.0.0.1:1234"];
        all.extend(extra); assert!(cli::parse(args(&all)).is_err());
    }
}

#[test]
fn r5_publication_capacity_keeps_exact_existing_insert_idempotent() {
    let (ws, root) = workspace();
    for id in 1..=100 {
        let mut record = view(PublicationStatus::Requested);
        record.request_id = format!("{id:032x}");
        ws.insert_publication_record(&record).unwrap();
    }
    let existing = view(PublicationStatus::Requested);
    ws.insert_publication_record(&existing).unwrap();
    let mut extra = existing.clone(); extra.request_id = format!("{:032x}", 101);
    assert!(ws.insert_publication_record(&extra).is_err());
    assert_eq!(ws.publication_records().unwrap().len(), 100);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn r5_publication_update_preserves_dangling_journal_and_hard_link_evidence() {
    let (ws, root) = workspace();
    let old = view(PublicationStatus::Requested);
    let next = view(PublicationStatus::Approved);
    ws.insert_publication_record(&old).unwrap();
    let path = ws.publication_records_dir().join(format!("{}.json", old.request_id));
    let previous = path.with_extension("json.previous");
    std::os::unix::fs::symlink(root.join("missing"), &previous).unwrap();
    assert!(ws.update_publication_record(&next).is_err());
    assert!(previous.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(ws.publication_record(&old.request_id).unwrap(), old);
    fs::remove_file(&previous).unwrap();
    fs::hard_link(&path, root.join("outside-record")).unwrap();
    assert!(ws.publication_record(&old.request_id).is_err());
    assert!(ws.recover_publication_record(&old.request_id).is_err());
    assert_eq!(fs::read(&path).unwrap(), serde_json::to_vec(&old).unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn r5_publication_read_refuses_oversized_and_wrong_filename_identity() {
    let (ws, root) = workspace();
    let old = view(PublicationStatus::Requested);
    let path = ws.publication_records_dir().join(format!("{}.json", old.request_id));
    fs::write(&path, vec![b' '; 65_537]).unwrap();
    assert!(ws.publication_record(&old.request_id).is_err());
    let mut wrong = old.clone(); wrong.request_id = format!("{:032x}", 23);
    write_view(&path, &wrong);
    assert!(ws.publication_record(&old.request_id).is_err());
    fs::remove_dir_all(root).unwrap();
}
