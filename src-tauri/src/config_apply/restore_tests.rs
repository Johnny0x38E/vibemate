//! Restore tests: what a preview classifies, what a confirmed restore writes,
//! and what a retry does after a partial rollback. Every fixture is a temporary
//! directory removed when the test ends.
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
            "vibemate-config-restore-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let storage = crate::storage::Storage::open_in_directory(&root).unwrap();
        Self { root, storage }
    }

    fn backups(&self) -> BackupStore {
        BackupStore::new(self.root.join("backups"))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn write(&self, name: &str, content: &str) {
        fs::write(self.path(name), content).unwrap();
    }

    fn content(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap()
    }

    /// A target whose token matches the file as it is now.
    fn target(&self, name: &str) -> WriteTarget {
        WriteTarget::from_plan(&read_target_document(&self.path(name)).unwrap().target)
    }

    /// A file to hand to an apply step.
    fn file(&self, name: &str, content: &str) -> PlannedFile {
        PlannedFile {
            target: self.target(name),
            content: content.as_bytes().to_vec(),
        }
    }

    /// Apply one file through the real write path and record it.
    fn apply(&self, name: &str, content: &str, owner: Option<&str>) -> AppliedOperation {
        self.apply_all(&[(name, content)], owner)
    }

    /// Apply several files as one operation.
    fn apply_all(&self, files: &[(&str, &str)], owner: Option<&str>) -> AppliedOperation {
        let planned: Vec<PlannedFile> = files
            .iter()
            .map(|(name, content)| self.file(name, content))
            .collect();
        apply_files(
            &self.storage,
            OperationKind::ClaudeCodeSettings,
            owner,
            &planned,
            &self.backups(),
        )
        .unwrap()
    }

    /// Begin an operation by hand, the way a crash between steps would.
    fn begin(&self, names: &[&str]) -> String {
        let targets: Vec<WriteTarget> = names.iter().map(|name| self.target(name)).collect();
        begin_operation(
            &self.storage,
            OperationKind::ClaudeCodeSettings,
            None,
            &targets,
        )
        .unwrap()
    }

    fn plan(&self, operation_id: &str) -> RestorePlan {
        plan_restore(&self.storage, operation_id)
            .unwrap()
            .expect("recorded operation")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The tokens the interface would send back for a preview.
fn reviewed(plan: &RestorePlan) -> Vec<ReviewedRestoreStep> {
    plan.steps
        .iter()
        .filter(|step| step.action.changes_files())
        .map(|step| ReviewedRestoreStep {
            ordinal: step.ordinal,
            current_digest: step.current_digest.clone(),
        })
        .collect()
}

/// The preview line for one path.
fn step<'a>(plan: &'a RestorePlan, name: &str) -> &'a RestoreStepPlan {
    plan.steps
        .iter()
        .find(|step| step.target_path.ends_with(name))
        .expect("step for the path")
}

#[test]
fn a_preview_classifies_every_step_from_the_file_as_it_is_now() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"model\":\"one\"}\n");
    // One operation that replaces a file and creates another.
    let applied = fixture.apply_all(
        &[
            ("settings.json", "{\"model\":\"two\"}\n"),
            ("models.json", "{\"models\":[]}\n"),
        ],
        Some("0123456789abcdef0123456789abcdef"),
    );
    let plan = fixture.plan(&applied.operation_id);
    assert_eq!(plan.state, OperationState::Applied);
    assert_eq!(
        plan.owner_ref.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert!(plan.restorable);
    assert_eq!(step(&plan, "settings.json").action, RestoreAction::Restore);
    assert_eq!(step(&plan, "settings.json").reason, None);
    assert_eq!(step(&plan, "models.json").action, RestoreAction::Remove);

    // A file the user edited after the write is not a restore any more.
    let edited = fixture.apply("settings.json", "{\"model\":\"three\"}\n", None);
    fixture.write("settings.json", "{\"model\":\"three-plus\"}\n");
    let plan = fixture.plan(&edited.operation_id);
    let conflicted = step(&plan, "settings.json");
    assert_eq!(conflicted.action, RestoreAction::Conflict);
    assert_eq!(conflicted.reason.as_deref(), Some("target_changed"));
    // The write did happen; it is the file that moved on afterwards.
    assert!(conflicted.applied_digest.is_some());
    assert_ne!(conflicted.current_digest, conflicted.applied_digest);
    assert!(!plan.restorable);
}

#[test]
fn a_step_without_a_copy_is_removed_only_when_the_log_says_it_created_the_file() {
    let fixture = Fixture::new();
    let created = fixture.apply("models.json", "{\"models\":[]}\n", None);
    // The no-op case: the planned content was already in the file, so the step
    // keeps no copy although the file existed before it.
    fixture.write("settings.json", "{\"model\":\"same\"}\n");
    let unchanged = fixture.apply("settings.json", "{\"model\":\"same\"}\n", None);
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"same\"}\n");

    let plan = fixture.plan(&created.operation_id);
    assert_eq!(step(&plan, "models.json").action, RestoreAction::Remove);
    assert!(plan.restorable);

    let plan = fixture.plan(&unchanged.operation_id);
    assert_eq!(step(&plan, "settings.json").action, RestoreAction::Skip);
    assert!(!plan.restorable, "nothing to undo for an unchanged write");

    // A row an older build wrote cannot tell the two cases apart, so the restore
    // refuses instead of deleting a file that may have been there before.
    {
        let connection = fixture.storage.lock().unwrap();
        connection
            .execute_batch("PRAGMA ignore_check_constraints = ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE config_operation_step SET previous_existed = NULL WHERE operation_id = ?1",
                [&created.operation_id],
            )
            .unwrap();
    }
    let plan = fixture.plan(&created.operation_id);
    assert_eq!(step(&plan, "models.json").action, RestoreAction::Blocked);
    assert_eq!(
        step(&plan, "models.json").reason.as_deref(),
        Some("restore_history_unknown")
    );
    assert!(!plan.restorable);
}

#[test]
fn a_restore_puts_the_previous_bytes_back_and_keeps_the_original_copy() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"model\":\"one\"}\n");
    let applied = fixture.apply("settings.json", "{\"model\":\"two\"}\n", None);
    let record = operation(&fixture.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    let original_copy = record.steps[0].backup.clone().unwrap();

    let plan = fixture.plan(&applied.operation_id);
    let outcome = apply_restore(
        &fixture.storage,
        &applied.operation_id,
        &reviewed(&plan),
        &fixture.backups(),
    )
    .unwrap();

    assert_eq!(outcome.state, OperationState::Compensated);
    assert_eq!(outcome.steps.len(), 1);
    assert_eq!(outcome.steps[0].action, RestoreAction::Restore);
    assert_eq!(outcome.steps[0].state, RestoreState::Restored);
    assert!(outcome.leftovers.is_empty());
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"one\"}\n");
    // The operation now reads as undone, and the copy it replaced stays for a
    // later audit or another restore.
    let record = operation(&fixture.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.state, OperationState::Compensated);
    assert_eq!(record.steps[0].restore_state, Some(RestoreState::Restored));
    assert!(Path::new(&original_copy.path).exists());
    // The rollback's own copy of what it replaced has no referent, so it is gone.
    let directories: Vec<_> = fs::read_dir(fixture.backups().root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(
        directories,
        vec![fixture.backups().root().join(&original_copy.id)]
    );
}

#[test]
fn a_restore_removes_a_file_the_operation_created() {
    let fixture = Fixture::new();
    let applied = fixture.apply("models.json", "{\"models\":[]}\n", None);
    let plan = fixture.plan(&applied.operation_id);
    let outcome = apply_restore(
        &fixture.storage,
        &applied.operation_id,
        &reviewed(&plan),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(outcome.state, OperationState::Compensated);
    assert_eq!(outcome.steps[0].action, RestoreAction::Remove);
    assert!(!fixture.path("models.json").exists());
}

#[test]
fn a_conflict_is_left_alone_and_keeps_the_operation_partial() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"model\":\"one\"}\n");
    fixture.write("models.json", "{\"models\":[]}\n");
    let applied = fixture.apply_all(
        &[
            ("settings.json", "{\"model\":\"two\"}\n"),
            ("models.json", "{\"models\":[\"one\"]}\n"),
        ],
        None,
    );
    fixture.write("models.json", "{\"models\":[\"edited\"]}\n");

    let plan = fixture.plan(&applied.operation_id);
    assert_eq!(step(&plan, "models.json").action, RestoreAction::Conflict);
    let outcome = apply_restore(
        &fixture.storage,
        &applied.operation_id,
        &reviewed(&plan),
        &fixture.backups(),
    )
    .unwrap();

    assert_eq!(outcome.state, OperationState::Partial);
    assert_eq!(
        outcome.steps.len(),
        1,
        "only the restorable step is attempted"
    );
    assert_eq!(outcome.steps[0].state, RestoreState::Restored);
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"one\"}\n");
    assert_eq!(
        fixture.content("models.json"),
        "{\"models\":[\"edited\"]}\n"
    );
    let record = operation(&fixture.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.state, OperationState::Partial);
    assert_eq!(record.steps[1].restore_state, None);
}

#[test]
fn a_second_attempt_continues_a_partly_rolled_back_operation() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"model\":\"one\"}\n");
    fixture.write("models.json", "{\"models\":[]}\n");
    let applied = fixture.apply_all(
        &[
            ("settings.json", "{\"model\":\"two\"}\n"),
            ("models.json", "{\"models\":[\"one\"]}\n"),
        ],
        None,
    );
    fixture.write("models.json", "{\"models\":[\"edited\"]}\n");

    let plan = fixture.plan(&applied.operation_id);
    let first = apply_restore(
        &fixture.storage,
        &applied.operation_id,
        &reviewed(&plan),
        &fixture.backups(),
    )
    .unwrap();
    assert_eq!(first.state, OperationState::Partial);
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"one\"}\n");

    // The user resolves the conflict by putting the written content back, and
    // asks again. The step already restored must be skipped, not restored twice:
    // a second attempt at it would see the previous content and call it a
    // conflict, leaving the operation partial forever.
    fixture.write("models.json", "{\"models\":[\"one\"]}\n");
    let plan = fixture.plan(&applied.operation_id);
    assert_eq!(step(&plan, "settings.json").action, RestoreAction::Skip);
    let second = apply_restore(
        &fixture.storage,
        &applied.operation_id,
        &reviewed(&plan),
        &fixture.backups(),
    )
    .unwrap();

    assert_eq!(second.state, OperationState::Compensated);
    assert_eq!(second.steps.len(), 1);
    assert!(second.steps[0].target_path.ends_with("models.json"));
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"one\"}\n");
    assert_eq!(fixture.content("models.json"), "{\"models\":[]}\n");
}

#[test]
fn compensating_twice_continues_instead_of_failing_on_a_restored_step() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"model\":\"one\"}\n");
    fixture.write("models.json", "{\"models\":[]}\n");
    // An unfinished operation, the way a crash partway through an apply leaves it.
    let id = fixture.begin(&["settings.json", "models.json"]);
    let backups = fixture.backups();
    for (ordinal, (name, content)) in [
        ("settings.json", "{\"model\":\"two\"}\n"),
        ("models.json", "{\"models\":[\"one\"]}\n"),
    ]
    .iter()
    .enumerate()
    {
        let target = fixture.target(name);
        let applied = apply_file(&target, content.as_bytes(), &backups).unwrap();
        record_step_applied(
            &fixture.storage,
            &id,
            u32::try_from(ordinal).unwrap(),
            applied.backup.as_ref(),
            applied.target.content_digest.as_deref().unwrap(),
            true,
        )
        .unwrap();
    }
    fixture.write("models.json", "{\"models\":[\"edited\"]}\n");

    let state = compensate_operation(&fixture.storage, &id, &backups).unwrap();
    assert_eq!(state, OperationState::Partial);
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"one\"}\n");

    fixture.write("models.json", "{\"models\":[\"one\"]}\n");
    let state = compensate_operation(&fixture.storage, &id, &backups).unwrap();
    assert_eq!(state, OperationState::Compensated);
    assert_eq!(fixture.content("settings.json"), "{\"model\":\"one\"}\n");
    assert_eq!(fixture.content("models.json"), "{\"models\":[]}\n");
}

#[test]
fn a_stale_approval_writes_nothing() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"model\":\"one\"}\n");
    let applied = fixture.apply("settings.json", "{\"model\":\"two\"}\n", None);
    let plan = fixture.plan(&applied.operation_id);
    let stale = reviewed(&plan);

    // The file changes between the preview and the confirmation.
    fixture.write("settings.json", "{\"model\":\"two-plus\"}\n");
    assert_eq!(
        apply_restore(
            &fixture.storage,
            &applied.operation_id,
            &stale,
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::PlanStale
    );
    assert_eq!(
        fixture.content("settings.json"),
        "{\"model\":\"two-plus\"}\n"
    );
    // A reviewed ordinal that was never offered is refused the same way.
    let current = fixture.plan(&applied.operation_id);
    assert!(!current.restorable);
    let invented = [ReviewedRestoreStep {
        ordinal: 9,
        current_digest: None,
    }];
    assert_eq!(
        apply_restore(
            &fixture.storage,
            &applied.operation_id,
            &invented,
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::PlanStale
    );
}

#[test]
fn an_operation_with_nothing_to_restore_is_refused() {
    let fixture = Fixture::new();
    let id = fixture.begin(&["settings.json"]);
    let plan = fixture.plan(&id);
    assert!(!plan.restorable);
    assert_eq!(plan.steps[0].action, RestoreAction::Skip);
    assert_eq!(
        apply_restore(&fixture.storage, &id, &[], &fixture.backups()).unwrap_err(),
        WriteError::PlanStale
    );
    // Compensating an operation that never wrote anything reports the same state
    // the apply step would.
    assert_eq!(
        compensate_operation(&fixture.storage, &id, &fixture.backups()).unwrap(),
        OperationState::Failed
    );
    assert_eq!(
        plan_restore(&fixture.storage, &id).unwrap().unwrap().state,
        OperationState::Failed
    );
}

#[test]
fn a_restore_labels_the_provider_that_wrote_the_restored_content() {
    let fixture = Fixture::new();
    let first_owner = "0123456789abcdef0123456789abcdef";
    let second_owner = "fedcba9876543210fedcba9876543210";
    fixture.write("settings.json", "{\"model\":\"hand-written\"}\n");

    let first = fixture.apply("settings.json", "{\"model\":\"one\"}\n", Some(first_owner));
    let second = fixture.apply("settings.json", "{\"model\":\"two\"}\n", Some(second_owner));

    // Restoring the second write puts back what the first write left, so the key
    // that comes back belongs to the first Provider.
    let plan = fixture.plan(&second.operation_id);
    assert_eq!(
        step(&plan, "settings.json").restored_owner_ref.as_deref(),
        Some(first_owner)
    );
    // The first write replaced content the log never wrote, so its owner is
    // unknown instead of being guessed from the operation that ran last.
    let plan = fixture.plan(&first.operation_id);
    assert_eq!(step(&plan, "settings.json").restored_owner_ref, None);
}

#[test]
fn the_preview_never_carries_file_content() {
    let fixture = Fixture::new();
    let secret = "synthetic-restore-secret";
    fixture.write(
        "settings.json",
        &format!("{{\"env\":{{\"ANTHROPIC_AUTH_TOKEN\":\"{secret}\"}}}}\n"),
    );
    let applied = fixture.apply("settings.json", "{\"env\":{}}\n", None);
    let plan = fixture.plan(&applied.operation_id);
    let encoded = serde_json::to_string(&plan).unwrap();
    assert!(!encoded.contains(secret));
    assert!(!encoded.contains("ANTHROPIC_AUTH_TOKEN"));
    // The stored copy is where the secret lives, not the answer.
    let record = operation(&fixture.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    let copy = record.steps[0].backup.clone().unwrap();
    assert!(fs::read_to_string(&copy.path).unwrap().contains(secret));
}

#[test]
fn a_malformed_applied_step_is_refused_instead_of_classified() {
    let fixture = Fixture::new();
    let applied = fixture.apply("settings.json", "{\"model\":\"one\"}\n", None);
    {
        let connection = fixture.storage.lock().unwrap();
        connection
            .execute_batch("PRAGMA ignore_check_constraints = ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE config_operation_step SET applied_digest = NULL WHERE operation_id = ?1",
                [&applied.operation_id],
            )
            .unwrap();
    }
    assert_eq!(
        plan_restore(&fixture.storage, &applied.operation_id).unwrap_err(),
        WriteError::InvalidStoredOperation
    );
}

#[cfg(unix)]
#[test]
fn a_copy_that_cannot_be_removed_is_reported_instead_of_hidden() {
    // Imported here because only this Unix-only test uses it: on Windows a
    // read-only directory does not block removal, so the case cannot be built.
    use crate::config_apply::restore::discard_copy;
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let backups = fixture.backups();
    let copy = Backup {
        id: "1700000000000-1-0".to_owned(),
        path: backups
            .root()
            .join("1700000000000-1-0")
            .join("settings.json")
            .to_str()
            .unwrap()
            .to_owned(),
        content_digest: "abababababababababababababababababababababababababababababababab"
            .to_owned(),
    };
    fs::create_dir_all(Path::new(&copy.path).parent().unwrap()).unwrap();
    fs::write(&copy.path, "{}").unwrap();
    // A nested directory without write permission is what makes the removal fail:
    // the copy cannot be emptied, so the directory stays behind.
    let nested = Path::new(&copy.path).parent().unwrap().join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("kept.json"), "{}").unwrap();
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o500)).unwrap();

    assert_eq!(discard_copy(&backups, Some(&copy)), Some(copy.path.clone()));
    assert!(Path::new(&copy.path).parent().unwrap().exists());

    fs::set_permissions(&nested, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(discard_copy(&backups, Some(&copy)), None);
    assert!(!Path::new(&copy.path).parent().unwrap().exists());
}
