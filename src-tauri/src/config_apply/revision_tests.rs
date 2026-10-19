//! Integration tests for pre-mutation snapshots using temporary files and the
//! real SQLite log. Fault injection uses local triggers, never real credentials.

#[path = "cleanup_tests.rs"]
mod cleanup_tests;
#[path = "history_surface_tests.rs"]
mod history_surface_tests;
#[path = "history_tests.rs"]
mod history_tests;
#[path = "recovery_surface_tests.rs"]
mod recovery_surface_tests;
#[path = "resolution_tests.rs"]
mod resolution_tests;

use super::*;
use crate::{config_plan::read_target_bytes, storage::Storage};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture {
    root: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-revisions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open_in_directory(&root).unwrap();
        Self { root, storage }
    }

    fn backups(&self) -> BackupStore {
        BackupStore::new(self.root.join("backups"))
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PlannedFile {
        let target = read_target_bytes(&self.root.join(name)).unwrap().0;
        PlannedFile {
            target: WriteTarget::from_plan(&target),
            content: bytes.to_vec(),
        }
    }

    fn apply(
        &self,
        kind: OperationKind,
        files: &[PlannedFile],
    ) -> Result<AppliedOperation, WriteError> {
        apply_files(&self.storage, kind, None, files, &self.backups())
    }

    fn restore(&self, id: &str) -> Result<RestoreOutcome, WriteError> {
        let plan = plan_restore(&self.storage, id).unwrap().unwrap();
        let reviewed: Vec<_> = plan
            .steps
            .iter()
            .filter(|step| step.action.changes_files())
            .map(|step| ReviewedRestoreStep {
                ordinal: step.ordinal,
                current_digest: step.current_digest.clone(),
            })
            .collect();
        apply_restore(&self.storage, id, &reviewed, &self.backups())
    }

    fn sql(&self, sql: &str) {
        self.storage.lock().unwrap().execute_batch(sql).unwrap();
    }

    fn revisions(&self) -> Vec<Revision> {
        revisions(&self.storage)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
struct Revision {
    id: String,
    batch: String,
    operation: String,
    ordinal: u32,
    intent: String,
    before: Option<String>,
    after: Option<String>,
    backup: Option<String>,
    state: String,
}

fn revisions(storage: &Storage) -> Vec<Revision> {
    let connection = storage.lock().unwrap();
    let mut query = connection
        .prepare(
            "SELECT id, batch_id, operation_id, ordinal, intent, previous_digest,
            next_digest, backup_path, state FROM config_file_revision ORDER BY created_at, rowid",
        )
        .unwrap();
    query
        .query_map([], |row| {
            Ok(Revision {
                id: row.get(0)?,
                batch: row.get(1)?,
                operation: row.get(2)?,
                ordinal: row.get(3)?,
                intent: row.get(4)?,
                before: row.get(5)?,
                after: row.get(6)?,
                backup: row.get(7)?,
                state: row.get(8)?,
            })
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn every_adapter_keeps_exact_independent_pre_images_across_writes_and_restart() {
    for kind in [
        OperationKind::ClaudeCodeSettings,
        OperationKind::PiSettings,
        OperationKind::GrokSettings,
    ] {
        let fixture = Fixture::new();
        // Byte preservation is independent of JSON/TOML syntax or UTF-8.
        let original = b"# original\r\nkey = 'synthetic-private'\r\n\xff";
        fs::write(fixture.root.join("config"), original).unwrap();
        let first = fixture
            .apply(kind, &[fixture.file("config", b"B")])
            .unwrap();
        let second = fixture
            .apply(kind, &[fixture.file("config", b"C")])
            .unwrap();
        let rows = fixture.revisions();
        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0].id, rows[1].id);
        assert_eq!(rows[0].operation, first.operation_id);
        assert_eq!(rows[1].operation, second.operation_id);
        assert_eq!(rows[0].batch, first.operation_id);
        assert_eq!(rows[0].ordinal, 0);
        assert_eq!(rows[0].intent, "apply");
        assert_eq!(rows[0].before, Some(digest_hex(original)));
        assert_eq!(rows[0].after, Some(digest_hex(b"B")));
        assert_eq!(rows[1].before, Some(digest_hex(b"B")));
        assert_eq!(
            fs::read(rows[0].backup.as_ref().unwrap()).unwrap(),
            original
        );
        assert_eq!(fs::read(rows[1].backup.as_ref().unwrap()).unwrap(), b"B");
        assert!(rows.iter().all(|row| row.state == "completed"));
        let reopened = Storage::open_in_directory(&fixture.root).unwrap();
        assert_eq!(revisions(&reopened).len(), 2);
        let connection = reopened.lock().unwrap();
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(connection);
        for entry in fs::read_dir(&fixture.root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file()
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("sqlite")
            {
                assert!(
                    !fs::read(path)
                        .unwrap()
                        .windows(b"synthetic-private".len())
                        .any(|bytes| bytes == b"synthetic-private")
                );
            }
        }
    }
}

#[test]
fn absence_and_unchanged_submissions_are_not_confused_with_empty_files() {
    let fixture = Fixture::new();
    let applied = fixture
        .apply(OperationKind::PiSettings, &[fixture.file("new", b"")])
        .unwrap();
    fixture
        .apply(OperationKind::PiSettings, &[fixture.file("new", b"")])
        .unwrap();
    let rows = fixture.revisions();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].before, None);
    assert_eq!(rows[0].backup, None);
    assert_eq!(rows[0].after, Some(digest_hex(b"")));
    fixture.restore(&applied.operation_id).unwrap();
    assert!(!fixture.root.join("new").exists());
    let rows = fixture.revisions();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].intent, "restore");
    assert_eq!(rows[1].before, Some(digest_hex(b"")));
    assert_eq!(rows[1].after, None);
    assert_eq!(fs::read(rows[1].backup.as_ref().unwrap()).unwrap(), b"");
}

#[test]
fn manual_restore_keeps_a_separate_batch_and_all_of_its_pre_images() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"A1").unwrap();
    fs::write(fixture.root.join("b"), b"B1").unwrap();
    let applied = fixture
        .apply(
            OperationKind::PiSettings,
            &[fixture.file("a", b"A2"), fixture.file("b", b"B2")],
        )
        .unwrap();
    fixture.restore(&applied.operation_id).unwrap();
    let rows = fixture.revisions();
    assert_eq!(rows.len(), 4);
    let restored = &rows[2..];
    assert!(
        restored
            .iter()
            .all(|row| row.intent == "restore" && row.state == "completed")
    );
    assert_eq!(restored[0].batch, restored[1].batch);
    assert_ne!(restored[0].batch, applied.operation_id);
    assert_eq!((restored[0].ordinal, restored[1].ordinal), (1, 0));
    assert_eq!(
        fs::read(restored[0].backup.as_ref().unwrap()).unwrap(),
        b"B2"
    );
    assert_eq!(
        fs::read(restored[1].backup.as_ref().unwrap()).unwrap(),
        b"A2"
    );
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
    assert_eq!(
        recent_operations(&fixture.storage, OperationKind::PiSettings, 50)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn automatic_compensation_keeps_the_content_it_replaces_or_deletes() {
    for existed in [false, true] {
        let fixture = Fixture::new();
        if existed {
            fs::write(fixture.root.join("a"), b"old").unwrap();
        }
        let first = fixture.file("a", b"new");
        let second = fixture.file("b", b"never written");
        fs::write(fixture.root.join("b"), b"external change").unwrap();
        let result = fixture
            .apply(OperationKind::GrokSettings, &[first, second])
            .unwrap();
        assert_eq!(result.state, OperationState::Compensated);
        let rows = fixture.revisions();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].intent, "compensation");
        assert_eq!(rows[1].operation, result.operation_id);
        assert_eq!(rows[1].state, "completed");
        assert_eq!(fs::read(rows[1].backup.as_ref().unwrap()).unwrap(), b"new");
        assert_eq!(rows[1].after, existed.then(|| digest_hex(b"old")));
        assert_eq!(fixture.root.join("a").exists(), existed);
    }
}

#[test]
fn failed_pre_write_journal_never_changes_existing_or_absent_targets() {
    for existed in [false, true] {
        let fixture = Fixture::new();
        if existed {
            fs::write(fixture.root.join("a"), b"old").unwrap();
        }
        fixture.sql("CREATE TRIGGER refuse_revision BEFORE INSERT ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
        assert_eq!(
            fixture
                .apply(
                    OperationKind::ClaudeCodeSettings,
                    &[fixture.file("a", b"new")]
                )
                .unwrap_err(),
            WriteError::LogWriteFailed
        );
        assert_eq!(fixture.root.join("a").exists(), existed);
        if existed {
            assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"old");
        }
        assert!(fixture.revisions().is_empty());
    }
}

#[test]
fn interrupted_result_recording_keeps_prepared_snapshot_and_blocks_legacy_replay() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"old").unwrap();
    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        fixture
            .apply(
                OperationKind::ClaudeCodeSettings,
                &[fixture.file("a", b"new")]
            )
            .unwrap_err(),
        WriteError::LogWriteFailed
    );
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
    let rows = fixture.revisions();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, "prepared");
    assert_eq!(fs::read(rows[0].backup.as_ref().unwrap()).unwrap(), b"old");
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    assert_eq!(revisions(&reopened)[0].state, "prepared");
    assert_eq!(
        compensate_operation(&reopened, &rows[0].operation, &fixture.backups()).unwrap_err(),
        WriteError::OutcomeUnknown
    );
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
}

#[test]
fn restore_journal_failure_blocks_replacement_and_removal() {
    for existed in [false, true] {
        let fixture = Fixture::new();
        if existed {
            fs::write(fixture.root.join("a"), b"old").unwrap();
        }
        let applied = fixture
            .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
            .unwrap();
        fixture.sql("CREATE TRIGGER refuse_restore BEFORE INSERT ON config_file_revision WHEN NEW.intent = 'restore' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
        assert_eq!(
            fixture.restore(&applied.operation_id).unwrap_err(),
            WriteError::LogWriteFailed
        );
        assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
        assert_eq!(fixture.revisions().len(), 1);
    }
}

#[test]
fn an_interrupted_restore_is_not_reversed_by_legacy_recovery() {
    for existed in [false, true] {
        let fixture = Fixture::new();
        if existed {
            fs::write(fixture.root.join("a"), b"old").unwrap();
        }
        let applied = fixture
            .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
            .unwrap();
        fixture.sql("CREATE TRIGGER refuse_restore_result BEFORE UPDATE ON config_file_revision WHEN OLD.intent = 'restore' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
        assert_eq!(
            fixture.restore(&applied.operation_id).unwrap_err(),
            WriteError::LogWriteFailed
        );
        assert_eq!(fixture.root.join("a").exists(), existed);
        let rows = fixture.revisions();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].intent, "restore");
        assert_eq!(rows[1].state, "prepared");
        assert_eq!(fs::read(rows[1].backup.as_ref().unwrap()).unwrap(), b"new");
        assert_eq!(
            fixture.restore(&applied.operation_id).unwrap_err(),
            WriteError::OutcomeUnknown
        );
        // Simulate a legacy operation that was already partial at interruption.
        finish_operation(
            &fixture.storage,
            &applied.operation_id,
            OperationState::Partial,
        )
        .unwrap();
        assert_eq!(
            compensate_operation(&fixture.storage, &applied.operation_id, &fixture.backups())
                .unwrap_err(),
            WriteError::OutcomeUnknown
        );
        assert_eq!(fixture.root.join("a").exists(), existed);
        if existed {
            assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"old");
        }
    }
}

#[test]
fn uncertain_mutations_retain_their_snapshot_and_require_reconciliation() {
    use super::revisions::{RevisionContext, RevisionIntent, require_known_revisions};
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"old").unwrap();
    let file = fixture.file("a", b"new");
    let source = begin_operation(
        &fixture.storage,
        OperationKind::PiSettings,
        None,
        std::slice::from_ref(&file.target),
    )
    .unwrap();
    let backup = fixture.backups().store("uncertain", "a", b"old").unwrap();
    let revision = RevisionContext {
        storage: &fixture.storage,
        operation_id: &source,
        ordinal: 0,
        batch_id: "batch",
        intent: RevisionIntent::Apply,
    };
    revision
        .prepare(
            "uncertain",
            &file.target,
            Some(&backup),
            Some(&digest_hex(b"new")),
        )
        .unwrap();
    revision
        .finish::<()>("uncertain", &Err(WriteError::OutcomeUnknown))
        .unwrap();
    assert_eq!(fixture.revisions()[0].state, "unknown");
    let inspected = inspect_interrupted_revisions(&fixture.storage, &source, &fixture.backups())
        .unwrap()
        .unwrap();
    assert_eq!(inspected[0].state, "unknown");
    assert_eq!(inspected[0].current_match, RevisionMatch::Previous);
    assert_eq!(inspected[0].backup_state, RevisionBackupState::Available);
    assert_eq!(fs::read(&backup.path).unwrap(), b"old");
    assert_eq!(
        require_known_revisions(&fixture.storage, &source),
        Err(WriteError::OutcomeUnknown)
    );
}

/// Leave a real snapshot at the crash boundary after the target changed but
/// before its result could be committed. No hand-built log rows are needed.
fn interrupted_apply(fixture: &Fixture, existed: bool) -> String {
    if existed {
        fs::write(fixture.root.join("a"), b"old").unwrap();
    }
    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        fixture
            .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
            .unwrap_err(),
        WriteError::LogWriteFailed
    );
    fixture.sql("DROP TRIGGER refuse_result;");
    fixture.revisions()[0].operation.clone()
}

fn inspect(fixture: &Fixture, id: &str) -> Vec<RevisionInspection> {
    inspect_interrupted_revisions(&fixture.storage, id, &fixture.backups())
        .unwrap()
        .unwrap()
}

#[test]
fn inspection_after_restart_reports_intended_bytes_without_settling_or_exposing_them() {
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let before = operation(&fixture.storage, &id).unwrap();
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let results = inspect_interrupted_revisions(&reopened, &id, &fixture.backups())
        .unwrap()
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].current_match, RevisionMatch::Intended);
    assert_eq!(results[0].backup_state, RevisionBackupState::Available);
    assert_eq!(results[0].current_digest, Some(digest_hex(b"new")));
    assert_eq!(results[0].error_code, None);
    assert_eq!(operation(&fixture.storage, &id).unwrap(), before);
    assert_eq!(fixture.revisions()[0].state, "prepared");
    assert_eq!(
        compensate_operation(&fixture.storage, &id, &fixture.backups()).unwrap_err(),
        WriteError::OutcomeUnknown
    );
    let serialized = serde_json::to_string(&results).unwrap();
    assert!(!serialized.contains("\"old\""));
    assert!(!serialized.contains("\"new\""));
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
}

#[test]
fn inspection_distinguishes_previous_diverged_absence_and_empty_content() {
    for existed in [false, true] {
        let fixture = Fixture::new();
        let id = interrupted_apply(&fixture, existed);
        if existed {
            fs::write(fixture.root.join("a"), b"old").unwrap();
        } else {
            fs::remove_file(fixture.root.join("a")).unwrap();
        }
        assert_eq!(
            inspect(&fixture, &id)[0].current_match,
            RevisionMatch::Previous
        );
        assert_eq!(
            inspect(&fixture, &id)[0].backup_state,
            if existed {
                RevisionBackupState::Available
            } else {
                RevisionBackupState::NotNeeded
            }
        );
        fs::write(fixture.root.join("a"), b"").unwrap();
        assert_eq!(
            inspect(&fixture, &id)[0].current_match,
            RevisionMatch::Diverged
        );
        assert_eq!(
            inspect(&fixture, &id)[0].current_digest,
            Some(digest_hex(b""))
        );
    }
}

#[test]
fn inspection_checks_copy_integrity_independently_of_target_matching() {
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let path = fixture.revisions()[0].backup.clone().unwrap();
    fs::write(&path, b"damaged").unwrap();
    let result = inspect(&fixture, &id);
    assert_eq!(result[0].current_match, RevisionMatch::Intended);
    assert_eq!(result[0].backup_state, RevisionBackupState::Unavailable);
    fs::remove_file(&path).unwrap();
    assert_eq!(
        inspect(&fixture, &id)[0].backup_state,
        RevisionBackupState::Unavailable
    );
}

#[test]
fn inspection_reports_oversized_and_non_regular_targets_without_hiding_the_copy() {
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let target = fixture.root.join("a");
    fs::OpenOptions::new()
        .write(true)
        .open(&target)
        .unwrap()
        .set_len(MAX_TARGET_BYTES + 1)
        .unwrap();
    let result = inspect(&fixture, &id);
    assert_eq!(result[0].current_match, RevisionMatch::Unreadable);
    assert_eq!(result[0].error_code.as_deref(), Some("target_too_large"));
    assert_eq!(result[0].backup_state, RevisionBackupState::Available);
    fs::remove_file(&target).unwrap();
    fs::create_dir(&target).unwrap();
    assert_eq!(
        inspect(&fixture, &id)[0].current_match,
        RevisionMatch::Unreadable
    );
}

#[cfg(unix)]
#[test]
fn inspection_refuses_target_and_backup_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let target = fixture.root.join("a");
    let outside = fixture.root.join("outside");
    fs::write(&outside, b"new").unwrap();
    fs::remove_file(&target).unwrap();
    symlink(&outside, &target).unwrap();
    let result = inspect(&fixture, &id);
    assert_eq!(result[0].current_match, RevisionMatch::Unreadable);
    assert_eq!(result[0].error_code.as_deref(), Some("symlink_refused"));
    let copy = PathBuf::from(fixture.revisions()[0].backup.clone().unwrap());
    let directory = copy.parent().unwrap();
    let moved = fixture.root.join("moved-copy");
    fs::rename(directory, &moved).unwrap();
    symlink(&moved, directory).unwrap();
    assert_eq!(
        inspect(&fixture, &id)[0].backup_state,
        RevisionBackupState::Unavailable
    );
    assert_eq!(fs::read(outside).unwrap(), b"new");
}

#[test]
fn inspection_keeps_each_file_result_when_a_multi_file_attempt_is_partly_observable() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"old a").unwrap();
    fs::write(fixture.root.join("b"), b"old b").unwrap();
    let applied = fixture
        .apply(
            OperationKind::PiSettings,
            &[fixture.file("a", b"new a"), fixture.file("b", b"new b")],
        )
        .unwrap();
    // Reproduce durable rows whose result commits were lost at interruption.
    fixture.sql("UPDATE config_file_revision SET state = 'prepared';");
    fs::remove_file(fixture.root.join("b")).unwrap();
    fs::create_dir(fixture.root.join("b")).unwrap();
    let results = inspect(&fixture, &applied.operation_id);
    assert_eq!(results.len(), 2);
    let first = results.iter().find(|result| result.ordinal == 0).unwrap();
    let second = results.iter().find(|result| result.ordinal == 1).unwrap();
    assert_eq!(first.current_match, RevisionMatch::Intended);
    assert_eq!(second.current_match, RevisionMatch::Unreadable);
    assert!(
        results
            .iter()
            .all(|result| result.backup_state == RevisionBackupState::Available)
    );
    assert_eq!(first.batch_id, second.batch_id);
}

#[test]
fn inspection_handles_missing_operations_and_ignores_resolved_revisions() {
    let fixture = Fixture::new();
    assert!(
        inspect_interrupted_revisions(&fixture.storage, &"a".repeat(32), &fixture.backups())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        inspect_interrupted_revisions(&fixture.storage, "bad", &fixture.backups()).unwrap_err(),
        WriteError::InvalidRequest
    );
    let applied = fixture
        .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
        .unwrap();
    assert!(inspect(&fixture, &applied.operation_id).is_empty());
}

#[test]
fn inspection_rejects_mismatched_targets_and_invalid_revision_metadata() {
    for change in [
        "UPDATE config_file_revision SET target_path = target_path || '-other'",
        "UPDATE config_file_revision SET id = '../escape'",
        "UPDATE config_file_revision SET id = 'C:escape'",
        "UPDATE config_file_revision SET id = '..\\escape'",
        "UPDATE config_file_revision SET error_code = 'outcome_unknown'",
        "UPDATE config_file_revision SET intent = 'other'",
        "UPDATE config_file_revision SET previous_digest = 'bad'",
    ] {
        let fixture = Fixture::new();
        let id = interrupted_apply(&fixture, true);
        fixture.sql("PRAGMA ignore_check_constraints = ON;");
        fixture.sql(change);
        assert_eq!(
            inspect_interrupted_revisions(&fixture.storage, &id, &fixture.backups()).unwrap_err(),
            WriteError::InvalidStoredOperation
        );
        assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
    }
}

#[test]
fn inspection_never_reads_an_out_of_store_copy_named_in_the_log() {
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let outside = fixture.root.join("outside");
    fs::write(&outside, b"old").unwrap();
    fixture
        .storage
        .lock()
        .unwrap()
        .execute(
            "UPDATE config_file_revision SET backup_path = ?1",
            [outside.to_str().unwrap()],
        )
        .unwrap();
    assert_eq!(
        inspect(&fixture, &id)[0].backup_state,
        RevisionBackupState::Unavailable
    );
}

#[test]
fn inspection_recognizes_interrupted_restore_and_compensation_without_reversing_them() {
    for existed in [false, true] {
        for intent in ["restore", "compensation"] {
            let fixture = Fixture::new();
            if existed {
                fs::write(fixture.root.join("a"), b"old").unwrap();
            }
            let applied = fixture
                .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
                .unwrap();
            fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision WHEN OLD.intent != 'apply' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
            let result = if intent == "restore" {
                fixture.restore(&applied.operation_id).map(|_| ())
            } else {
                finish_operation(
                    &fixture.storage,
                    &applied.operation_id,
                    OperationState::Partial,
                )
                .unwrap();
                compensate_operation(&fixture.storage, &applied.operation_id, &fixture.backups())
                    .map(|_| ())
            };
            assert_eq!(result.unwrap_err(), WriteError::LogWriteFailed);
            let rows = inspect(&fixture, &applied.operation_id);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].intent, intent);
            assert_eq!(rows[0].current_match, RevisionMatch::Intended);
            assert_eq!(rows[0].backup_state, RevisionBackupState::Available);
            assert_eq!(rows[0].intended_digest, existed.then(|| digest_hex(b"old")));
            assert_eq!(fixture.root.join("a").exists(), existed);
        }
    }
}

#[cfg(unix)]
#[test]
fn inspection_does_not_infer_permission_change_from_equal_digests() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"same").unwrap();
    fs::set_permissions(fixture.root.join("a"), fs::Permissions::from_mode(0o644)).unwrap();
    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        apply_private_files(
            &fixture.storage,
            OperationKind::PiSettings,
            None,
            &[fixture.file("a", b"same")],
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::LogWriteFailed
    );
    let id = fixture.revisions()[0].operation.clone();
    assert_eq!(inspect(&fixture, &id)[0].current_match, RevisionMatch::Both);
    assert_eq!(fixture.revisions()[0].state, "prepared");
}

#[test]
fn a_reused_backup_identifier_cannot_overwrite_a_retained_version() {
    let fixture = Fixture::new();
    let backups = fixture.backups();
    let backup = backups.store("retained", "config", b"first").unwrap();
    assert_eq!(
        backups.store("retained", "config", b"second").unwrap_err(),
        WriteError::BackupUnavailable
    );
    assert_eq!(fs::read(&backup.path).unwrap(), b"first");
}

#[cfg(unix)]
#[test]
fn a_failed_replacement_keeps_its_journaled_pre_image() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let directory = fixture.root.join("readonly");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("config"), b"old").unwrap();
    let file = fixture.file("readonly/config", b"new");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o500)).unwrap();
    let result = fixture.apply(OperationKind::GrokSettings, &[file]);
    // Always restore permissions before assertions so fixture cleanup is safe.
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result.unwrap().state, OperationState::Failed);
    assert_eq!(fs::read(directory.join("config")).unwrap(), b"old");
    let rows = fixture.revisions();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, "failed");
    assert_eq!(fs::read(rows[0].backup.as_ref().unwrap()).unwrap(), b"old");
}

#[test]
fn damaged_original_snapshot_does_not_start_a_new_revision() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"old").unwrap();
    let applied = fixture
        .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
        .unwrap();
    fs::write(fixture.revisions()[0].backup.as_ref().unwrap(), b"damaged").unwrap();
    assert_eq!(
        fixture.restore(&applied.operation_id).unwrap_err(),
        WriteError::PlanStale
    );
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
    assert_eq!(fixture.revisions().len(), 1);
}
