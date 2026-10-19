//! Independent configuration-version lifecycle tests with real private copies.

use super::*;
use crate::config_apply::{
    HistoryScope, ReviewedVersionFile, VersionRestoreAction, apply_written_version,
    configuration_versions, delete_configuration_version, materialize_configuration_versions,
    plan_written_version,
};

fn key(id: &str) -> String {
    format!("operation:{id}")
}

fn select(fixture: &Fixture, scope: &HistoryScope, id: &str) {
    let plan = plan_written_version(&fixture.storage, scope, &key(id), &fixture.backups()).unwrap();
    assert!(plan.restorable);
    let reviewed: Vec<_> = plan
        .files
        .iter()
        .filter(|file| {
            matches!(
                file.action,
                VersionRestoreAction::Restore | VersionRestoreAction::Remove
            )
        })
        .map(|file| ReviewedVersionFile {
            revision_id: file.revision_id.clone().unwrap(),
            current_digest: file.current_digest.clone(),
        })
        .collect();
    let outcome = apply_written_version(
        &fixture.storage,
        scope,
        &key(id),
        &reviewed,
        &fixture.backups(),
    )
    .unwrap();
    assert!(
        outcome
            .files
            .iter()
            .all(|file| file.state == RestoreState::Restored)
    );
    let page =
        configuration_versions(&fixture.storage, scope, None, 1, &fixture.backups()).unwrap();
    assert_eq!(
        page.current.matched_entry_key.as_deref(),
        Some(key(id).as_str())
    );
    assert_eq!(page.current_entry.unwrap().entry_key, key(id));
}

#[test]
fn independent_versions_allow_deleting_middle_and_previously_selected_versions() {
    for (kind, names) in [
        (OperationKind::ClaudeCodeSettings, &["settings.json"][..]),
        (
            OperationKind::PiSettings,
            &["models.json", "settings.json"][..],
        ),
        (OperationKind::GrokSettings, &["config.toml"][..]),
    ] {
        let fixture = Fixture::new();
        let scope = HistoryScope::new(kind, &fixture.root, names).unwrap();
        let mut ids = Vec::new();
        for value in [b"A", b"B", b"C", b"D"] {
            let files: Vec<_> = names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    // Pi's unchanged second file must still get its own full copy.
                    fixture.file(name, if index == 0 { value } else { b"unchanged" })
                })
                .collect();
            ids.push(fixture.apply(kind, &files).unwrap().operation_id);
        }
        for id in &ids {
            let files = {
                let connection = fixture.storage.lock().unwrap();
                super::super::versions::files(&connection, id).unwrap()
            };
            assert_eq!(files.len(), names.len());
            for file in files {
                assert!(std::path::Path::new(&file.copy_path).is_file());
                assert!(file.copy_id.starts_with(&format!("version-{id}-")));
            }
        }
        // B is removable even though C's old recovery backup contains B.
        let removed = delete_configuration_version(
            &fixture.storage,
            &scope,
            &key(&ids[1]),
            &fixture.backups(),
        )
        .unwrap();
        assert_eq!(removed.state, HistoryDeleteState::Completed);
        select(&fixture, &scope, &ids[2]);
        select(&fixture, &scope, &ids[0]);
        // C now has a completed restore batch. It is still independently deletable.
        assert!(
            fixture
                .revisions()
                .iter()
                .any(|row| row.operation == ids[2] && row.intent == "restore")
        );
        let before = fs::read(fixture.root.join(names[0])).unwrap();
        delete_configuration_version(&fixture.storage, &scope, &key(&ids[2]), &fixture.backups())
            .unwrap();
        assert_eq!(fs::read(fixture.root.join(names[0])).unwrap(), before);
        assert!(operation(&fixture.storage, &ids[2]).unwrap().is_some());
        select(&fixture, &scope, &ids[3]);
        select(&fixture, &scope, &ids[0]);
        let page =
            configuration_versions(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
        assert_eq!(page.entries.len(), 2);
        for index in [1, 2] {
            assert!(
                !fixture
                    .backups()
                    .root()
                    .join(format!("version-{}-0", ids[index]))
                    .exists()
            );
            assert!(
                plan_written_version(
                    &fixture.storage,
                    &scope,
                    &key(&ids[index]),
                    &fixture.backups()
                )
                .is_err()
            );
            assert_eq!(
                delete_configuration_version(
                    &fixture.storage,
                    &scope,
                    &key(&ids[index]),
                    &fixture.backups()
                )
                .unwrap()
                .state,
                HistoryDeleteState::Completed
            );
        }
        let reopened = Storage::open_in_directory(&fixture.root).unwrap();
        materialize_configuration_versions(&reopened, &fixture.backups()).unwrap();
        assert_eq!(
            configuration_versions(&reopened, &scope, None, 50, &fixture.backups())
                .unwrap()
                .entries
                .len(),
            2
        );
    }
}

#[test]
fn a_missing_version_copy_is_not_borrowed_from_another_journal() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let first = fixture
        .apply(kind, &[fixture.file("settings.json", b"A")])
        .unwrap()
        .operation_id;
    fixture
        .apply(kind, &[fixture.file("settings.json", b"B")])
        .unwrap();
    let scope = HistoryScope::new(kind, &fixture.root, &["settings.json"]).unwrap();
    fs::remove_file(
        fixture
            .backups()
            .root()
            .join(format!("version-{first}-0/settings.json")),
    )
    .unwrap();
    let plan =
        plan_written_version(&fixture.storage, &scope, &key(&first), &fixture.backups()).unwrap();
    assert!(!plan.restorable);
    assert_eq!(plan.files[0].action, VersionRestoreAction::Blocked);
    assert_eq!(fs::read(fixture.root.join("settings.json")).unwrap(), b"B");
}

#[test]
fn snapshot_record_failure_prevents_any_target_write() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("settings.json"), b"original").unwrap();
    fixture.sql("CREATE TRIGGER refuse_version_copy BEFORE INSERT ON config_version_file BEGIN SELECT RAISE(FAIL, 'synthetic'); END;");
    assert_eq!(
        fixture.apply(
            OperationKind::ClaudeCodeSettings,
            &[fixture.file("settings.json", b"planned")]
        ),
        Err(WriteError::LogWriteFailed)
    );
    assert_eq!(
        fs::read(fixture.root.join("settings.json")).unwrap(),
        b"original"
    );
    assert!(fixture.revisions().is_empty());
}

#[test]
fn failed_version_delete_retries_after_restart_without_losing_other_versions() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let a = fixture
        .apply(kind, &[fixture.file("settings.json", b"A")])
        .unwrap()
        .operation_id;
    let b = fixture
        .apply(kind, &[fixture.file("settings.json", b"B")])
        .unwrap()
        .operation_id;
    let scope = HistoryScope::new(kind, &fixture.root, &["settings.json"]).unwrap();
    let blocked = fixture
        .backups()
        .root()
        .join(format!("version-{b}-0"))
        .join("unexpected-directory");
    fs::create_dir(&blocked).unwrap();
    let outcome =
        delete_configuration_version(&fixture.storage, &scope, &key(&b), &fixture.backups())
            .unwrap();
    assert_eq!(outcome.state, HistoryDeleteState::Failed);
    assert_eq!(outcome.leftovers.len(), 1);
    assert_eq!(
        configuration_versions(&fixture.storage, &scope, None, 50, &fixture.backups())
            .unwrap()
            .entries
            .len(),
        2
    );
    fs::remove_dir(&blocked).unwrap();
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    assert_eq!(
        delete_configuration_version(&reopened, &scope, &key(&b), &fixture.backups())
            .unwrap()
            .state,
        HistoryDeleteState::Completed
    );
    select(&fixture, &scope, &a);
}

#[test]
fn delete_retry_rechecks_new_unfinished_and_interrupted_restore_work() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let a = fixture
        .apply(kind, &[fixture.file("settings.json", b"A")])
        .unwrap()
        .operation_id;
    let b = fixture
        .apply(kind, &[fixture.file("settings.json", b"B")])
        .unwrap()
        .operation_id;
    let scope = HistoryScope::new(kind, &fixture.root, &["settings.json"]).unwrap();
    select(&fixture, &scope, &a);
    let copy_dir = fixture.backups().root().join(format!("version-{b}-0"));
    let obstruction = copy_dir.join("unexpected-directory");
    fs::create_dir(&obstruction).unwrap();
    assert_eq!(
        delete_configuration_version(&fixture.storage, &scope, &key(&b), &fixture.backups())
            .unwrap()
            .state,
        HistoryDeleteState::Failed
    );
    // Model a failed unlink that left the saved bytes readable (for example,
    // an open file handle on Windows), rather than relying on host permissions.
    fs::write(copy_dir.join("settings.json"), b"B").unwrap();
    select(&fixture, &scope, &b);
    fs::remove_dir(obstruction).unwrap();
    fixture.sql(&format!(
        "UPDATE config_operation SET state = 'planned' WHERE id = '{b}';"
    ));
    assert_eq!(
        delete_configuration_version(&fixture.storage, &scope, &key(&b), &fixture.backups()),
        Err(WriteError::HistoryActive)
    );
    fixture.sql(&format!("UPDATE config_operation SET state = 'applied' WHERE id = '{b}'; UPDATE config_file_revision SET state = 'prepared' WHERE operation_id = '{b}' AND intent = 'restore';"));
    assert_eq!(
        delete_configuration_version(&fixture.storage, &scope, &key(&b), &fixture.backups()),
        Err(WriteError::HistoryUnresolved)
    );
    assert!(copy_dir.join("settings.json").exists());
    fixture.sql(&format!("UPDATE config_file_revision SET state = 'completed' WHERE operation_id = '{b}' AND intent = 'restore';"));
    assert_eq!(
        delete_configuration_version(&fixture.storage, &scope, &key(&b), &fixture.backups())
            .unwrap()
            .state,
        HistoryDeleteState::Completed
    );
    assert_eq!(fs::read(fixture.root.join("settings.json")).unwrap(), b"B");
}

#[test]
fn deleting_current_version_keeps_agent_bytes_and_rejects_stale_selection_and_wrong_scope() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let a = fixture
        .apply(kind, &[fixture.file("settings.json", b"A")])
        .unwrap()
        .operation_id;
    fixture
        .apply(kind, &[fixture.file("settings.json", b"B")])
        .unwrap();
    let scope = HistoryScope::new(kind, &fixture.root, &["settings.json"]).unwrap();
    let plan =
        plan_written_version(&fixture.storage, &scope, &key(&a), &fixture.backups()).unwrap();
    let token = ReviewedVersionFile {
        revision_id: plan.files[0].revision_id.clone().unwrap(),
        current_digest: plan.files[0].current_digest.clone(),
    };
    select(&fixture, &scope, &a);
    delete_configuration_version(&fixture.storage, &scope, &key(&a), &fixture.backups()).unwrap();
    let page =
        configuration_versions(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.current_entry.is_none());
    assert!(page.current.matched_entry_key.is_none());
    assert!(
        apply_written_version(
            &fixture.storage,
            &scope,
            &key(&a),
            std::slice::from_ref(&token),
            &fixture.backups()
        )
        .is_err()
    );
    assert!(
        crate::config_apply::read_written_version_text(
            &fixture.storage,
            &scope,
            &key(&a),
            &token.revision_id,
            token.current_digest.as_deref(),
            &fixture.backups()
        )
        .is_err()
    );
    assert_eq!(fs::read(fixture.root.join("settings.json")).unwrap(), b"A");
    assert!(operation(&fixture.storage, &a).unwrap().is_some());
    let other_scope =
        HistoryScope::new(OperationKind::PiSettings, &fixture.root, &["settings.json"]).unwrap();
    assert_eq!(
        delete_configuration_version(&fixture.storage, &other_scope, &key(&a), &fixture.backups()),
        Err(WriteError::InvalidStoredOperation)
    );
}

#[test]
fn interrupted_legacy_copy_and_publication_resume_without_touching_live_files() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    let applied = fixture
        .apply(
            kind,
            &[
                fixture.file("models.json", b"models"),
                fixture.file("settings.json", b"settings"),
            ],
        )
        .unwrap();
    fixture.sql("UPDATE config_version SET state = 'migrating';");
    let missing = fixture
        .backups()
        .root()
        .join(format!("version-{}-1", applied.operation_id));
    fs::remove_dir_all(missing).unwrap();
    // The first file's already-migrated copy is now the only trustworthy bytes.
    fs::write(fixture.root.join("models.json"), b"external").unwrap();
    materialize_configuration_versions(&fixture.storage, &fixture.backups()).unwrap();
    assert_eq!(
        fs::read(fixture.root.join("models.json")).unwrap(),
        b"external"
    );
    let scope = HistoryScope::new(kind, &fixture.root, &["models.json", "settings.json"]).unwrap();
    select(&fixture, &scope, &applied.operation_id);
    assert_eq!(
        fs::read(fixture.root.join("models.json")).unwrap(),
        b"models"
    );
    fixture.sql("UPDATE config_version SET state = 'prepared';");
    materialize_configuration_versions(&fixture.storage, &fixture.backups()).unwrap();
    assert_eq!(
        fixture
            .storage
            .lock()
            .unwrap()
            .query_row("SELECT state FROM config_version", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "available"
    );
}

#[test]
fn unavailable_legacy_bytes_stay_explicit_and_the_version_can_be_deleted() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let id = fixture
        .apply(kind, &[fixture.file("settings.json", b"lost")])
        .unwrap()
        .operation_id;
    fixture.sql("DELETE FROM config_version_file; UPDATE config_version SET state = 'legacy';");
    fs::remove_dir_all(fixture.backups().root().join(format!("version-{id}-0"))).unwrap();
    fs::write(fixture.root.join("settings.json"), b"external").unwrap();
    materialize_configuration_versions(&fixture.storage, &fixture.backups()).unwrap();
    let scope = HistoryScope::new(kind, &fixture.root, &["settings.json"]).unwrap();
    let plan =
        plan_written_version(&fixture.storage, &scope, &key(&id), &fixture.backups()).unwrap();
    assert!(!plan.restorable);
    assert_eq!(plan.files[0].action, VersionRestoreAction::Blocked);
    assert_eq!(
        delete_configuration_version(&fixture.storage, &scope, &key(&id), &fixture.backups())
            .unwrap()
            .state,
        HistoryDeleteState::Completed
    );
    assert_eq!(
        fs::read(fixture.root.join("settings.json")).unwrap(),
        b"external"
    );
}

#[test]
fn legacy_conversion_copies_bytes_once_and_does_not_create_a_reference_chain() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let a = fixture
        .apply(kind, &[fixture.file("settings.json", b"A")])
        .unwrap()
        .operation_id;
    let b = fixture
        .apply(kind, &[fixture.file("settings.json", b"B")])
        .unwrap()
        .operation_id;
    // Recreate the metadata state of schema-v24 data. Recovery copies remain;
    // version-owned copies did not exist in that schema.
    fixture.sql("DELETE FROM config_version_file; UPDATE config_version SET state = 'legacy';");
    for id in [&a, &b] {
        fs::remove_dir_all(fixture.backups().root().join(format!("version-{id}-0"))).unwrap();
    }
    materialize_configuration_versions(&fixture.storage, &fixture.backups()).unwrap();
    let scope = HistoryScope::new(kind, &fixture.root, &["settings.json"]).unwrap();
    // Remove B's actual audit backups as well: A must not depend on them now.
    delete_history_entry(&fixture.storage, &scope, &key(&b), &fixture.backups()).unwrap();
    select(&fixture, &scope, &a);
    assert_eq!(fs::read(fixture.root.join("settings.json")).unwrap(), b"A");
}
