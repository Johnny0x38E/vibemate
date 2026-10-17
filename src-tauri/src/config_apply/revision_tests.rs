//! Integration tests for pre-mutation snapshots using temporary files and the
//! real SQLite log. Fault injection uses local triggers, never real credentials.

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
    assert_eq!(fs::read(&backup.path).unwrap(), b"old");
    assert_eq!(
        require_known_revisions(&fixture.storage, &source),
        Err(WriteError::OutcomeUnknown)
    );
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
