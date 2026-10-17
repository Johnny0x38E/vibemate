//! Resolution fault tests reuse real-file snapshot fixtures from revision tests.

use super::*;

fn plan(fixture: &Fixture, id: &str) -> RevisionResolutionPlan {
    plan_revision_resolution(&fixture.storage, id, &fixture.backups())
        .unwrap()
        .unwrap()
}

fn resolve(
    fixture: &Fixture,
    reviewed: &RevisionResolutionPlan,
) -> Result<OperationState, WriteError> {
    resolve_interrupted_revisions(
        &fixture.storage,
        &reviewed.operation_id,
        reviewed,
        &fixture.backups(),
    )
}

fn audit_count(fixture: &Fixture) -> i64 {
    fixture
        .storage
        .lock()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM config_revision_resolution",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn crash_apply(fixture: &Fixture, kind: OperationKind, existed: bool, legacy: bool) -> String {
    if existed {
        fs::write(fixture.root.join("a"), b"old").unwrap();
    }
    if legacy {
        fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_operation_step WHEN NEW.state = 'applied' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    } else {
        fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    }
    assert_eq!(
        fixture
            .apply(kind, &[fixture.file("a", b"new")])
            .unwrap_err(),
        WriteError::LogWriteFailed
    );
    fixture.sql("DROP TRIGGER refuse_result;");
    fixture.revisions()[0].operation.clone()
}

#[test]
fn resolving_either_apply_crash_window_repairs_history_and_allows_real_restore_after_restart() {
    for kind in [
        OperationKind::ClaudeCodeSettings,
        OperationKind::PiSettings,
        OperationKind::GrokSettings,
    ] {
        for existed in [false, true] {
            for legacy in [false, true] {
                let fixture = Fixture::new();
                let id = crash_apply(&fixture, kind, existed, legacy);
                assert_eq!(
                    operation(&fixture.storage, &id).unwrap().unwrap().steps[0].state,
                    StepState::Pending
                );
                assert_eq!(
                    compensate_operation(&fixture.storage, &id, &fixture.backups()).unwrap_err(),
                    WriteError::OutcomeUnknown
                );
                let approval = plan(&fixture, &id);
                assert_eq!(
                    approval.revisions[0].state,
                    if legacy { "completed" } else { "prepared" }
                );
                let reopened = Storage::open_in_directory(&fixture.root).unwrap();
                assert_eq!(
                    resolve_interrupted_revisions(&reopened, &id, &approval, &fixture.backups())
                        .unwrap(),
                    OperationState::Applied
                );
                let repaired = operation(&reopened, &id).unwrap().unwrap();
                assert_eq!(repaired.steps[0].previous_existed, Some(existed));
                assert_eq!(repaired.steps[0].backup.is_some(), existed);
                assert_eq!(repaired.steps[0].applied_digest, Some(digest_hex(b"new")));
                assert_eq!(audit_count(&fixture), 1);
                assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
                assert!(inspect(&fixture, &id).is_empty());
                assert_eq!(
                    resolve(&fixture, &approval).unwrap_err(),
                    WriteError::PlanStale
                );
                fixture.restore(&id).unwrap();
                assert_eq!(fixture.root.join("a").exists(), existed);
                if existed {
                    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"old");
                }
                let audit: (String, String, Option<String>) = fixture.storage.lock().unwrap().query_row(
                    "SELECT previous_state, matched_state, observed_digest FROM config_revision_resolution", [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                ).unwrap();
                assert_eq!(
                    audit,
                    (
                        if legacy { "completed" } else { "prepared" }.to_owned(),
                        "intended".to_owned(),
                        Some(digest_hex(b"new"))
                    )
                );
            }
        }
    }
}

#[test]
fn resolving_the_previous_state_records_observation_without_claiming_a_write_never_happened() {
    for existed in [false, true] {
        let fixture = Fixture::new();
        let id = interrupted_apply(&fixture, existed);
        // An external change can also put the pre-image back. Resolution records
        // the observed state, not authorship or absence of a transient write.
        if existed {
            fs::write(fixture.root.join("a"), b"old").unwrap();
        } else {
            fs::remove_file(fixture.root.join("a")).unwrap();
        }
        fixture.sql(
            "UPDATE config_file_revision SET state = 'unknown', error_code = 'outcome_unknown';",
        );
        let approval = plan(&fixture, &id);
        assert_eq!(
            resolve(&fixture, &approval).unwrap(),
            OperationState::Failed
        );
        let step = operation(&fixture.storage, &id)
            .unwrap()
            .unwrap()
            .steps
            .remove(0);
        assert_eq!(step.state, StepState::Failed);
        assert_eq!(step.backup, None);
        assert_eq!(step.applied_digest, None);
        assert_eq!(fixture.revisions()[0].state, "failed");
        assert_eq!(audit_count(&fixture), 1);
        assert!(inspect(&fixture, &id).is_empty());
        assert_eq!(fixture.root.join("a").exists(), existed);
        if existed {
            assert_eq!(
                fs::read(fixture.revisions()[0].backup.as_ref().unwrap()).unwrap(),
                b"old"
            );
        }
    }
}

#[test]
fn resolving_restore_and_compensation_never_replays_them_in_reverse() {
    for intent in ["restore", "compensation"] {
        for existed in [false, true] {
            for legacy in [false, true] {
                let fixture = Fixture::new();
                if existed {
                    fs::write(fixture.root.join("a"), b"old").unwrap();
                }
                let applied = fixture
                    .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
                    .unwrap();
                if legacy {
                    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_operation_step WHEN NEW.restore_state = 'restored' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
                } else {
                    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision WHEN OLD.intent != 'apply' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
                }
                let result = if intent == "restore" {
                    fixture.restore(&applied.operation_id).map(|_| ())
                } else {
                    finish_operation(
                        &fixture.storage,
                        &applied.operation_id,
                        OperationState::Partial,
                    )
                    .unwrap();
                    compensate_operation(
                        &fixture.storage,
                        &applied.operation_id,
                        &fixture.backups(),
                    )
                    .map(|_| ())
                };
                assert_eq!(result.unwrap_err(), WriteError::LogWriteFailed);
                fixture.sql("DROP TRIGGER refuse_result;");
                let approval = plan(&fixture, &applied.operation_id);
                assert_eq!(approval.revisions.len(), 1);
                assert_eq!(
                    resolve(&fixture, &approval).unwrap(),
                    OperationState::Compensated
                );
                let record = operation(&fixture.storage, &applied.operation_id)
                    .unwrap()
                    .unwrap();
                assert_eq!(record.steps[0].restore_state, Some(RestoreState::Restored));
                assert_eq!(fixture.root.join("a").exists(), existed);
                if existed {
                    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"old");
                }
                assert_eq!(
                    compensate_operation(
                        &fixture.storage,
                        &applied.operation_id,
                        &fixture.backups()
                    )
                    .unwrap_err(),
                    WriteError::InvalidRequest
                );
                assert_eq!(fixture.revisions().len(), 2);
                assert_eq!(
                    fs::read(fixture.revisions()[1].backup.as_ref().unwrap()).unwrap(),
                    b"new"
                );
            }
        }
    }
}

#[test]
fn a_restore_observed_at_its_previous_state_can_be_retried_after_resolution() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("a"), b"old").unwrap();
    let applied = fixture
        .apply(OperationKind::PiSettings, &[fixture.file("a", b"new")])
        .unwrap();
    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision WHEN OLD.intent = 'restore' BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        fixture.restore(&applied.operation_id).unwrap_err(),
        WriteError::LogWriteFailed
    );
    fixture.sql("DROP TRIGGER refuse_result;");
    fs::write(fixture.root.join("a"), b"new").unwrap();
    assert_eq!(
        resolve(&fixture, &plan(&fixture, &applied.operation_id)).unwrap(),
        OperationState::Partial
    );
    assert_eq!(
        operation(&fixture.storage, &applied.operation_id)
            .unwrap()
            .unwrap()
            .steps[0]
            .restore_state,
        Some(RestoreState::Failed)
    );
    fixture.restore(&applied.operation_id).unwrap();
    assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"old");
    assert_eq!(fixture.revisions().len(), 3);
}

#[test]
fn partial_apply_resolution_keeps_unattempted_files_out_of_compensation() {
    let fixture = Fixture::new();
    for name in ["a", "b", "c"] {
        fs::write(fixture.root.join(name), name.as_bytes()).unwrap();
    }
    fixture.sql("CREATE TRIGGER refuse_result BEFORE UPDATE ON config_file_revision WHEN OLD.ordinal = 1 BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        fixture
            .apply(
                OperationKind::PiSettings,
                &[
                    fixture.file("a", b"A"),
                    fixture.file("b", b"B"),
                    fixture.file("c", b"C")
                ]
            )
            .unwrap_err(),
        WriteError::LogWriteFailed
    );
    fixture.sql("DROP TRIGGER refuse_result;");
    let id = fixture.revisions()[0].operation.clone();
    assert_eq!(
        resolve(&fixture, &plan(&fixture, &id)).unwrap(),
        OperationState::Partial
    );
    assert_eq!(
        operation(&fixture.storage, &id).unwrap().unwrap().steps[2].state,
        StepState::Pending
    );
    assert_eq!(
        compensate_operation(&fixture.storage, &id, &fixture.backups()).unwrap(),
        OperationState::Compensated
    );
    for name in ["a", "b", "c"] {
        assert_eq!(fs::read(fixture.root.join(name)).unwrap(), name.as_bytes());
    }
}

#[test]
fn resolution_approval_expires_on_any_file_operation_or_candidate_change() {
    for change in 0..5 {
        let fixture = Fixture::new();
        let id = interrupted_apply(&fixture, true);
        let approval = plan(&fixture, &id);
        match change {
            0 => fs::write(fixture.root.join("a"), b"old").unwrap(),
            1 => fixture.sql("UPDATE config_operation SET owner_ref = 'changed'"),
            2 => fixture.sql("UPDATE config_file_revision SET batch_id = 'changed'"),
            3 => fixture.sql(
                "UPDATE config_file_revision SET state = 'unknown', error_code = 'outcome_unknown'",
            ),
            _ => fs::remove_file(fixture.revisions()[0].backup.as_ref().unwrap()).unwrap(),
        }
        assert_eq!(
            resolve(&fixture, &approval).unwrap_err(),
            WriteError::PlanStale
        );
        assert_eq!(audit_count(&fixture), 0);
        assert_eq!(
            operation(&fixture.storage, &id).unwrap().unwrap().steps[0].state,
            StepState::Pending
        );
    }
}

#[test]
fn partial_duplicate_cross_operation_and_empty_approvals_are_refused() {
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let approval = plan(&fixture, &id);
    let mut duplicate = approval.clone();
    duplicate.revisions.push(duplicate.revisions[0].clone());
    assert_eq!(
        resolve(&fixture, &duplicate).unwrap_err(),
        WriteError::PlanStale
    );
    let mut empty = approval.clone();
    empty.revisions.clear();
    assert_eq!(
        resolve(&fixture, &empty).unwrap_err(),
        WriteError::PlanStale
    );
    let second = fixture
        .apply(
            OperationKind::ClaudeCodeSettings,
            &[fixture.file("other", b"other")],
        )
        .unwrap();
    assert_eq!(
        resolve_interrupted_revisions(
            &fixture.storage,
            &second.operation_id,
            &approval,
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::PlanStale
    );
    assert_eq!(audit_count(&fixture), 0);
}

#[test]
fn damaged_unreadable_diverged_and_ambiguous_observations_do_not_unlock_recovery() {
    for case in 0..4 {
        let fixture = Fixture::new();
        let id = interrupted_apply(&fixture, true);
        let expected = match case {
            0 => {
                fs::write(fixture.revisions()[0].backup.as_ref().unwrap(), b"damaged").unwrap();
                WriteError::BackupUnavailable
            }
            1 => {
                fs::write(fixture.root.join("a"), b"external").unwrap();
                WriteError::OutcomeUnknown
            }
            2 => {
                fs::remove_file(fixture.root.join("a")).unwrap();
                fs::create_dir(fixture.root.join("a")).unwrap();
                WriteError::OutcomeUnknown
            }
            _ => {
                fixture
                    .storage
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE config_file_revision SET next_digest = previous_digest",
                        [],
                    )
                    .unwrap();
                fs::write(fixture.root.join("a"), b"old").unwrap();
                WriteError::OutcomeUnknown
            }
        };
        assert_eq!(
            resolve(&fixture, &plan(&fixture, &id)).unwrap_err(),
            expected
        );
        assert_eq!(fixture.revisions()[0].state, "prepared");
        assert_eq!(audit_count(&fixture), 0);
    }
}

#[test]
fn completed_results_are_not_reclassified_as_failed_when_an_external_writer_restores_the_pre_image()
{
    let fixture = Fixture::new();
    let id = crash_apply(&fixture, OperationKind::PiSettings, true, true);
    fs::write(fixture.root.join("a"), b"old").unwrap();
    assert_eq!(
        resolve(&fixture, &plan(&fixture, &id)).unwrap_err(),
        WriteError::OutcomeUnknown
    );
    assert_eq!(fixture.revisions()[0].state, "completed");
    assert_eq!(audit_count(&fixture), 0);
}

#[test]
fn an_older_uncertain_revision_cannot_override_a_later_mutation_of_the_same_step() {
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    fixture.sql(
        "INSERT INTO config_file_revision
        SELECT 'later', batch_id, operation_id, ordinal, intent, target_path,
        previous_digest, next_digest, backup_path, 'failed', 'write_failed', created_at, updated_at
        FROM config_file_revision;",
    );
    assert_eq!(
        resolve(&fixture, &plan(&fixture, &id)).unwrap_err(),
        WriteError::OutcomeUnknown
    );
    assert_eq!(audit_count(&fixture), 0);
}

#[test]
fn audit_revision_step_and_operation_updates_roll_back_together_on_failure() {
    for table in [
        "config_revision_resolution",
        "config_file_revision",
        "config_operation_step",
        "config_operation",
    ] {
        let fixture = Fixture::new();
        let id = interrupted_apply(&fixture, true);
        let approval = plan(&fixture, &id);
        let before = operation(&fixture.storage, &id).unwrap();
        let verb = if table == "config_revision_resolution" {
            "INSERT"
        } else {
            "UPDATE"
        };
        fixture.sql(&format!("CREATE TRIGGER refuse_resolution BEFORE {verb} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;"));
        assert_eq!(
            resolve(&fixture, &approval).unwrap_err(),
            WriteError::LogWriteFailed
        );
        assert_eq!(operation(&fixture.storage, &id).unwrap(), before);
        assert_eq!(fixture.revisions()[0].state, "prepared");
        assert_eq!(audit_count(&fixture), 0);
        assert_eq!(fs::read(fixture.root.join("a")).unwrap(), b"new");
        fixture.sql("DROP TRIGGER refuse_resolution;");
        assert_eq!(
            resolve(&fixture, &approval).unwrap(),
            OperationState::Applied
        );
    }
}

#[test]
fn another_storage_instance_cannot_resolve_while_a_configuration_writer_holds_the_lock() {
    use std::{sync::mpsc, time::Duration};
    let fixture = Fixture::new();
    let id = interrupted_apply(&fixture, true);
    let approval = plan(&fixture, &id);
    let reopened = Storage::open_in_directory(&fixture.root).unwrap();
    let held = fixture.storage.lock_configuration().unwrap();
    std::thread::scope(|scope| {
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let backups = fixture.backups();
        scope.spawn(move || {
            started_tx.send(()).unwrap();
            let result = resolve_interrupted_revisions(&reopened, &id, &approval, &backups);
            done_tx.send(result).unwrap();
        });
        started_rx.recv().unwrap();
        assert_eq!(
            done_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        assert_eq!(audit_count(&fixture), 0);
        assert_eq!(fixture.revisions()[0].state, "prepared");
        drop(held);
        assert_eq!(
            done_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            OperationState::Applied
        );
    });
    // The lock's inode remains; removing a lock file would break coordination.
    assert!(fixture.root.join("vibemate.config-lock").is_file());
}

#[test]
fn one_bad_file_or_failed_audit_rolls_back_the_entire_multi_file_resolution() {
    let fixture = Fixture::new();
    for name in ["a", "b"] {
        fs::write(fixture.root.join(name), b"old").unwrap();
    }
    let applied = fixture
        .apply(
            OperationKind::PiSettings,
            &[fixture.file("a", b"new"), fixture.file("b", b"new")],
        )
        .unwrap();
    fixture.sql("UPDATE config_file_revision SET state = 'prepared';");
    let approval = plan(&fixture, &applied.operation_id);
    assert_eq!(approval.revisions.len(), 2);
    let mut partial = approval.clone();
    partial.revisions.pop();
    assert_eq!(
        resolve(&fixture, &partial).unwrap_err(),
        WriteError::PlanStale
    );
    fixture.sql("CREATE TRIGGER refuse_second BEFORE INSERT ON config_revision_resolution WHEN (SELECT count(*) FROM config_revision_resolution) = 1 BEGIN SELECT RAISE(ABORT, 'injected'); END;");
    assert_eq!(
        resolve(&fixture, &approval).unwrap_err(),
        WriteError::LogWriteFailed
    );
    assert_eq!(audit_count(&fixture), 0);
    assert!(
        fixture
            .revisions()
            .iter()
            .all(|row| row.state == "prepared")
    );
    fixture.sql("DROP TRIGGER refuse_second;");
    assert_eq!(
        resolve(&fixture, &approval).unwrap(),
        OperationState::Applied
    );
    assert_eq!(audit_count(&fixture), 2);
}
