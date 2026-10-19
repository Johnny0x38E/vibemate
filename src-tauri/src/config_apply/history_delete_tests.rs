//! History deletion tests: real SQLite, private snapshots and temp files.
//!
//! Synthetic configuration only. No real Agent file or credential is touched.

use super::*;
use crate::config_apply::{
    HistoryDeleteState, HistoryScope, OperationKind, WriteError, apply_version_restore,
    configuration_history, delete_history_entry, plan_version_restore,
};
use std::fs;

fn scope(fixture: &Fixture, kind: OperationKind, names: &[&str]) -> HistoryScope {
    HistoryScope::new(kind, &fixture.root, names).expect("scope")
}

fn entry(operation_id: &str) -> String {
    format!("operation:{operation_id}")
}

fn review(
    plan: &crate::config_apply::VersionRestorePlan,
) -> Vec<crate::config_apply::ReviewedVersionFile> {
    plan.files
        .iter()
        .filter(|file| {
            matches!(
                file.action,
                crate::config_apply::VersionRestoreAction::Restore
                    | crate::config_apply::VersionRestoreAction::Remove
            )
        })
        .map(|file| crate::config_apply::ReviewedVersionFile {
            revision_id: file.revision_id.clone().expect("revision"),
            current_digest: file.current_digest.clone(),
        })
        .collect()
}

#[test]
fn deleting_an_apply_entry_removes_its_private_backup_and_leaves_config_untouched() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let original = b"synthetic-original\n";
    fs::write(fixture.root.join("a"), original).unwrap();
    let first = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    let second = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-c")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let before = fs::read(fixture.root.join("a")).unwrap();
    let first_revision = fixture
        .revisions()
        .into_iter()
        .find(|row| row.operation == first.operation_id && row.intent == "apply")
        .expect("first apply revision");
    let backup_dir = fixture.backups().root().join(&first_revision.id);
    assert!(backup_dir.is_dir());

    let outcome = delete_history_entry(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .expect("delete");
    assert_eq!(outcome.state, HistoryDeleteState::Completed);
    assert!(outcome.leftovers.is_empty());
    assert!(!backup_dir.exists());
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), before);
    assert!(
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups())
            .unwrap()
            .entries
            .iter()
            .all(|item| item.entry_key != entry(&first.operation_id))
    );
    // The later entry keeps its own snapshot and remains restorable.
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&second.operation_id),
        &fixture.backups(),
    )
    .expect("later preview");
    assert!(plan.restorable);
    apply_version_restore(
        &fixture.storage,
        &scope,
        &entry(&second.operation_id),
        &review(&plan),
        &fixture.backups(),
    )
    .expect("restore later");
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"synthetic-b");
}

#[test]
fn deleting_a_batch_leaves_the_source_operation_and_its_snapshot() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    fs::write(fixture.root.join("a"), b"synthetic-a").unwrap();
    let first = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"synthetic-c")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    apply_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &review(&plan),
        &fixture.backups(),
    )
    .unwrap();
    let batch = fixture
        .revisions()
        .into_iter()
        .find(|row| row.intent == "restore")
        .expect("restore batch");
    let batch_key = format!("batch:{}", batch.batch);
    let batch_dir = fixture.backups().root().join(&batch.id);
    assert!(batch_dir.is_dir());

    let outcome =
        delete_history_entry(&fixture.storage, &scope, &batch_key, &fixture.backups()).unwrap();
    assert_eq!(outcome.state, HistoryDeleteState::Completed);
    assert!(!batch_dir.exists());
    assert!(
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups())
            .unwrap()
            .entries
            .iter()
            .any(|item| item.entry_key == entry(&first.operation_id))
    );
    let apply_dir = fixture.backups().root().join(
        &fixture
            .revisions()
            .into_iter()
            .find(|row| row.operation == first.operation_id && row.intent == "apply")
            .expect("apply revision")
            .id,
    );
    assert!(apply_dir.is_dir());
}

#[test]
fn unfinished_and_unresolved_entries_refuse_deletion() {
    let fixture = Fixture::new();
    let kind = OperationKind::GrokSettings;
    fs::write(fixture.root.join("a"), b"synthetic-a").unwrap();
    let applied = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    fixture.sql(&format!(
        "UPDATE config_operation SET state = 'partial' WHERE id = '{}';",
        applied.operation_id
    ));
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &scope,
            &entry(&applied.operation_id),
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::HistoryActive
    );
    fixture.sql(&format!(
        "UPDATE config_operation SET state = 'applied' WHERE id = '{}';
         UPDATE config_file_revision SET state = 'prepared', error_code = NULL
         WHERE operation_id = '{}';",
        applied.operation_id, applied.operation_id
    ));
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &scope,
            &entry(&applied.operation_id),
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::HistoryUnresolved
    );
}

#[test]
fn operation_with_dependent_batches_refuses_until_batches_are_gone() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"synthetic-a").unwrap();
    let first = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"synthetic-c")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    apply_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &review(&plan),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &scope,
            &entry(&first.operation_id),
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::HistoryReferenced
    );
    let batch = fixture
        .revisions()
        .into_iter()
        .find(|row| row.intent == "restore")
        .unwrap();
    delete_history_entry(
        &fixture.storage,
        &scope,
        &format!("batch:{}", batch.batch),
        &fixture.backups(),
    )
    .unwrap();
    let outcome = delete_history_entry(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(outcome.state, HistoryDeleteState::Completed);
}

#[test]
fn shared_step_and_revision_refs_are_removed_together_once() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"synthetic-a").unwrap();
    let applied = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let revision = fixture
        .revisions()
        .into_iter()
        .find(|row| row.operation == applied.operation_id)
        .unwrap();
    // Modern applies store the step backup under the revision id.
    assert_eq!(
        operation(&fixture.storage, &applied.operation_id)
            .unwrap()
            .unwrap()
            .steps[0]
            .backup
            .as_ref()
            .unwrap()
            .id,
        revision.id
    );
    let dir = fixture.backups().root().join(&revision.id);
    assert!(dir.is_dir());
    delete_history_entry(
        &fixture.storage,
        &scope,
        &entry(&applied.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    assert!(!dir.exists());
}

#[test]
fn interrupted_residue_retries_idempotently_without_hiding_the_row_first() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"synthetic-a").unwrap();
    let applied = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let revision = fixture
        .revisions()
        .into_iter()
        .find(|row| row.operation == applied.operation_id)
        .unwrap();
    let dir = fixture.backups().root().join(&revision.id);
    // Simulate a prepared claim that already listed the exclusive backup, then
    // a permission failure leaving the directory behind.
    let delete_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let now = 1_700_000_000_000i64;
    fixture.sql(&format!(
        "INSERT INTO config_history_delete
         (id, entry_key, operation_id, scope_kind, state, leftovers, error_code, created_at, updated_at)
         VALUES ('{delete_id}', 'operation:{}', '{}', 'claude_code_settings', 'failed',
                 '[{{}}]', 'backup_unavailable', {now}, {now});
         INSERT INTO config_history_delete_backup (delete_id, backup_id, backup_dir)
         VALUES ('{delete_id}', '{}', '{}');",
        applied.operation_id,
        applied.operation_id,
        revision.id,
        dir.to_string_lossy().replace('\'', "''"),
    ));
    assert!(
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups())
            .unwrap()
            .entries
            .iter()
            .any(|item| item.entry_key == entry(&applied.operation_id)),
        "failed deletes must not hide the history row while copies remain"
    );
    let outcome = delete_history_entry(
        &fixture.storage,
        &scope,
        &entry(&applied.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(outcome.state, HistoryDeleteState::Completed);
    assert!(!dir.exists());
    assert!(
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups())
            .unwrap()
            .entries
            .iter()
            .all(|item| item.entry_key != entry(&applied.operation_id))
    );
    // A second call is a no-op success.
    let again = delete_history_entry(
        &fixture.storage,
        &scope,
        &entry(&applied.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(again.state, HistoryDeleteState::Completed);
}

#[test]
fn cross_root_entries_are_refused_and_credentials_bindings_stay() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let other = fixture.root.join("other-root");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("a"), b"synthetic-a").unwrap();
    let target = read_target_bytes(&other.join("a")).unwrap().0;
    let applied = apply_files(
        &fixture.storage,
        kind,
        None,
        &[PlannedFile {
            target: WriteTarget::from_plan(&target),
            content: b"synthetic-b".to_vec(),
        }],
        &fixture.backups(),
    )
    .unwrap();
    let wrong_scope = scope(&fixture, kind, &["a"]);
    assert_eq!(
        delete_history_entry(
            &fixture.storage,
            &wrong_scope,
            &entry(&applied.operation_id),
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::InvalidStoredOperation
    );
    // Seed alias ownership rows; deletion must not touch them.
    fixture.sql(
        "INSERT INTO pi_provider_binding(config_root, provider_key, provider_id)
         VALUES ('/tmp/pi', 'vibemate-OpenRouter', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb');
         INSERT INTO grok_model_binding(config_root, model_alias, provider_id, model_id)
         VALUES ('/tmp/grok', 'owned', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'model-1');",
    );
    let right_scope = HistoryScope::new(kind, &other, &["a"]).unwrap();
    delete_history_entry(
        &fixture.storage,
        &right_scope,
        &entry(&applied.operation_id),
        &fixture.backups(),
    )
    .unwrap();
    let connection = fixture.storage.lock().unwrap();
    let pi: i64 = connection
        .query_row("SELECT COUNT(*) FROM pi_provider_binding", [], |row| {
            row.get(0)
        })
        .unwrap();
    let grok: i64 = connection
        .query_row("SELECT COUNT(*) FROM grok_model_binding", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(pi, 1);
    assert_eq!(grok, 1);
    assert_eq!(fs::read(other.join("a")).unwrap(), b"synthetic-b");
}
