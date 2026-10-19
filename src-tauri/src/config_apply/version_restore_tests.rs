//! Direct version-restore tests: real SQLite, private snapshots and temp files.
//!
//! Every test uses synthetic configuration content. No real Agent file or
//! credential is read, and no credential value is involved.

use super::*;
use crate::config_apply::{
    HistoryIntent, HistoryScope, OperationKind, OperationState, RestoreState, ReviewedVersionFile,
    VersionRestoreAction, WriteError, apply_files, apply_version_restore, configuration_history,
    operation, operation_requires_resolution, plan_revision_resolution, plan_version_restore,
    resolve_interrupted_revisions,
};
use crate::config_plan::digest_hex;
use std::fs;

fn scope(fixture: &Fixture, kind: OperationKind, names: &[&str]) -> HistoryScope {
    HistoryScope::new(kind, &fixture.root, names).expect("scope")
}

fn entry(operation_id: &str) -> String {
    format!("operation:{operation_id}")
}

fn operation_revision(fixture: &Fixture, operation_id: &str) -> Revision {
    fixture
        .revisions()
        .into_iter()
        .find(|row| row.operation == operation_id && row.intent == "apply")
        .expect("apply revision")
}

fn review(plan: &crate::config_apply::VersionRestorePlan) -> Vec<ReviewedVersionFile> {
    plan.files
        .iter()
        .filter(|file| {
            matches!(
                file.action,
                VersionRestoreAction::Restore | VersionRestoreAction::Remove
            )
        })
        .map(|file| ReviewedVersionFile {
            revision_id: file.revision_id.clone().expect("revision identity"),
            current_digest: file.current_digest.clone(),
        })
        .collect()
}

#[test]
fn restoring_an_apply_entry_returns_the_original_and_the_restore_can_be_undone() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let original = b"# original\nkey = \"synthetic-original\"\n";
    fs::write(fixture.root.join("a"), original).unwrap();
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
    .expect("preview");
    assert!(plan.restorable);
    assert_eq!(plan.operation_id, first.operation_id);
    assert_eq!(plan.files.len(), 1);
    let file = &plan.files[0];
    assert_eq!(file.action, VersionRestoreAction::Restore);
    assert_eq!(
        file.target_digest.as_deref(),
        Some(digest_hex(original).as_str())
    );
    assert_eq!(
        file.current_digest.as_deref(),
        Some(digest_hex(b"synthetic-c").as_str())
    );
    assert_eq!(file.target_path, fixture.root.join("a").to_string_lossy());

    let outcome = apply_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &review(&plan),
        &fixture.backups(),
    )
    .expect("restore");
    assert_eq!(outcome.files.len(), 1);
    assert_eq!(outcome.files[0].state, RestoreState::Restored);
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), original);

    // The restore keeps an independent copy of what it replaced, under a new
    // batch entry rather than editing the original apply.
    let rows = fixture.revisions();
    assert_eq!(rows.len(), 3);
    let undo = rows
        .iter()
        .find(|row| row.intent == "restore")
        .expect("restore revision");
    assert_eq!(undo.operation, first.operation_id);
    assert_eq!(
        undo.before.as_deref(),
        Some(digest_hex(b"synthetic-c").as_str())
    );
    assert_eq!(undo.after.as_deref(), Some(digest_hex(original).as_str()));
    assert_eq!(
        fs::read(undo.backup.as_ref().expect("undo snapshot")).unwrap(),
        b"synthetic-c"
    );

    let undo_entry = format!("batch:{}", undo.batch);
    let undo_plan = plan_version_restore(&fixture.storage, &scope, &undo_entry, &fixture.backups())
        .expect("undo preview");
    assert!(undo_plan.restorable);
    assert_eq!(
        undo_plan.intent,
        crate::config_apply::HistoryIntent::VersionRestore
    );
    assert_eq!(
        undo_plan.files[0].target_digest.as_deref(),
        Some(digest_hex(b"synthetic-c").as_str())
    );
    let undone = apply_version_restore(
        &fixture.storage,
        &scope,
        &undo_entry,
        &review(&undo_plan),
        &fixture.backups(),
    )
    .expect("undo restore");
    assert_eq!(undone.files[0].state, RestoreState::Restored);
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"synthetic-c");
}

#[test]
fn restoring_the_first_entry_removes_a_file_that_did_not_exist_and_can_recreate_it() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    let applied = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-b")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&applied.operation_id),
        &fixture.backups(),
    )
    .expect("preview");
    assert_eq!(plan.files[0].action, VersionRestoreAction::Remove);
    assert_eq!(plan.files[0].target_digest, None);
    let outcome = apply_version_restore(
        &fixture.storage,
        &scope,
        &entry(&applied.operation_id),
        &review(&plan),
        &fixture.backups(),
    )
    .expect("remove");
    assert_eq!(outcome.files[0].state, RestoreState::Restored);
    assert!(!fixture.root.join("a").exists());

    let removal = fixture
        .revisions()
        .into_iter()
        .find(|row| row.intent == "restore")
        .expect("removal revision");
    assert_eq!(removal.after, None);
    assert_eq!(
        fs::read(removal.backup.as_ref().expect("removed bytes")).unwrap(),
        b"synthetic-b"
    );
    let undo_entry = format!("batch:{}", removal.batch);
    let undo_plan = plan_version_restore(&fixture.storage, &scope, &undo_entry, &fixture.backups())
        .expect("undo preview");
    assert_eq!(undo_plan.files[0].action, VersionRestoreAction::Restore);
    apply_version_restore(
        &fixture.storage,
        &scope,
        &undo_entry,
        &review(&undo_plan),
        &fixture.backups(),
    )
    .expect("recreate");
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"synthetic-b");
}

#[test]
fn a_missing_or_damaged_snapshot_blocks_the_whole_entry() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original-a").unwrap();
    fs::write(fixture.root.join("b"), b"original-b").unwrap();
    let first = fixture
        .apply(
            kind,
            &[fixture.file("a", b"first-a"), fixture.file("b", b"first-b")],
        )
        .unwrap();
    fixture
        .apply(
            kind,
            &[
                fixture.file("a", b"second-a"),
                fixture.file("b", b"second-b"),
            ],
        )
        .unwrap();
    let scope = scope(&fixture, kind, &["a", "b"]);
    let row = operation_revision(&fixture, &first.operation_id);
    let broken = row.backup.expect("snapshot");
    fs::remove_file(&broken).unwrap();

    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .expect("preview");
    assert!(!plan.restorable);
    assert!(plan.files.iter().any(|file| {
        file.action == VersionRestoreAction::Blocked
            && file.reason.as_deref() == Some(WriteError::BackupUnavailable.code())
    }));
    assert_eq!(
        apply_version_restore(
            &fixture.storage,
            &scope,
            &entry(&first.operation_id),
            &[],
            &fixture.backups()
        ),
        Err(WriteError::PlanStale)
    );
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"second-a");
    assert_eq!(fs::read(fixture.root.join("b")).unwrap(), b"second-b");
}

#[test]
fn a_corrupted_snapshot_is_reported_as_unavailable() {
    let fixture = Fixture::new();
    let kind = OperationKind::GrokSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let row = operation_revision(&fixture, &first.operation_id);
    fs::write(row.backup.expect("snapshot"), b"damaged").unwrap();
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .expect("preview");
    assert!(!plan.restorable);
    assert_eq!(
        plan.files[0].reason.as_deref(),
        Some(WriteError::BackupUnavailable.code())
    );
}

#[test]
fn a_changed_file_after_preview_is_refused_without_writing() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let entry_key = entry(&first.operation_id);
    let plan = plan_version_restore(&fixture.storage, &scope, &entry_key, &fixture.backups())
        .expect("preview");
    fs::write(fixture.root.join("a"), b"external-change").unwrap();
    assert_eq!(
        apply_version_restore(
            &fixture.storage,
            &scope,
            &entry_key,
            &review(&plan),
            &fixture.backups()
        ),
        Err(WriteError::PlanStale)
    );
    assert_eq!(
        fs::read(fixture.root.join("a")).unwrap(),
        b"external-change"
    );
}

#[test]
fn an_interrupted_operation_requires_inspection_before_version_restore() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let row = operation_revision(&fixture, &first.operation_id);
    fixture.sql(&format!(
        "UPDATE config_file_revision SET state = 'prepared' WHERE id = '{}'",
        row.id
    ));
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .expect("preview");
    assert!(!plan.restorable);
    assert_eq!(plan.reason.as_deref(), Some("history_unresolved"));
    assert!(plan.files.is_empty());
    assert_eq!(
        apply_version_restore(
            &fixture.storage,
            &scope,
            &entry(&first.operation_id),
            &[],
            &fixture.backups()
        ),
        Err(WriteError::PlanStale)
    );
}

#[test]
fn legacy_entries_without_retained_bytes_block_only_when_the_file_changed() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    // Simulate an older log that predates retained snapshots.
    fixture.sql(&format!(
        "DELETE FROM config_file_revision WHERE operation_id = '{}'",
        first.operation_id
    ));
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .expect("preview");
    assert!(!plan.restorable);
    assert_eq!(plan.files[0].action, VersionRestoreAction::Blocked);
    assert_eq!(
        plan.files[0].reason.as_deref(),
        Some("snapshot_unavailable")
    );
    assert_eq!(
        plan.files[0].current_digest.as_deref(),
        Some(digest_hex(b"first").as_str())
    );

    // When the file still holds the pre-entry bytes, no retained copy is
    // needed: the plan reports that the entry is already current.
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&first.operation_id),
        &fixture.backups(),
    )
    .expect("preview at original");
    assert!(!plan.restorable);
    assert_eq!(plan.files[0].action, VersionRestoreAction::Skip);
    // Already-current files still have a real target side. The IPC contract
    // and full-text reader must not confuse a skipped present file with absence.
    assert_eq!(plan.files[0].target_digest, plan.files[0].current_digest);
    assert_eq!(
        plan.files[0].target_digest.as_deref(),
        Some(digest_hex(b"original").as_str())
    );
}

#[test]
fn entries_from_another_agent_root_or_file_scope_are_refused() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    let entry_key = entry(&first.operation_id);
    let backups = fixture.backups();

    let wrong_kind = scope(&fixture, OperationKind::PiSettings, &["a"]);
    assert_eq!(
        plan_version_restore(&fixture.storage, &wrong_kind, &entry_key, &backups),
        Err(WriteError::InvalidStoredOperation)
    );
    let wrong_root = HistoryScope::new(kind, &fixture.root.join("elsewhere"), &["a"]).unwrap();
    assert_eq!(
        plan_version_restore(&fixture.storage, &wrong_root, &entry_key, &backups),
        Err(WriteError::InvalidStoredOperation)
    );
    let wrong_file = scope(&fixture, kind, &["b"]);
    assert_eq!(
        plan_version_restore(&fixture.storage, &wrong_file, &entry_key, &backups),
        Err(WriteError::InvalidStoredOperation)
    );
    for invalid in ["operation:short", "batch:../../outside", "other:id", ""] {
        assert_eq!(
            plan_version_restore(
                &fixture.storage,
                &scope(&fixture, kind, &["a"]),
                invalid,
                &backups
            ),
            Err(WriteError::InvalidRequest)
        );
    }
}

#[test]
fn the_restored_content_names_the_provider_whose_write_produced_it() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = apply_files(
        &fixture.storage,
        kind,
        Some("provider-one"),
        &[fixture.file("a", b"synthetic-first")],
        &fixture.backups(),
    )
    .unwrap();
    let second = fixture
        .apply(kind, &[fixture.file("a", b"synthetic-second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let plan = plan_version_restore(
        &fixture.storage,
        &scope,
        &entry(&second.operation_id),
        &fixture.backups(),
    )
    .expect("preview");
    assert_eq!(
        plan.files[0].restored_owner_ref.as_deref(),
        Some("provider-one")
    );
    assert_eq!(
        operation_revision(&fixture, &first.operation_id)
            .before
            .as_deref(),
        Some(digest_hex(b"original").as_str())
    );
}

#[test]
fn a_direct_restore_is_listed_as_a_version_restore_without_requiring_resolution() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let entry_key = entry(&first.operation_id);
    let plan = plan_version_restore(&fixture.storage, &scope, &entry_key, &fixture.backups())
        .expect("preview");
    apply_version_restore(
        &fixture.storage,
        &scope,
        &entry_key,
        &review(&plan),
        &fixture.backups(),
    )
    .expect("restore");

    let entries = configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups())
        .expect("history")
        .entries;
    let restored = entries
        .iter()
        .find(|entry| entry.intent == HistoryIntent::VersionRestore)
        .expect("version restore entry");
    assert_eq!(restored.operation_id, first.operation_id);
    assert_eq!(restored.revisions.len(), 1);
    assert!(
        entries
            .iter()
            .any(|entry| { entry.intent == HistoryIntent::Apply && entry.entry_key == entry_key })
    );
    // A completed direct restore is not a missing legacy step result.
    assert!(!operation_requires_resolution(&fixture.storage, &first.operation_id).unwrap());
}

#[test]
fn an_interrupted_direct_restore_is_resolved_without_repairing_legacy_step_results() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let entry_key = entry(&first.operation_id);
    let plan = plan_version_restore(&fixture.storage, &scope, &entry_key, &fixture.backups())
        .expect("preview");
    apply_version_restore(
        &fixture.storage,
        &scope,
        &entry_key,
        &review(&plan),
        &fixture.backups(),
    )
    .expect("restore");
    let direct = fixture
        .revisions()
        .into_iter()
        .find(|row| row.intent == "restore")
        .expect("direct revision");
    fixture.sql(&format!(
        "UPDATE config_file_revision SET state = 'prepared' WHERE id = '{}'",
        direct.id
    ));
    assert!(operation_requires_resolution(&fixture.storage, &first.operation_id).unwrap());
    let step_before = operation(&fixture.storage, &first.operation_id)
        .unwrap()
        .unwrap()
        .steps[0]
        .restore_state;
    let review =
        plan_revision_resolution(&fixture.storage, &first.operation_id, &fixture.backups())
            .unwrap()
            .expect("review");
    assert!(review.resolvable);
    assert_eq!(review.revisions.len(), 1);
    assert_eq!(review.revisions[0].intent, "version_restore");
    let state = resolve_interrupted_revisions(
        &fixture.storage,
        &first.operation_id,
        &review,
        &fixture.backups(),
    )
    .expect("resolve");
    assert_eq!(state, OperationState::Applied);
    let after = operation(&fixture.storage, &first.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(after.steps[0].restore_state, step_before);
    assert!(!operation_requires_resolution(&fixture.storage, &first.operation_id).unwrap());
}

#[cfg(unix)]
#[test]
fn a_read_only_target_or_symlink_snapshot_blocks_the_entry() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::write(fixture.root.join("a"), b"original").unwrap();
    let first = fixture.apply(kind, &[fixture.file("a", b"first")]).unwrap();
    fixture
        .apply(kind, &[fixture.file("a", b"second")])
        .unwrap();
    let scope = scope(&fixture, kind, &["a"]);
    let entry_key = entry(&first.operation_id);

    fs::set_permissions(fixture.root.join("a"), fs::Permissions::from_mode(0o444)).unwrap();
    let plan = plan_version_restore(&fixture.storage, &scope, &entry_key, &fixture.backups())
        .expect("read-only preview");
    assert_eq!(
        plan.files[0].reason.as_deref(),
        Some(WriteError::TargetReadOnly.code())
    );

    fs::set_permissions(fixture.root.join("a"), fs::Permissions::from_mode(0o644)).unwrap();
    let row = operation_revision(&fixture, &first.operation_id);
    let snapshot = row.backup.expect("snapshot");
    let linked = fixture.root.join("linked-snapshot");
    fs::write(&linked, b"original").unwrap();
    fs::remove_file(&snapshot).unwrap();
    symlink(&linked, &snapshot).unwrap();
    let plan = plan_version_restore(&fixture.storage, &scope, &entry_key, &fixture.backups())
        .expect("symlink preview");
    assert_eq!(
        plan.files[0].reason.as_deref(),
        Some(WriteError::BackupUnavailable.code())
    );
}

#[test]
fn current_text_reads_only_scope_files_and_masks_credentials() {
    let fixture = Fixture::new();
    let path = fixture.root.join("config.toml");
    fs::write(
        &path,
        "[model.owned]\napi_key = \"synthetic-secret\"\nname = \"keep\"\n",
    )
    .unwrap();
    let scope = scope(&fixture, OperationKind::GrokSettings, &["config.toml"]);
    let text = read_current_text(&scope, path.to_str().unwrap()).expect("current text");
    assert_eq!(text.target_path, path.to_str().unwrap());
    let VersionTextSide::Present { text: body, bytes } = text.current else {
        panic!("expected present text");
    };
    assert!(body.contains("name = \"keep\""));
    assert!(!body.contains("synthetic-secret"));
    assert!(bytes > 0);

    // A path outside the adapter's fixed scope is refused, not read.
    assert_eq!(
        read_current_text(&scope, "/etc/passwd"),
        Err(WriteError::InvalidRequest)
    );

    // A missing file is an absent side, not a stale-plan or raw read error.
    fs::remove_file(&path).unwrap();
    let text = read_current_text(&scope, path.to_str().unwrap()).expect("absent text");
    assert_eq!(text.current, VersionTextSide::Absent);
}
