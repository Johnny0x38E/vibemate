//! Fault and restart coverage for explicit unreferenced-copy cleanup.

use super::*;

fn inventory(fixture: &Fixture) -> BackupInventory {
    inspect_backup_copies(&fixture.storage, &fixture.backups()).unwrap()
}

fn orphan(fixture: &Fixture, id: &str, bytes: &[u8]) -> UnreferencedBackup {
    let copy = fixture.backups().store(id, "config", bytes).unwrap();
    UnreferencedBackup {
        id: id.to_owned(),
        file: Some(copy),
    }
}

fn cleanup(fixture: &Fixture, copies: &[UnreferencedBackup]) -> Vec<BackupCleanupOutcome> {
    cleanup_backup_copies(&fixture.storage, &fixture.backups(), copies).unwrap()
}

#[test]
fn pre_log_failure_is_discoverable_after_restart_and_cleanup_never_changes_target_or_history() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"synthetic-private-original").unwrap();
    fixture.sql("CREATE TRIGGER refuse_preparation BEFORE INSERT ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        fixture
            .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
            .unwrap_err(),
        WriteError::LogWriteFailed
    );
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let found = inspect_backup_copies(&reopened, &fixture.backups()).unwrap();
    assert_eq!(found.unreferenced.len(), 1);
    assert!(found.retained.is_empty());
    assert!(found.blocked.is_empty());
    let before = recent_operations(&reopened, OperationKind::PiSettings, 50).unwrap();
    let result = cleanup_backup_copies(&reopened, &fixture.backups(), &found.unreferenced).unwrap();
    assert!(result[0].error_code.is_none());
    assert!(inventory(&fixture).unreferenced.is_empty());
    assert_eq!(
        fs::read(fixture.root.join("a")).unwrap(),
        b"synthetic-private-original"
    );
    assert_eq!(
        recent_operations(&reopened, OperationKind::PiSettings, 50).unwrap(),
        before
    );
    assert_eq!(
        cleanup_backup_copies(&reopened, &fixture.backups(), &found.unreferenced).unwrap(),
        result
    );
    let metadata = serde_json::to_string(&found).unwrap();
    assert!(!metadata.contains("synthetic-private-original"));
}

#[test]
fn old_steps_and_revisions_independently_protect_copies_including_failed_and_interrupted_writes() {
    for reference in ["legacy", "revision", "prepared", "failed"] {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("a"), b"original").unwrap();
        let applied = fixture
            .apply(
                OperationKind::ClaudeCodeSettings,
                &[fixture.file("a", b"B")],
            )
            .unwrap();
        let backup = fixture.revisions()[0].backup.clone().unwrap();
        match reference {
            "legacy" => fixture.sql("DELETE FROM config_file_revision;"),
            "revision" => fixture.sql("UPDATE config_operation_step SET backup_id = NULL, backup_path = NULL, backup_digest = NULL;"),
            "prepared" => fixture.sql("UPDATE config_file_revision SET state = 'prepared'; UPDATE config_operation_step SET state = 'pending', applied_digest = NULL, previous_existed = NULL, backup_id = NULL, backup_path = NULL, backup_digest = NULL;"),
            "failed" => fixture.sql("UPDATE config_file_revision SET state = 'failed', error_code = 'write_failed'; UPDATE config_operation_step SET state = 'failed', error_code = 'write_failed', applied_digest = NULL, previous_existed = NULL, backup_id = NULL, backup_path = NULL, backup_digest = NULL;"),
            _ => unreachable!(),
        }
        let found = inventory(&fixture);
        assert!(found.unreferenced.is_empty(), "{reference}");
        assert_eq!(
            found.retained,
            vec![PathBuf::from(&backup).parent().unwrap().to_str().unwrap()]
        );
        assert_eq!(fs::read(backup).unwrap(), b"original");
        assert!(
            operation(&fixture.storage, &applied.operation_id)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn restore_and_compensation_snapshots_are_not_cleanup_candidates() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"A").unwrap();
    let applied = fixture
        .apply(OperationKind::GrokSettings, &[fixture.file("a", b"B")])
        .unwrap();
    fixture.restore(&applied.operation_id).unwrap();
    let files = [fixture.file("a", b"C"), fixture.file("b", b"new")];
    fs::write(fixture.root.join("b"), b"concurrent change").unwrap();
    let applied = fixture.apply(OperationKind::PiSettings, &files).unwrap();
    assert_eq!(applied.state, OperationState::Compensated);
    let found = inventory(&fixture);
    assert!(found.unreferenced.is_empty());
    assert_eq!(found.retained.len(), 4);
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"A");
}

#[test]
fn gained_reference_or_changed_bytes_rejects_the_whole_selection_before_deleting() {
    for change in ["bytes", "reference", "extra", "missing-file"] {
        let fixture = Fixture::new();
        let first = orphan(&fixture, "100-1-0", b"first");
        let second = orphan(&fixture, "100-1-1", b"second");
        let path = &second.file.as_ref().unwrap().path;
        match change {
            "bytes" => fs::write(path, b"changed").unwrap(),
            "extra" => fs::write(
                PathBuf::from(path).parent().unwrap().join("extra"),
                b"extra",
            )
            .unwrap(),
            "missing-file" => fs::remove_file(path).unwrap(),
            "reference" => {
                let plan = fixture.file("a", b"anything");
                let id = begin_operation(
                    &fixture.storage,
                    OperationKind::PiSettings,
                    None,
                    &[plan.target],
                )
                .unwrap();
                record_step_applied(
                    &fixture.storage,
                    &id,
                    0,
                    second.file.as_ref(),
                    &digest_hex(b"anything"),
                    true,
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            cleanup_backup_copies(
                &fixture.storage,
                &fixture.backups(),
                &[first.clone(), second]
            )
            .unwrap_err(),
            WriteError::PlanStale,
            "{change}"
        );
        assert_eq!(fs::read(first.file.unwrap().path).unwrap(), b"first");
    }
}

#[test]
fn referenced_identity_and_shared_directory_paths_both_block_cleanup() {
    let fixture = Fixture::new();
    let first = orphan(&fixture, "100-1-0", b"first");
    let second = orphan(&fixture, "100-1-1", b"second");
    let target = fixture.file("a", b"new").target;
    let id = begin_operation(&fixture.storage, OperationKind::PiSettings, None, &[target]).unwrap();
    let mismatched_reference = Backup {
        id: first.id.clone(),
        path: second.file.as_ref().unwrap().path.clone(),
        content_digest: digest_hex(b"second"),
    };
    record_step_applied(
        &fixture.storage,
        &id,
        0,
        Some(&mismatched_reference),
        &digest_hex(b"new"),
        true,
    )
    .unwrap();
    assert_eq!(inventory(&fixture).retained.len(), 2);
    for candidate in [first, second] {
        assert_eq!(
            cleanup_backup_copies(&fixture.storage, &fixture.backups(), &[candidate]).unwrap_err(),
            WriteError::PlanStale
        );
    }
}

#[test]
fn unknown_layouts_and_large_files_are_blocked_without_recursive_deletion() {
    let fixture = Fixture::new();
    let backups = fixture.backups();
    fs::create_dir_all(backups.root().join("unknown")).unwrap();
    fs::create_dir_all(backups.root().join("100-1-0/nested")).unwrap();
    orphan(&fixture, "100-1-1", b"first");
    fs::write(backups.root().join("100-1-1/extra"), b"extra").unwrap();
    orphan(&fixture, "100-1-2", &vec![0; MAX_TARGET_BYTES as usize + 1]);
    fs::write(backups.root().join("100-1-3"), b"not a directory").unwrap();
    let found = inventory(&fixture);
    assert_eq!(found.blocked.len(), 5);
    assert!(found.unreferenced.is_empty());
    assert!(backups.root().join("100-1-0/nested").is_dir());
}

#[test]
fn malformed_or_unreadable_references_fail_closed_before_any_deletion() {
    for change in ["relative", "traversal", "missing-table"] {
        let fixture = Fixture::new();
        let candidate = orphan(&fixture, "100-1-0", b"keep");
        let id = begin_operation(
            &fixture.storage,
            OperationKind::PiSettings,
            None,
            &[fixture.file("a", b"B").target],
        )
        .unwrap();
        if change == "missing-table" {
            fixture.sql("ALTER TABLE config_file_revision RENAME TO hidden_revisions;");
        } else {
            let mut copy = candidate.file.clone().unwrap();
            copy.path = if change == "relative" {
                "relative/config".to_owned()
            } else {
                fixture
                    .backups()
                    .root()
                    .join("other/../100-1-0/config")
                    .to_str()
                    .unwrap()
                    .to_owned()
            };
            record_step_applied(
                &fixture.storage,
                &id,
                0,
                Some(&copy),
                &digest_hex(b"B"),
                true,
            )
            .unwrap();
        }
        let error = if change == "missing-table" {
            WriteError::LogReadFailed
        } else {
            WriteError::InvalidStoredOperation
        };
        assert_eq!(
            inspect_backup_copies(&fixture.storage, &fixture.backups()).unwrap_err(),
            error
        );
        assert_eq!(
            cleanup_backup_copies(
                &fixture.storage,
                &fixture.backups(),
                std::slice::from_ref(&candidate)
            )
            .unwrap_err(),
            error
        );
        assert_eq!(fs::read(candidate.file.unwrap().path).unwrap(), b"keep");
    }
}

#[test]
fn missing_store_stays_absent_and_invalid_approvals_cannot_select_arbitrary_paths() {
    let fixture = Fixture::new();
    assert!(inventory(&fixture).unreferenced.is_empty());
    assert!(!fixture.backups().root().exists());
    let candidate = orphan(&fixture, "100-1-0", b"keep");
    let mut outside = candidate.clone();
    outside.file.as_mut().unwrap().path = fixture.root.join("outside").to_str().unwrap().to_owned();
    let traversal = UnreferencedBackup {
        id: "../outside".to_owned(),
        file: None,
    };
    for selection in [
        vec![],
        vec![candidate.clone(), candidate.clone()],
        vec![outside],
        vec![traversal],
    ] {
        assert_eq!(
            cleanup_backup_copies(&fixture.storage, &fixture.backups(), &selection).unwrap_err(),
            WriteError::InvalidRequest
        );
    }
    assert_eq!(fs::read(candidate.file.unwrap().path).unwrap(), b"keep");
}

#[test]
fn an_empty_directory_after_interrupted_cleanup_is_rediscovered_after_restart() {
    let fixture = Fixture::new();
    let candidate = orphan(&fixture, "100-1-0", b"synthetic-private");
    // Simulate termination between unlinking the sole file and its directory.
    fs::remove_file(&candidate.file.unwrap().path).unwrap();
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let found = inspect_backup_copies(&reopened, &fixture.backups()).unwrap();
    assert_eq!(
        found.unreferenced,
        vec![UnreferencedBackup {
            id: candidate.id,
            file: None
        }]
    );
    assert!(
        cleanup_backup_copies(&reopened, &fixture.backups(), &found.unreferenced).unwrap()[0]
            .error_code
            .is_none()
    );
    assert!(inventory(&fixture).unreferenced.is_empty());
}

#[cfg(unix)]
#[test]
fn linked_root_directory_and_file_are_never_followed_or_deleted() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let external = fixture.root.join("external");
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("config"), b"external-private").unwrap();
    symlink(&external, fixture.backups().root()).unwrap();
    assert_eq!(
        inspect_backup_copies(&fixture.storage, &fixture.backups()).unwrap_err(),
        WriteError::BackupUnavailable
    );
    fs::remove_file(fixture.backups().root()).unwrap();
    fs::create_dir(fixture.backups().root()).unwrap();
    symlink(&external, fixture.backups().root().join("100-1-0")).unwrap();
    fs::create_dir(fixture.backups().root().join("100-1-1")).unwrap();
    symlink(
        external.join("config"),
        fixture.backups().root().join("100-1-1/config"),
    )
    .unwrap();
    let found = inventory(&fixture);
    assert_eq!(found.blocked.len(), 2);
    assert!(found.unreferenced.is_empty());
    let forged = UnreferencedBackup {
        id: "100-1-0".to_owned(),
        file: None,
    };
    assert_eq!(
        cleanup_backup_copies(&fixture.storage, &fixture.backups(), &[forged]).unwrap_err(),
        WriteError::PlanStale
    );
    let candidate = orphan(&fixture, "100-1-2", b"reviewed");
    let file = &candidate.file.as_ref().unwrap().path;
    fs::remove_file(file).unwrap();
    symlink(external.join("config"), file).unwrap();
    assert_eq!(
        cleanup_backup_copies(&fixture.storage, &fixture.backups(), &[candidate]).unwrap_err(),
        WriteError::PlanStale
    );
    assert_eq!(
        fs::read(external.join("config")).unwrap(),
        b"external-private"
    );
}

#[cfg(unix)]
#[test]
fn cleanup_failure_leaves_a_discoverable_copy_and_other_selected_copies_can_finish() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let first = orphan(&fixture, "100-1-0", b"first");
    let second = orphan(&fixture, "100-1-1", b"second");
    let directory = fixture.backups().root().join(&first.id);
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o500)).unwrap();
    let result = cleanup(&fixture, &[first.clone(), second]);
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result[0].error_code, Some(WriteError::BackupUnavailable));
    assert!(result[1].error_code.is_none());
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let found = inspect_backup_copies(&reopened, &fixture.backups()).unwrap();
    assert_eq!(found.unreferenced, vec![first]);
    assert!(
        cleanup_backup_copies(&reopened, &fixture.backups(), &found.unreferenced).unwrap()[0]
            .error_code
            .is_none()
    );
}

#[test]
fn inspection_and_cleanup_wait_for_active_cross_instance_configuration_writes() {
    use std::{sync::mpsc, time::Duration};
    for delete in [false, true] {
        let fixture = Fixture::new();
        let candidate = orphan(&fixture, "100-1-0", b"active");
        let reopened = Storage::open_in_directory(&fixture.root).unwrap();
        let active_copy = candidate.file.clone().unwrap();
        let held = fixture.storage.lock_configuration().unwrap();
        std::thread::scope(|scope| {
            let (started_tx, started_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            let backups = fixture.backups();
            scope.spawn(move || {
                started_tx.send(()).unwrap();
                let result = if delete {
                    cleanup_backup_copies(&reopened, &backups, &[candidate]).map(|_| true)
                } else {
                    inspect_backup_copies(&reopened, &backups)
                        .map(|found| found.unreferenced.is_empty() && found.retained.len() == 1)
                };
                done_tx.send(result).unwrap();
            });
            started_rx.recv().unwrap();
            assert_eq!(
                done_rx.recv_timeout(Duration::from_millis(100)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            assert!(fixture.backups().root().join("100-1-0/config").exists());
            // The active writer acquires a durable reference before releasing
            // its lock. A waiting inspector/cleaner must see the new reference.
            let id = begin_operation(
                &fixture.storage,
                OperationKind::PiSettings,
                None,
                &[fixture.file("a", b"new").target],
            )
            .unwrap();
            record_step_applied(
                &fixture.storage,
                &id,
                0,
                Some(&active_copy),
                &digest_hex(b"new"),
                true,
            )
            .unwrap();
            drop(held);
            let result = done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(
                result,
                if delete {
                    Err(WriteError::PlanStale)
                } else {
                    Ok(true)
                }
            );
            assert!(fixture.backups().root().join("100-1-0/config").exists());
        });
    }
}
