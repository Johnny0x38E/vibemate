//! Real-SQLite history pagination tests. All files and journal faults are synthetic.

use super::*;
use std::collections::BTreeSet;

fn scope(fixture: &Fixture, kind: OperationKind, directory: &str, files: &[&str]) -> HistoryScope {
    HistoryScope::new(kind, &fixture.root.join(directory), files).unwrap()
}

fn file_at(fixture: &Fixture, directory: &str, name: &str, content: &[u8]) -> PlannedFile {
    // Match production's root.join(file) spelling, including Windows separators.
    // Equivalent filesystem paths can still differ as exact journal strings.
    let relative = std::path::Path::new(directory).join(name);
    fixture.file(relative.to_str().unwrap(), content)
}

fn apply_at(
    fixture: &Fixture,
    kind: OperationKind,
    directory: &str,
    contents: &[(&str, &[u8])],
) -> String {
    fs::create_dir_all(fixture.root.join(directory)).unwrap();
    let files: Vec<_> = contents
        .iter()
        .map(|(name, content)| file_at(fixture, directory, name, content))
        .collect();
    fixture.apply(kind, &files).unwrap().operation_id
}

fn all_pages(
    storage: &Storage,
    scope: &HistoryScope,
    limit: u32,
    backups: &BackupStore,
) -> Vec<HistoryEntry> {
    let mut cursor = None;
    let mut entries = Vec::new();
    for _ in 0..200 {
        let page = configuration_history(storage, scope, cursor.as_ref(), limit, backups).unwrap();
        assert!(page.entries.len() <= limit as usize);
        assert!(page.next_cursor.is_none() || !page.entries.is_empty());
        entries.extend(page.entries);
        let Some(next) = page.next_cursor else {
            return entries;
        };
        // A cursor survives serialization and a future IPC round trip without
        // relying on private Rust fields or an in-memory server session.
        cursor = Some(serde_json::from_str(&serde_json::to_string(&next).unwrap()).unwrap());
    }
    panic!("history cursor failed to reach the end");
}

#[test]
fn all_agents_page_beyond_ten_and_fifty_with_timestamp_ties_without_duplicates() {
    for kind in [
        OperationKind::ClaudeCodeSettings,
        OperationKind::PiSettings,
        OperationKind::GrokSettings,
    ] {
        let fixture = Fixture::new();
        let mut expected = Vec::new();
        for index in 0..53 {
            let id = apply_at(
                &fixture,
                kind,
                "agent",
                &[("settings", format!("value-{index}").as_bytes())],
            );
            expected.push(format!("operation:{id}"));
        }
        fixture.sql("UPDATE config_operation SET created_at = 1000;");
        expected.sort_by(|left, right| right.cmp(left));
        let scope = scope(&fixture, kind, "agent", &["settings"]);
        for limit in [1, 7, 50] {
            let entries = all_pages(&fixture.storage, &scope, limit, &fixture.backups());
            assert_eq!(
                entries
                    .iter()
                    .map(|row| row.entry_key.clone())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                entries
                    .iter()
                    .map(|row| &row.entry_key)
                    .collect::<BTreeSet<_>>()
                    .len(),
                53
            );
            assert!(
                entries
                    .iter()
                    .all(|row| row.intent == HistoryIntent::Apply && row.revisions.len() == 1)
            );
        }
    }
}

#[test]
fn exact_scope_excludes_other_agents_prefix_roots_unowned_files_and_mixed_root_operations() {
    let fixture = Fixture::new();
    let kinds = [
        OperationKind::ClaudeCodeSettings,
        OperationKind::PiSettings,
        OperationKind::GrokSettings,
    ];
    // SQL LIKE metacharacters and a quote must remain literal bound path text.
    let directories = ["root_%'", "root_%'-other", "another"];
    let mut expected = Vec::new();
    for kind in kinds {
        for directory in directories {
            let id = apply_at(
                &fixture,
                kind,
                directory,
                &[("settings", kind.as_str().as_bytes())],
            );
            expected.push((kind, directory, id));
        }
    }
    let unowned = apply_at(
        &fixture,
        kinds[0],
        directories[0],
        &[("unowned", b"private")],
    );
    let mixed = fixture
        .apply(
            kinds[0],
            &[
                file_at(&fixture, directories[0], "settings", b"mixed-a"),
                file_at(&fixture, directories[1], "settings", b"mixed-b"),
            ],
        )
        .unwrap()
        .operation_id;
    for (kind, directory, id) in expected {
        let scope = scope(&fixture, kind, directory, &["settings"]);
        let entries = all_pages(&fixture.storage, &scope, 2, &fixture.backups());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operation_id, id);
        assert_ne!(entries[0].operation_id, mixed);
        assert_ne!(entries[0].operation_id, unowned);
    }
}

#[test]
fn restores_and_compensations_are_separate_whole_multi_file_entries_not_new_legacy_jobs() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    let scope = scope(&fixture, kind, "pi", &["models.json", "settings.json"]);
    fs::create_dir_all(fixture.root.join("pi")).unwrap();
    fs::write(fixture.root.join("pi/models.json"), b"original-models").unwrap();
    fs::write(fixture.root.join("pi/settings.json"), b"original-settings").unwrap();
    let first = apply_at(
        &fixture,
        kind,
        "pi",
        &[
            ("models.json", b"A-models"),
            ("settings.json", b"A-settings"),
        ],
    );
    fixture.restore(&first).unwrap();
    let second = apply_at(
        &fixture,
        kind,
        "pi",
        &[
            ("models.json", b"B-models"),
            ("settings.json", b"B-settings"),
        ],
    );
    // Model a crash after step results committed, before final operation state.
    finish_operation(&fixture.storage, &second, OperationState::Partial).unwrap();
    compensate_operation(&fixture.storage, &second, &fixture.backups()).unwrap();
    fixture.sql(
        "UPDATE config_operation SET created_at = 1000;
        UPDATE config_file_revision SET created_at = 1000;
        UPDATE config_file_revision SET created_at = 999 WHERE rowid = (
            SELECT max(rowid) FROM config_file_revision WHERE intent = 'restore');",
    );
    let entries = all_pages(&fixture.storage, &scope, 1, &fixture.backups());
    assert_eq!(entries.len(), 4);
    for entry in &entries {
        assert_eq!(entry.revisions.len(), 2);
        assert_eq!(entry.source_steps.len(), 2);
        assert_eq!(
            entry
                .revisions
                .iter()
                .map(|row| row.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(
            entry
                .revisions
                .iter()
                .all(|row| row.state == HistoryRevisionState::Completed && row.backup_recorded)
        );
    }
    let restore = entries
        .iter()
        .find(|entry| entry.intent == HistoryIntent::Restore)
        .unwrap();
    assert_eq!(restore.operation_id, first);
    // A later file with an earlier wall clock does not move its batch's sort key.
    assert_eq!(restore.created_at, 1000);
    let compensation = entries
        .iter()
        .find(|entry| entry.intent == HistoryIntent::Compensation)
        .unwrap();
    assert_eq!(compensation.operation_id, second);
    assert_eq!(
        recent_operations(&fixture.storage, kind, 50).unwrap().len(),
        2
    );
    assert_eq!(
        fs::read(fixture.root.join("pi/models.json")).unwrap(),
        b"original-models"
    );
    assert_eq!(
        fs::read(fixture.root.join("pi/settings.json")).unwrap(),
        b"original-settings"
    );
    let mut expected_order: Vec<_> = entries.iter().map(|entry| &entry.entry_key).collect();
    expected_order.sort_by(|left, right| right.cmp(left));
    assert_eq!(
        entries
            .iter()
            .map(|entry| &entry.entry_key)
            .collect::<Vec<_>>(),
        expected_order
    );
}

#[test]
fn legacy_failed_pending_and_unchanged_attempts_remain_visible_without_invented_snapshots() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::create_dir_all(fixture.root.join("legacy")).unwrap();
    let target = file_at(&fixture, "legacy", "settings", b"legacy-value");
    let legacy = begin_operation(
        &fixture.storage,
        kind,
        None,
        std::slice::from_ref(&target.target),
    )
    .unwrap();
    let applied = apply_file(&target.target, &target.content, &fixture.backups()).unwrap();
    record_step_applied(
        &fixture.storage,
        &legacy,
        0,
        applied.backup.as_ref(),
        applied.target.content_digest.as_deref().unwrap(),
        false,
    )
    .unwrap();
    finish_operation(&fixture.storage, &legacy, OperationState::Applied).unwrap();
    let target = file_at(&fixture, "legacy", "settings", b"next");
    let pending = begin_operation(
        &fixture.storage,
        kind,
        None,
        std::slice::from_ref(&target.target),
    )
    .unwrap();
    let failed = begin_operation(&fixture.storage, kind, None, &[target.target]).unwrap();
    record_step_failed(&fixture.storage, &failed, 0, WriteError::TargetChanged).unwrap();
    finish_operation(&fixture.storage, &failed, OperationState::Failed).unwrap();
    let unchanged = apply_at(&fixture, kind, "legacy", &[("settings", b"legacy-value")]);
    let scope = scope(&fixture, kind, "legacy", &["settings"]);
    let entries = all_pages(&fixture.storage, &scope, 2, &fixture.backups());
    assert_eq!(entries.len(), 4);
    assert!(
        entries
            .iter()
            .all(|entry| entry.revisions.is_empty() && entry.intent == HistoryIntent::Apply)
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| &entry.operation_id)
            .collect::<BTreeSet<_>>(),
        [&legacy, &pending, &failed, &unchanged]
            .into_iter()
            .collect()
    );
    let legacy = entries
        .iter()
        .find(|entry| entry.operation_id == legacy)
        .unwrap();
    assert_eq!(legacy.source_steps[0].previous_existed, Some(false));
    assert!(!legacy.source_steps[0].backup_recorded);
    assert_eq!(
        legacy.source_steps[0].applied_digest.as_deref(),
        Some(digest_hex(b"legacy-value").as_str())
    );
    let failed = entries
        .iter()
        .find(|entry| entry.operation_id == failed)
        .unwrap();
    assert_eq!(
        failed.source_steps[0].error_code,
        Some(WriteError::TargetChanged)
    );
}

#[test]
fn restart_updates_newer_insertions_and_removed_cursor_anchors_do_not_shift_later_pages() {
    let fixture = Fixture::new();
    let kind = OperationKind::GrokSettings;
    let scope = scope(&fixture, kind, "grok", &["settings"]);
    for index in 0..6 {
        apply_at(
            &fixture,
            kind,
            "grok",
            &[("settings", format!("old-{index}").as_bytes())],
        );
    }
    fixture.sql("UPDATE config_operation SET created_at = 1000;");
    let original = all_pages(&fixture.storage, &scope, 50, &fixture.backups());
    let first =
        configuration_history(&fixture.storage, &scope, None, 2, &fixture.backups()).unwrap();
    let cursor = first.next_cursor.unwrap();
    let anchor_id = &first.entries[1].operation_id;
    {
        let connection = fixture.storage.lock().unwrap();
        connection
            .execute(
                "DELETE FROM config_file_revision WHERE operation_id = ?1",
                [anchor_id],
            )
            .unwrap();
        connection
            .execute("DELETE FROM config_operation WHERE id = ?1", [anchor_id])
            .unwrap();
    }
    let newer = apply_at(&fixture, kind, "grok", &[("settings", b"newer")]);
    finish_operation(
        &fixture.storage,
        &original[3].operation_id,
        OperationState::Partial,
    )
    .unwrap();
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let later =
        configuration_history(&reopened, &scope, Some(&cursor), 50, &fixture.backups()).unwrap();
    assert_eq!(
        later
            .entries
            .iter()
            .map(|entry| &entry.entry_key)
            .collect::<Vec<_>>(),
        original[2..]
            .iter()
            .map(|entry| &entry.entry_key)
            .collect::<Vec<_>>()
    );
    assert!(later.next_cursor.is_none());
    assert!(
        !later
            .entries
            .iter()
            .any(|entry| entry.operation_id == newer)
    );
    let refreshed = configuration_history(&reopened, &scope, None, 1, &fixture.backups()).unwrap();
    assert_eq!(refreshed.entries[0].operation_id, newer);
    assert_eq!(later.entries[1].source_state, OperationState::Partial);
}

#[test]
fn history_keeps_metadata_after_target_and_backup_removal_and_exposes_no_copy_paths_or_bytes() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    apply_at(
        &fixture,
        kind,
        "pi",
        &[("settings", b"synthetic-private-original")],
    );
    apply_at(
        &fixture,
        kind,
        "pi",
        &[("settings", b"synthetic-private-next")],
    );
    let scope = scope(&fixture, kind, "pi", &["settings"]);
    let before =
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
    assert!(
        before
            .entries
            .iter()
            .any(|entry| entry.revisions[0].backup_recorded)
    );
    fs::remove_dir_all(fixture.root.join("pi")).unwrap();
    fs::remove_dir_all(fixture.backups().root()).unwrap();
    let after =
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
    assert_eq!(before.entries.len(), after.entries.len());
    for (previous, current) in before.entries.iter().zip(&after.entries) {
        assert_eq!(previous.entry_key, current.entry_key);
        assert_eq!(previous.operation_id, current.operation_id);
        assert_eq!(previous.revisions.len(), current.revisions.len());
        for (before_revision, after_revision) in previous.revisions.iter().zip(&current.revisions) {
            assert_eq!(before_revision.revision_id, after_revision.revision_id);
            assert_eq!(
                before_revision.backup_recorded,
                after_revision.backup_recorded
            );
            if after_revision.backup_recorded {
                assert_eq!(before_revision.backup_state, RevisionBackupState::Available);
                assert_eq!(
                    after_revision.backup_state,
                    RevisionBackupState::Unavailable
                );
            } else {
                assert_eq!(after_revision.backup_state, RevisionBackupState::NotNeeded);
            }
        }
    }
    let serialized = serde_json::to_string(&after).unwrap();
    for private in [
        "synthetic-private-original",
        "synthetic-private-next",
        "backupPath",
        "backupDigest",
        fixture.backups().root().to_str().unwrap(),
    ] {
        assert!(!serialized.contains(private));
    }
    assert!(serialized.contains("backupRecorded"));
    assert!(serialized.contains("backupState"));
    assert!(serialized.contains("unavailable"));
}

#[test]
fn empty_pages_limits_scopes_and_cursor_shapes_are_explicitly_validated() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    let root = fixture.root.join("root");
    for files in [
        vec![],
        vec!["settings", "settings"],
        vec!["../settings"],
        vec!["nested/settings"],
        vec!["C:settings"],
        vec!["nested\\settings"],
        vec!["settings/"],
        vec![""],
        vec!["."],
    ] {
        assert!(matches!(
            HistoryScope::new(kind, &root, &files),
            Err(WriteError::InvalidRequest)
        ));
    }
    let too_many: Vec<_> = (0..17).map(|index| format!("file-{index}")).collect();
    assert!(
        HistoryScope::new(
            kind,
            &root,
            &too_many.iter().map(String::as_str).collect::<Vec<_>>()
        )
        .is_err()
    );
    assert!(HistoryScope::new(kind, std::path::Path::new("relative"), &["settings"]).is_err());
    let scope = scope(&fixture, kind, "root", &["settings", "models"]);
    for limit in [0, 51, u32::MAX] {
        assert_eq!(
            configuration_history(&fixture.storage, &scope, None, limit, &fixture.backups()),
            Err(WriteError::InvalidRequest)
        );
    }
    for limit in [1, 50] {
        let page = configuration_history(&fixture.storage, &scope, None, limit, &fixture.backups())
            .unwrap();
        assert!(page.entries.is_empty() && page.next_cursor.is_none());
    }
    for index in 0..3 {
        apply_at(
            &fixture,
            kind,
            "root",
            &[("settings", format!("{index}").as_bytes())],
        );
    }
    let page =
        configuration_history(&fixture.storage, &scope, None, 1, &fixture.backups()).unwrap();
    let cursor = page.next_cursor.unwrap();
    for changed in [
        HistoryScope::new(OperationKind::PiSettings, &root, &["settings", "models"]).unwrap(),
        HistoryScope::new(
            kind,
            &fixture.root.join("root-other"),
            &["settings", "models"],
        )
        .unwrap(),
        HistoryScope::new(kind, &root, &["settings"]).unwrap(),
    ] {
        assert_eq!(
            configuration_history(
                &fixture.storage,
                &changed,
                Some(&cursor),
                1,
                &fixture.backups()
            ),
            Err(WriteError::InvalidRequest)
        );
    }
    let reordered = HistoryScope::new(kind, &root, &["models", "settings"]).unwrap();
    assert!(
        configuration_history(
            &fixture.storage,
            &reordered,
            Some(&cursor),
            1,
            &fixture.backups()
        )
        .is_ok()
    );
    let value = serde_json::to_value(&cursor).unwrap();
    for (field, bad) in [
        ("createdAt", serde_json::json!(-1)),
        ("entryKey", serde_json::json!("batch:../escape")),
        ("entryKey", serde_json::json!("operation:bad")),
        ("scopeDigest", serde_json::json!("wrong")),
    ] {
        let mut invalid = value.clone();
        invalid[field] = bad;
        let parsed: HistoryCursor = serde_json::from_value(invalid).unwrap();
        assert_eq!(
            configuration_history(
                &fixture.storage,
                &scope,
                Some(&parsed),
                1,
                &fixture.backups()
            ),
            Err(WriteError::InvalidRequest)
        );
    }
    let mut unknown = value;
    unknown["path"] = serde_json::json!("/not-an-input-path");
    assert!(serde_json::from_value::<HistoryCursor>(unknown).is_err());
}

#[test]
fn malformed_selected_revisions_are_errors_not_skipped_or_labeled_as_valid_versions() {
    for mutation in [
        "UPDATE config_file_revision SET batch_id = 'different-batch'",
        "UPDATE config_file_revision SET target_path = '/different-root/settings'",
        "UPDATE config_file_revision SET id = '../escape'",
        "UPDATE config_file_revision SET previous_digest = 'invalid'",
        "UPDATE config_file_revision SET state = 'unsupported'",
        "UPDATE config_file_revision SET intent = 'unsupported'",
        "UPDATE config_file_revision SET state = 'failed', error_code = NULL",
        "UPDATE config_file_revision SET state = 'unknown', error_code = 'write_failed'",
        "UPDATE config_file_revision SET state = 'failed', error_code = 'unverified_error_text'",
        "UPDATE config_file_revision SET error_code = 'write_failed'",
        "UPDATE config_file_revision SET updated_at = 0",
    ] {
        let fixture = Fixture::new();
        let kind = OperationKind::PiSettings;
        apply_at(&fixture, kind, "pi", &[("settings", b"first")]);
        fixture.sql(&format!(
            "PRAGMA ignore_check_constraints=ON; {mutation}; PRAGMA ignore_check_constraints=OFF;"
        ));
        let scope = scope(&fixture, kind, "pi", &["settings"]);
        assert_eq!(
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()),
            Err(WriteError::InvalidStoredOperation),
            "{mutation}"
        );
    }
}

#[test]
fn mixed_direction_duplicate_ordinal_and_cross_source_batch_collisions_fail_closed() {
    for fault in ["direction", "ordinal", "source"] {
        let fixture = Fixture::new();
        let kind = OperationKind::PiSettings;
        let first = apply_at(
            &fixture,
            kind,
            "pi",
            &[("settings", b"A"), ("models", b"B")],
        );
        fixture.restore(&first).unwrap();
        let batch: String = fixture
            .storage
            .lock()
            .unwrap()
            .query_row(
                "SELECT batch_id FROM config_file_revision WHERE intent = 'restore' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        match fault {
            "direction" => {
                fixture.storage.lock().unwrap().execute("UPDATE config_file_revision SET intent='compensation' WHERE batch_id=?1 AND ordinal=1", [&batch]).unwrap();
            }
            "ordinal" => {
                fixture.storage.lock().unwrap().execute("INSERT INTO config_file_revision SELECT id || '-duplicate', batch_id, operation_id, ordinal, intent, target_path, previous_digest, next_digest, backup_path, state, error_code, created_at, updated_at, direct FROM config_file_revision WHERE batch_id=?1 AND ordinal=0", [&batch]).unwrap();
            }
            "source" => {
                let other = apply_at(&fixture, kind, "pi-other", &[("settings", b"other")]);
                fixture.restore(&other).unwrap();
                fixture.storage.lock().unwrap().execute("UPDATE config_file_revision SET batch_id=?1 WHERE operation_id=?2 AND intent='restore'", rusqlite::params![batch, other]).unwrap();
            }
            _ => unreachable!(),
        }
        let scope = scope(&fixture, kind, "pi", &["settings", "models"]);
        assert_eq!(
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()),
            Err(WriteError::InvalidStoredOperation),
            "{fault}"
        );
        if fault == "source" {
            // A collision whose first inserted row belongs to another root must
            // not silently hide the scoped batch from this query.
            let scope =
                HistoryScope::new(kind, &fixture.root.join("pi-other"), &["settings"]).unwrap();
            assert_eq!(
                configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()),
                Err(WriteError::InvalidStoredOperation)
            );
        }
    }
}

#[test]
fn uncertain_and_failed_results_and_explicit_resolution_audits_remain_distinguishable() {
    for intended in [false, true] {
        let fixture = Fixture::new();
        let kind = OperationKind::ClaudeCodeSettings;
        fs::create_dir_all(fixture.root.join("claude")).unwrap();
        fs::write(fixture.root.join("claude/settings"), b"original").unwrap();
        fixture.sql("CREATE TRIGGER refuse_history_finish BEFORE UPDATE ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
        assert_eq!(
            fixture
                .apply(
                    kind,
                    &[file_at(&fixture, "claude", "settings", b"intended")]
                )
                .unwrap_err(),
            WriteError::LogWriteFailed
        );
        fixture.sql("DROP TRIGGER refuse_history_finish;");
        let scope = scope(&fixture, kind, "claude", &["settings"]);
        let before =
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
        assert_eq!(
            before.entries[0].revisions[0].state,
            HistoryRevisionState::Prepared
        );
        assert!(before.entries[0].revisions[0].resolution.is_none());
        fixture
            .sql("UPDATE config_file_revision SET state='unknown', error_code='outcome_unknown';");
        let unknown =
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
        assert_eq!(
            unknown.entries[0].revisions[0].state,
            HistoryRevisionState::Unknown
        );
        if !intended {
            fs::write(fixture.root.join("claude/settings"), b"original").unwrap();
        }
        let id = &before.entries[0].operation_id;
        let plan = plan_revision_resolution(&fixture.storage, id, &fixture.backups())
            .unwrap()
            .unwrap();
        resolve_interrupted_revisions(&fixture.storage, id, &plan, &fixture.backups()).unwrap();
        let after =
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
        let revision = &after.entries[0].revisions[0];
        assert_eq!(
            revision.state,
            if intended {
                HistoryRevisionState::Completed
            } else {
                HistoryRevisionState::Failed
            }
        );
        let audit = revision.resolution.as_ref().unwrap();
        assert_eq!(audit.previous_state, HistoryRevisionState::Unknown);
        assert_eq!(
            audit.current_match,
            if intended {
                RevisionMatch::Intended
            } else {
                RevisionMatch::Previous
            }
        );
        assert_eq!(
            audit.observed_digest,
            Some(digest_hex(if intended { b"intended" } else { b"original" }))
        );
        fixture.sql("PRAGMA ignore_check_constraints=ON; UPDATE config_revision_resolution SET observed_digest=NULL; PRAGMA ignore_check_constraints=OFF;");
        assert_eq!(
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()),
            Err(WriteError::InvalidStoredOperation)
        );
    }
}

#[test]
fn damaged_later_rows_fail_when_reached_and_query_failures_never_look_like_an_empty_history() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    for index in 0..2 {
        apply_at(
            &fixture,
            kind,
            "pi",
            &[("settings", format!("{index}").as_bytes())],
        );
    }
    let scope = scope(&fixture, kind, "pi", &["settings"]);
    let initial =
        configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()).unwrap();
    fixture
        .storage
        .lock()
        .unwrap()
        .execute(
            "UPDATE config_file_revision SET batch_id='invalid-batch' WHERE operation_id=?1",
            [&initial.entries[1].operation_id],
        )
        .unwrap();
    let first =
        configuration_history(&fixture.storage, &scope, None, 1, &fixture.backups()).unwrap();
    assert_eq!(first.entries.len(), 1);
    assert_eq!(
        configuration_history(
            &fixture.storage,
            &scope,
            first.next_cursor.as_ref(),
            1,
            &fixture.backups()
        ),
        Err(WriteError::InvalidStoredOperation)
    );
    fixture.sql("DROP TABLE config_revision_resolution;");
    assert_eq!(
        configuration_history(&fixture.storage, &scope, None, 1, &fixture.backups()),
        Err(WriteError::LogReadFailed)
    );
}

#[test]
fn each_page_observes_atomic_metadata_updates_from_an_independent_app_connection() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    let id = apply_at(&fixture, kind, "pi", &[("settings", b"value")]);
    let scope = scope(&fixture, kind, "pi", &["settings"]);
    let writer = Storage::open_in_directory(&fixture.root).unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        let handle = threads.spawn(|| {
            barrier.wait();
            for index in 0..100 {
                let applied = index % 2 == 0;
                let mut connection = writer.lock().unwrap();
                let transaction = connection.transaction().unwrap();
                transaction
                    .execute(
                        "UPDATE config_operation_step SET state=?2, error_code=?3,
                     applied_digest=?4, previous_existed=?5 WHERE operation_id=?1",
                        rusqlite::params![
                            id,
                            if applied { "applied" } else { "failed" },
                            if applied { None } else { Some("write_failed") },
                            if applied {
                                Some(digest_hex(b"value"))
                            } else {
                                None
                            },
                            if applied { Some(false) } else { None }
                        ],
                    )
                    .unwrap();
                transaction
                    .execute(
                        "UPDATE config_operation SET state=?2 WHERE id=?1",
                        rusqlite::params![id, if applied { "applied" } else { "failed" }],
                    )
                    .unwrap();
                transaction.commit().unwrap();
                std::thread::yield_now();
            }
        });
        barrier.wait();
        for _ in 0..100 {
            let page = configuration_history(&fixture.storage, &scope, None, 1, &fixture.backups())
                .unwrap();
            let entry = &page.entries[0];
            assert_eq!(
                entry.source_steps[0].state,
                match entry.source_state {
                    OperationState::Applied => StepState::Applied,
                    OperationState::Failed => StepState::Failed,
                    other => panic!("unexpected journal state: {other:?}"),
                }
            );
            std::thread::yield_now();
        }
        handle.join().unwrap();
    });
}

#[test]
fn malformed_source_steps_and_oversized_batches_are_not_returned_as_history() {
    for fault in [
        "source-error",
        "too-many-source-steps",
        "too-many-revisions",
    ] {
        let fixture = Fixture::new();
        let kind = OperationKind::PiSettings;
        apply_at(&fixture, kind, "pi", &[("settings", b"value")]);
        match fault {
            "source-error" => fixture.sql(
                "UPDATE config_operation_step SET state='failed',
                error_code='unknown_error_text', applied_digest=NULL, previous_existed=NULL;",
            ),
            "too-many-source-steps" => {
                let target = file_at(&fixture, "pi", "settings", b"next").target;
                begin_operation(&fixture.storage, kind, None, &vec![target; 17]).unwrap();
            }
            "too-many-revisions" => {
                // Extra rows sharing one ordinal must never inflate a batch
                // without bound, even if they were inserted outside the writer.
                for index in 0..20 {
                    fixture
                        .storage
                        .lock()
                        .unwrap()
                        .execute(
                            "INSERT INTO config_file_revision
                         SELECT id || ?1, batch_id, operation_id, ordinal, intent, target_path,
                         previous_digest, next_digest, backup_path, state, error_code,
                         created_at, updated_at, direct
                         FROM config_file_revision WHERE rowid=1",
                            [format!("-extra-{index}")],
                        )
                        .unwrap();
                }
            }
            _ => unreachable!(),
        }
        let scope = scope(&fixture, kind, "pi", &["settings"]);
        assert_eq!(
            configuration_history(&fixture.storage, &scope, None, 50, &fixture.backups()),
            Err(WriteError::InvalidStoredOperation),
            "{fault}"
        );
    }
}

#[test]
fn history_reports_real_backup_availability_not_journal_references_alone() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.root.join("agent")).unwrap();
    fs::write(fixture.root.join("agent/settings.json"), b"original").unwrap();
    let id = apply_at(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        "agent",
        &[("settings.json", b"first")],
    );
    let scope = scope(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        "agent",
        &["settings.json"],
    );
    let page =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    let entry = &page.entries[0];
    assert_eq!(entry.operation_id, id);
    assert_eq!(entry.revisions.len(), 1);
    assert!(entry.revisions[0].backup_recorded);
    assert_eq!(
        entry.revisions[0].backup_state,
        RevisionBackupState::Available
    );
    assert_eq!(
        entry.source_steps[0].backup_state,
        RevisionBackupState::Available
    );
    assert!(
        !serde_json::to_string(&page)
            .unwrap()
            .contains(fixture.backups().root().to_str().unwrap())
    );

    // Damaging the private copy must flip availability without exposing bytes.
    let revision_id = entry.revisions[0].revision_id.clone();
    let copy = fixture
        .backups()
        .root()
        .join(&revision_id)
        .join("settings.json");
    fs::write(&copy, b"tampered").unwrap();
    let damaged =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    assert_eq!(
        damaged.entries[0].revisions[0].backup_state,
        RevisionBackupState::Unavailable
    );
    fs::remove_file(&copy).unwrap();
    let missing =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    assert_eq!(
        missing.entries[0].revisions[0].backup_state,
        RevisionBackupState::Unavailable
    );
    assert!(missing.entries[0].revisions[0].backup_recorded);
}

#[test]
fn history_marks_absence_snapshots_as_backup_not_needed() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.root.join("agent")).unwrap();
    // Write a brand-new file so the revision records absence, not a private copy.
    fixture
        .apply(
            OperationKind::ClaudeCodeSettings,
            &[fixture.file("agent/settings.json", b"created")],
        )
        .unwrap();
    let scope = scope(
        &fixture,
        OperationKind::ClaudeCodeSettings,
        "agent",
        &["settings.json"],
    );
    let page =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    assert_eq!(page.entries[0].revisions[0].previous_digest, None);
    assert!(!page.entries[0].revisions[0].backup_recorded);
    assert_eq!(
        page.entries[0].revisions[0].backup_state,
        RevisionBackupState::NotNeeded
    );
    assert_eq!(
        page.entries[0].source_steps[0].backup_state,
        RevisionBackupState::NotNeeded
    );
}

#[test]
fn history_pins_current_by_digest_not_latest_operation_and_keeps_it_off_pagination() {
    let fixture = Fixture::new();
    let kind = OperationKind::ClaudeCodeSettings;
    fs::create_dir_all(fixture.root.join("agent")).unwrap();
    fs::write(fixture.root.join("agent/settings.json"), b"A").unwrap();
    let a = apply_at(&fixture, kind, "agent", &[("settings.json", b"B")]);
    let b = apply_at(&fixture, kind, "agent", &[("settings.json", b"C")]);
    let _c = apply_at(&fixture, kind, "agent", &[("settings.json", b"D")]);
    let scope = scope(&fixture, kind, "agent", &["settings.json"]);

    // After A→B→C, live files hold C's intended digest.
    let page =
        configuration_history(&fixture.storage, &scope, None, 1, &fixture.backups()).unwrap();
    assert_eq!(page.current.status, HistoryCurrentStatus::Matched);
    // Page size 1 returns only the newest entry; current still pins that version.
    assert_eq!(page.entries.len(), 1);
    assert_eq!(
        page.current.matched_entry_key.as_deref(),
        Some(page.entries[0].entry_key.as_str())
    );
    assert_eq!(
        page.current.matched_entry_key.as_deref(),
        Some(format!("operation:{_c}").as_str())
    );

    // Restore to B's pre-image (content B) by writing B's previous digest bytes.
    // B's previous is content after A (= "B" wait): A wrote B, so B's previous is "B"?
    // Start A: file "A". Apply A writes "B" (previous "A"). Apply B writes "C" (previous "B"). Apply C writes "D" (previous "C").
    // Restore to entry B means restore previous of B = "B". Pin should be B via previous.
    let revisions = fixture.revisions();
    let b_revision = revisions
        .iter()
        .find(|row| row.operation == b && row.intent == "apply")
        .unwrap();
    let b_previous = b_revision.before.clone().unwrap();
    // Write live file to match B's previous digest by restoring from backup.
    let bytes = fs::read(
        fixture
            .backups()
            .root()
            .join(&b_revision.id)
            .join("settings.json"),
    )
    .unwrap();
    assert_eq!(digest_hex(&bytes), b_previous);
    fs::write(fixture.root.join("agent/settings.json"), &bytes).unwrap();

    let page =
        configuration_history(&fixture.storage, &scope, None, 1, &fixture.backups()).unwrap();
    assert_eq!(page.current.status, HistoryCurrentStatus::Matched);
    assert_eq!(
        page.current.matched_entry_key.as_deref(),
        Some(format!("operation:{b}").as_str())
    );
    // Pagination still returns the newest history row, but the pin stays on B.
    assert_ne!(page.entries[0].entry_key, format!("operation:{b}"));
    let older = configuration_history(
        &fixture.storage,
        &scope,
        page.next_cursor.as_ref(),
        50,
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(
        older.current.matched_entry_key,
        page.current.matched_entry_key
    );
    assert_eq!(older.current.status, HistoryCurrentStatus::Matched);

    // External edit: do not mark any history version as current.
    fs::write(fixture.root.join("agent/settings.json"), b"external-edit").unwrap();
    let external =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    assert_eq!(external.current.status, HistoryCurrentStatus::Unrecorded);
    assert_eq!(external.current.matched_entry_key, None);
    assert!(external.current.files[0].readable);
    assert_ne!(a, b); // silence unused if any
}

#[test]
fn history_current_reports_unreadable_and_partial_multi_file_without_false_match() {
    let fixture = Fixture::new();
    let kind = OperationKind::PiSettings;
    fs::create_dir_all(fixture.root.join("pi")).unwrap();
    fs::write(fixture.root.join("pi/models.json"), b"models-a").unwrap();
    fs::write(fixture.root.join("pi/settings.json"), b"settings-a").unwrap();
    apply_at(
        &fixture,
        kind,
        "pi",
        &[
            ("models.json", b"models-b"),
            ("settings.json", b"settings-b"),
        ],
    );
    let scope = scope(&fixture, kind, "pi", &["models.json", "settings.json"]);
    let matched =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    assert_eq!(matched.current.status, HistoryCurrentStatus::Matched);
    assert_eq!(matched.current.files.len(), 2);

    // Only one of two files matches the retained version.
    fs::write(fixture.root.join("pi/settings.json"), b"settings-external").unwrap();
    let partial =
        configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
    assert_eq!(partial.current.status, HistoryCurrentStatus::Partial);
    assert_eq!(partial.current.matched_entry_key, None);

    // Replace settings with a symlink so the live read is unsafe.
    fs::remove_file(fixture.root.join("pi/settings.json")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("models.json", fixture.root.join("pi/settings.json")).unwrap();
        let unreadable =
            configuration_history(&fixture.storage, &scope, None, 10, &fixture.backups()).unwrap();
        assert_eq!(unreadable.current.status, HistoryCurrentStatus::Unreadable);
        assert_eq!(unreadable.current.matched_entry_key, None);
        assert!(unreadable.current.files.iter().any(|file| !file.readable));
    }
}
