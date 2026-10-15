//! Operation-log tests. Every fixture is a temporary directory removed when the
//! test ends; the log rows and the private database are the ones a real apply
//! step would write.
use super::*;
use crate::config_plan::read_target_document;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture {
    root: PathBuf,
    storage: crate::storage::Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-config-operation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let storage = crate::storage::Storage::open_in_directory(&root).unwrap();
        Self { root, storage }
    }

    fn target(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn step(&self, name: &str) -> WriteTarget {
        WriteTarget {
            path: self.target(name),
            expected_digest: None,
        }
    }

    /// Begin one single-step operation and return its identity.
    fn begin_one(&self, name: &str) -> String {
        begin_operation(
            &self.storage,
            OperationKind::ClaudeCodeSettings,
            None,
            &[self.step(name)],
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The private database file, whatever its current name.
fn database_file(root: &Path) -> PathBuf {
    fs::read_dir(root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "sqlite3")
        })
        .expect("database file")
}

/// Whether `haystack` contains `needle` as a run of bytes.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Change one stored row the way a foreign writer would, then read the log back.
///
/// The storage CHECK constraints are turned off for the update so the reader,
/// not SQLite, decides whether the row is usable.
fn rejection_after(fixture: &Fixture, id: &str, statement: &str) -> WriteError {
    {
        let connection = fixture.storage.lock().unwrap();
        connection
            .execute_batch("PRAGMA ignore_check_constraints = ON;")
            .unwrap();
        connection.execute(statement, [id]).unwrap();
    }
    assert_eq!(
        operation(&fixture.storage, id).unwrap_err(),
        WriteError::InvalidStoredOperation,
        "{statement}"
    );
    unfinished_operations(&fixture.storage).unwrap_err()
}

#[test]
fn an_operation_records_every_step_before_the_first_write() {
    let fixture = Fixture::new();
    let steps = [
        fixture.step("settings.json"),
        WriteTarget {
            path: fixture.target("models.json"),
            expected_digest: Some(digest_hex(b"replaced")),
        },
    ];
    let owner = "0123456789abcdef0123456789abcdef";
    let id = begin_operation(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        Some(owner),
        &steps,
    )
    .unwrap();

    let record = operation(&fixture.storage, &id).unwrap().unwrap();
    assert_eq!(record.kind, OperationKind::ClaudeCodeSettings);
    assert_eq!(record.owner_ref.as_deref(), Some(owner));
    assert_eq!(record.state, OperationState::Planned);
    assert!(record.updated_at >= record.created_at);
    assert!(record.created_at > 0);
    let paths: Vec<&str> = record
        .steps
        .iter()
        .map(|step| step.target_path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            steps[0].path.to_str().unwrap(),
            steps[1].path.to_str().unwrap()
        ]
    );
    assert_eq!(record.steps[0].ordinal, 0);
    assert_eq!(record.steps[0].expected_digest, None);
    assert_eq!(record.steps[1].ordinal, 1);
    assert_eq!(
        record.steps[1].expected_digest.as_deref(),
        Some(digest_hex(b"replaced").as_str())
    );
    for step in &record.steps {
        assert_eq!(step.state, StepState::Pending);
        assert_eq!(step.error_code, None);
        assert_eq!(step.backup, None);
        assert_eq!(step.applied_digest, None);
    }
    assert_eq!(
        unfinished_operations(&fixture.storage).unwrap(),
        vec![record]
    );
}

#[test]
fn an_applied_step_keeps_its_backup_reference() {
    let fixture = Fixture::new();
    let id = fixture.begin_one("settings.json");
    let backup = Backup {
        id: "1700000000000-42-0".to_owned(),
        path: fixture
            .target("backups/1700000000000-42-0/settings.json")
            .to_str()
            .unwrap()
            .to_owned(),
        content_digest: digest_hex(b"replaced"),
    };
    let applied = digest_hex(b"applied");
    record_step_applied(&fixture.storage, &id, 0, Some(&backup), &applied).unwrap();

    let record = operation(&fixture.storage, &id).unwrap().unwrap();
    assert_eq!(record.steps[0].state, StepState::Applied);
    assert_eq!(
        record.steps[0].applied_digest.as_deref(),
        Some(applied.as_str())
    );
    assert_eq!(record.steps[0].backup.as_ref(), Some(&backup));
    assert_eq!(record.steps[0].error_code, None);
    // The operation itself is still open until the caller finishes it.
    assert_eq!(record.state, OperationState::Planned);
    assert_eq!(unfinished_operations(&fixture.storage).unwrap().len(), 1);
}

#[test]
fn a_failed_step_carries_its_error_code() {
    let fixture = Fixture::new();
    let id = fixture.begin_one("settings.json");
    record_step_failed(&fixture.storage, &id, 0, WriteError::TargetChanged).unwrap();

    let step = &operation(&fixture.storage, &id).unwrap().unwrap().steps[0];
    assert_eq!(step.state, StepState::Failed);
    assert_eq!(step.error_code.as_deref(), Some("target_changed"));
    assert_eq!(step.applied_digest, None);
    assert_eq!(step.backup, None);
}

#[test]
fn only_unsettled_states_stay_in_the_unfinished_list() {
    let fixture = Fixture::new();
    let mut expected = Vec::new();
    let states = [
        OperationState::Planned,
        OperationState::Applied,
        OperationState::Failed,
        OperationState::Partial,
        OperationState::Unknown,
        OperationState::Compensated,
    ];
    for (index, state) in states.iter().enumerate() {
        let id = fixture.begin_one(&format!("settings-{index}.json"));
        finish_operation(&fixture.storage, &id, *state).unwrap();
        assert_eq!(
            operation(&fixture.storage, &id).unwrap().unwrap().state,
            *state
        );
        if state.is_unfinished() {
            expected.push(id);
        }
    }
    let mut listed: Vec<String> = unfinished_operations(&fixture.storage)
        .unwrap()
        .into_iter()
        .map(|record| record.id)
        .collect();
    expected.sort();
    listed.sort();
    assert_eq!(listed, expected);
}

#[test]
fn a_crash_between_steps_stays_discoverable_after_a_restart() {
    let fixture = Fixture::new();
    let steps = [fixture.step("settings.json"), fixture.step("models.json")];
    let id = begin_operation(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &steps,
    )
    .unwrap();
    record_step_applied(&fixture.storage, &id, 0, None, &digest_hex(b"first")).unwrap();
    // The process stops here: the operation is never finished.

    let reopened = crate::storage::Storage::open_in_directory(&fixture.root).unwrap();
    let listed = unfinished_operations(&reopened).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].state, OperationState::Planned);
    assert_eq!(listed[0].steps[0].state, StepState::Applied);
    assert_eq!(listed[0].steps[1].state, StepState::Pending);
}

#[test]
fn operations_keep_their_own_steps() {
    let fixture = Fixture::new();
    let first = fixture.begin_one("settings.json");
    let second = fixture.begin_one("models.json");
    record_step_applied(&fixture.storage, &first, 0, None, &digest_hex(b"one")).unwrap();

    let listed = unfinished_operations(&fixture.storage).unwrap();
    assert_eq!(listed.len(), 2);
    assert_ne!(first, second);
    let first_record = listed.iter().find(|record| record.id == first).unwrap();
    let second_record = listed.iter().find(|record| record.id == second).unwrap();
    assert_eq!(first_record.steps[0].state, StepState::Applied);
    assert_eq!(second_record.steps[0].state, StepState::Pending);
    assert!(second_record.steps[0].target_path.ends_with("models.json"));
}

#[test]
fn invalid_log_input_is_refused_without_recording_anything() {
    let fixture = Fixture::new();
    let empty: [WriteTarget; 0] = [];
    assert_eq!(
        begin_operation(
            &fixture.storage,
            OperationKind::ClaudeCodeSettings,
            None,
            &empty
        )
        .unwrap_err(),
        WriteError::InvalidRequest
    );
    let short_digest = [WriteTarget {
        path: fixture.target("settings.json"),
        expected_digest: Some("short".to_owned()),
    }];
    assert_eq!(
        begin_operation(
            &fixture.storage,
            OperationKind::ClaudeCodeSettings,
            None,
            &short_digest
        )
        .unwrap_err(),
        WriteError::InvalidRequest
    );
    let relative = [WriteTarget {
        path: PathBuf::from("relative/settings.json"),
        expected_digest: None,
    }];
    assert_eq!(
        begin_operation(
            &fixture.storage,
            OperationKind::ClaudeCodeSettings,
            None,
            &relative
        )
        .unwrap_err(),
        WriteError::TargetPathInvalid
    );
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());

    let id = fixture.begin_one("settings.json");
    assert_eq!(
        record_step_applied(&fixture.storage, &id, 1, None, &digest_hex(b"applied")).unwrap_err(),
        WriteError::InvalidRequest
    );
    assert_eq!(
        record_step_failed(&fixture.storage, &id, 1, WriteError::WriteFailed).unwrap_err(),
        WriteError::InvalidRequest
    );
    assert_eq!(
        record_step_applied(&fixture.storage, &id, 0, None, "not-a-digest").unwrap_err(),
        WriteError::InvalidRequest
    );
    let missing = "0".repeat(32);
    assert_eq!(
        finish_operation(&fixture.storage, &missing, OperationState::Applied).unwrap_err(),
        WriteError::InvalidRequest
    );
    assert_eq!(
        record_step_applied(&fixture.storage, &missing, 0, None, &digest_hex(b"applied"))
            .unwrap_err(),
        WriteError::InvalidRequest
    );
    assert_eq!(
        operation(&fixture.storage, "not-an-identity").unwrap_err(),
        WriteError::InvalidRequest
    );
    // A well-formed identity that is simply absent is not an error.
    assert_eq!(operation(&fixture.storage, &missing).unwrap(), None);
    // The one real step is still pending after every refusal.
    assert_eq!(
        operation(&fixture.storage, &id).unwrap().unwrap().steps[0].state,
        StepState::Pending
    );
}

#[test]
fn a_malformed_row_is_rejected_instead_of_displayed() {
    let cases = [
        "UPDATE config_operation SET kind = 'pdf_export' WHERE id = ?1",
        "UPDATE config_operation SET state = 'running' WHERE id = ?1",
        "UPDATE config_operation SET created_at = -1 WHERE id = ?1",
        "UPDATE config_operation SET updated_at = 0, created_at = 5 WHERE id = ?1",
        "UPDATE config_operation_step SET target_path = 'relative/settings.json' WHERE operation_id = ?1",
        "UPDATE config_operation_step SET expected_digest = 'not-a-digest' WHERE operation_id = ?1",
        "UPDATE config_operation_step SET ordinal = 7 WHERE operation_id = ?1",
        "UPDATE config_operation_step SET state = 'failed' WHERE operation_id = ?1",
        "UPDATE config_operation_step SET applied_digest = replace(hex(randomblob(32)), 'x', 'a') WHERE operation_id = ?1",
        "UPDATE config_operation_step SET backup_id = 'x', backup_path = '/tmp/x' WHERE operation_id = ?1",
        "UPDATE config_operation_step SET error_code = 'WRONG' WHERE operation_id = ?1",
        "UPDATE config_operation_step SET restore_state = 'half-done' WHERE operation_id = ?1",
        "UPDATE config_operation SET owner_ref = char(1) WHERE id = ?1",
        "DELETE FROM config_operation_step WHERE operation_id = ?1",
    ];
    for statement in cases {
        let fixture = Fixture::new();
        let id = fixture.begin_one("settings.json");
        assert_eq!(
            rejection_after(&fixture, &id, statement),
            WriteError::InvalidStoredOperation,
            "{statement}"
        );
    }
}

#[test]
fn the_log_never_stores_file_content() {
    let fixture = Fixture::new();
    let secret = "synthetic-secret";
    let replaced = format!("{{\"env\":{{\"ANTHROPIC_AUTH_TOKEN\":\"{secret}\"}}}}\n");
    let target = fixture.target("settings.json");
    fs::write(&target, &replaced).unwrap();

    // A real apply step: the replaced bytes go to the private backup directory.
    let plan = WriteTarget::from_plan(&read_target_document(&target).unwrap().target);
    let backup_root = fixture.root.join("backups");
    let written = apply_file(&plan, b"{\"env\":{}}", &BackupStore::new(backup_root)).unwrap();
    let id = begin_operation(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        std::slice::from_ref(&plan),
    )
    .unwrap();
    record_step_applied(
        &fixture.storage,
        &id,
        0,
        written.backup.as_ref(),
        written.target.content_digest.as_deref().unwrap(),
    )
    .unwrap();

    let backup = written.backup.unwrap();
    assert!(fs::read_to_string(&backup.path).unwrap().contains(secret));
    let database = fs::read(database_file(&fixture.root)).unwrap();
    assert!(!contains(&database, secret.as_bytes()));
    assert!(!contains(&database, b"ANTHROPIC_AUTH_TOKEN"));
}

#[test]
fn an_owner_reference_of_the_wrong_shape_is_refused() {
    let fixture = Fixture::new();
    let step = [fixture.step("settings.json")];
    for owner in ["", "line\nbreak", &"x".repeat(65)] {
        assert_eq!(
            begin_operation(
                &fixture.storage,
                OperationKind::ClaudeCodeSettings,
                Some(owner),
                &step
            )
            .unwrap_err(),
            WriteError::InvalidRequest,
            "{owner:?}"
        );
    }
    // Nothing was recorded for a refused reference.
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
}

#[test]
fn a_rollback_result_is_recorded_on_an_applied_step_only() {
    let fixture = Fixture::new();
    let id = fixture.begin_one("settings.json");
    // Nothing was written yet, so there is no rollback to record.
    assert_eq!(
        record_step_restored(&fixture.storage, &id, 0, RestoreState::Restored).unwrap_err(),
        WriteError::InvalidRequest
    );
    assert_eq!(
        record_step_restored(&fixture.storage, &id, 3, RestoreState::Restored).unwrap_err(),
        WriteError::InvalidRequest
    );
    record_step_applied(&fixture.storage, &id, 0, None, &digest_hex(b"applied")).unwrap();
    record_step_restored(&fixture.storage, &id, 0, RestoreState::Restored).unwrap();
    let record = operation(&fixture.storage, &id).unwrap().unwrap();
    assert_eq!(record.steps[0].state, StepState::Applied);
    assert_eq!(record.steps[0].restore_state, Some(RestoreState::Restored));
    assert!(record.updated_at >= record.created_at);
    // A later attempt replaces the earlier result: the row holds the latest
    // state, not a history of attempts.
    record_step_restored(&fixture.storage, &id, 0, RestoreState::Conflict).unwrap();
    let record = operation(&fixture.storage, &id).unwrap().unwrap();
    assert_eq!(record.steps[0].restore_state, Some(RestoreState::Conflict));
}

#[test]
fn a_recent_listing_is_newest_first_bounded_and_kind_filtered() {
    let fixture = Fixture::new();
    let older = fixture.begin_one("older.json");
    let newer = fixture.begin_one("newer.json");
    let foreign = "d".repeat(32);
    {
        // Known timestamps make the order the query's, not the clock's; the
        // second kind stands for a feature that reuses the log later.
        let connection = fixture.storage.lock().unwrap();
        connection
            .execute(
                "UPDATE config_operation SET created_at = 100, updated_at = 100 WHERE id = ?1",
                [&older],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE config_operation SET created_at = 200, updated_at = 200 WHERE id = ?1",
                [&newer],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO config_operation (id, kind, state, created_at, updated_at)
                 VALUES (?1, 'mcp_definition', 'planned', 300, 300)",
                [&foreign],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO config_operation_step
                     (operation_id, ordinal, target_path, state, updated_at)
                 VALUES (?1, 0, '/tmp/mcp.json', 'pending', 300)",
                [&foreign],
            )
            .unwrap();
    }
    let listed =
        recent_operations(&fixture.storage, OperationKind::ClaudeCodeSettings, 10).unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [newer.as_str(), older.as_str()]
    );
    assert_eq!(
        recent_operations(&fixture.storage, OperationKind::ClaudeCodeSettings, 1)
            .unwrap()
            .len(),
        1
    );
    // Zero is clamped to the newest single operation, not read as "none wanted".
    assert_eq!(
        recent_operations(&fixture.storage, OperationKind::ClaudeCodeSettings, 0)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_malformed_rollback_result_hides_the_operation_from_a_listing() {
    let fixture = Fixture::new();
    let id = fixture.begin_one("settings.json");
    record_step_applied(&fixture.storage, &id, 0, None, &digest_hex(b"applied")).unwrap();
    {
        let connection = fixture.storage.lock().unwrap();
        connection
            .execute_batch("PRAGMA ignore_check_constraints = ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE config_operation_step SET restore_state = 'half-done' WHERE operation_id = ?1",
                [&id],
            )
            .unwrap();
    }
    assert_eq!(
        recent_operations(&fixture.storage, OperationKind::ClaudeCodeSettings, 10).unwrap_err(),
        WriteError::InvalidStoredOperation
    );
}

#[test]
fn every_error_code_matches_its_serialized_form() {
    // Keep this list in step with the enum; `code()` is exhaustive, so a new
    // variant fails to compile until it names its code.
    let errors = [
        WriteError::InvalidRequest,
        WriteError::TargetChanged,
        WriteError::SymlinkRefused,
        WriteError::TargetReadOnly,
        WriteError::TargetPathInvalid,
        WriteError::TargetUnreadable,
        WriteError::TargetTooLarge,
        WriteError::BackupUnavailable,
        WriteError::WriteFailed,
        WriteError::OutcomeUnknown,
        WriteError::StorageUnavailable,
        WriteError::LogWriteFailed,
        WriteError::LogReadFailed,
        WriteError::InvalidStoredOperation,
    ];
    for error in errors {
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!(error.code())
        );
    }
}
