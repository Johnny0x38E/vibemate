//! Apply-step tests: several files, a failure partway, and the rollback that
//! follows. Every fixture is a temporary directory removed when the test ends.
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
            "vibemate-config-service-{}-{}",
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

    /// A target whose token matches the file as it is now.
    fn target(&self, name: &str) -> WriteTarget {
        WriteTarget::from_plan(&read_target_document(&self.path(name)).unwrap().target)
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

    /// A file to hand to an apply step.
    fn file(&self, name: &str, content: &str) -> PlannedFile {
        PlannedFile {
            target: self.target(name),
            content: content.as_bytes().to_vec(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn an_apply_step_replaces_every_file_and_settles_as_applied() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"old\":true}\n");
    fixture.write("models.json", "{\"models\":[]}\n");
    let files = [
        fixture.file("settings.json", "{\"old\":false}\n"),
        fixture.file("models.json", "{\"models\":[\"one\"]}\n"),
    ];
    let backups = fixture.backups();

    let result = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &files,
        &backups,
    )
    .unwrap();

    assert_eq!(result.state, OperationState::Applied);
    assert_eq!(result.steps.len(), 2);
    assert!(
        result
            .steps
            .iter()
            .all(|step| step.state == StepState::Applied)
    );
    assert_eq!(fixture.content("settings.json"), "{\"old\":false}\n");
    assert_eq!(fixture.content("models.json"), "{\"models\":[\"one\"]}\n");
    // Both replaced files have their previous content stored, and each step
    // reports where that copy is.
    assert_eq!(fs::read_dir(backups.root()).unwrap().count(), 2);
    for step in &result.steps {
        let backup = step.backup_path.as_ref().expect("backup path");
        assert!(Path::new(backup).is_file());
    }
    // The settled operation is no longer unfinished, and its steps are recorded.
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
    let record = operation(&fixture.storage, &result.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.state, OperationState::Applied);
    assert_eq!(record.steps.len(), 2);
    assert!(record.steps.iter().all(|step| step.backup.is_some()));
    assert!(
        record
            .steps
            .iter()
            .all(|step| step.applied_digest.is_some())
    );
}

#[test]
fn a_failing_second_file_rolls_the_first_one_back() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"old\":true}\n");
    fixture.write("models.json", "{\"models\":[]}\n");
    let first = fixture.file("settings.json", "{\"old\":false}\n");
    // Another writer changed the second file after the plan was built.
    let second = fixture.file("models.json", "{\"models\":[\"one\"]}\n");
    fixture.write("models.json", "{\"models\":[\"edited\"]}\n");

    let result = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &[first, second],
        &fixture.backups(),
    )
    .unwrap();

    assert_eq!(result.state, OperationState::Compensated);
    assert_eq!(result.steps[0].state, StepState::Applied);
    assert_eq!(result.steps[1].state, StepState::Failed);
    assert_eq!(
        result.steps[1].error_code.as_deref(),
        Some("target_changed")
    );
    // The first file holds its previous content again, and the user's edit to the
    // second file is still there.
    assert_eq!(fixture.content("settings.json"), "{\"old\":true}\n");
    assert_eq!(
        fixture.content("models.json"),
        "{\"models\":[\"edited\"]}\n"
    );

    let record = operation(&fixture.storage, &result.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.state, OperationState::Compensated);
    assert_eq!(record.steps[0].state, StepState::Applied);
    assert_eq!(record.steps[1].state, StepState::Failed);
    assert_eq!(
        record.steps[1].error_code.as_deref(),
        Some("target_changed")
    );
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
}

#[test]
fn a_created_file_is_removed_when_the_step_is_rolled_back() {
    let fixture = Fixture::new();
    // The first target does not exist yet, so the step creates it.
    let first = PlannedFile {
        target: fixture.target("settings.json"),
        content: b"{\"created\":true}\n".to_vec(),
    };
    let blocked = fixture.path("blocked");
    fs::create_dir(&blocked).unwrap();
    let second = PlannedFile {
        target: WriteTarget {
            path: blocked,
            expected_digest: None,
        },
        content: b"{\"never\":true}\n".to_vec(),
    };

    let result = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &[first, second],
        &fixture.backups(),
    )
    .unwrap();

    assert_eq!(result.state, OperationState::Compensated);
    assert_eq!(result.steps[0].state, StepState::Applied);
    assert_eq!(
        result.steps[1].error_code.as_deref(),
        Some("target_unreadable")
    );
    assert!(!fixture.path("settings.json").exists());
}

#[test]
fn a_failing_first_file_changes_nothing_and_settles_as_failed() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"kept\":true}\n");
    let stale = PlannedFile {
        target: WriteTarget {
            path: fixture.path("settings.json"),
            expected_digest: Some(digest_hex(b"what the plan saw")),
        },
        content: b"{\"kept\":false}\n".to_vec(),
    };
    let later = fixture.file("models.json", "{\"models\":[]}\n");

    let result = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &[stale, later],
        &fixture.backups(),
    )
    .unwrap();

    assert_eq!(result.state, OperationState::Failed);
    assert_eq!(result.steps.len(), 1);
    assert_eq!(
        result.steps[0].error_code.as_deref(),
        Some("target_changed")
    );
    assert_eq!(fixture.content("settings.json"), "{\"kept\":true}\n");
    // The file that was never attempted stays pending in the log.
    let record = operation(&fixture.storage, &result.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.state, OperationState::Failed);
    assert_eq!(record.steps[1].state, StepState::Pending);
    assert!(!fixture.backups().root().exists());
}

#[test]
fn compensation_leaves_a_file_a_later_edit_changed() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"before\":true}\n");
    let files = [fixture.file("settings.json", "{\"after\":true}\n")];
    let backups = fixture.backups();
    let result = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &files,
        &backups,
    )
    .unwrap();
    // The operation is settled, but the situation a crash would leave is built by
    // hand: an applied step whose file a later edit replaced.
    let record = operation(&fixture.storage, &result.operation_id)
        .unwrap()
        .unwrap();
    let step = &record.steps[0];
    fixture.write("settings.json", "{\"edited\":true}\n");
    let error = restore_file(
        &fixture.path("settings.json"),
        step.applied_digest.as_deref().unwrap(),
        step.backup.as_ref().unwrap(),
        &backups,
    )
    .unwrap_err();

    assert_eq!(error, WriteError::TargetChanged);
    assert_eq!(fixture.content("settings.json"), "{\"edited\":true}\n");

    // The same refusal settles an unfinished operation as partial.
    let unfinished = begin_operation(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &[WriteTarget {
            path: fixture.path("settings.json"),
            expected_digest: None,
        }],
    )
    .unwrap();
    record_step_applied(
        &fixture.storage,
        &unfinished,
        0,
        Some(step.backup.as_ref().unwrap()),
        step.applied_digest.as_deref().unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(
        compensate_operation(&fixture.storage, &unfinished, &backups).unwrap(),
        OperationState::Partial
    );
    assert_eq!(fixture.content("settings.json"), "{\"edited\":true}\n");
    // A rollback that could not finish stays listed for the recovery entry.
    let listed = unfinished_operations(&fixture.storage).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, unfinished);
    assert_eq!(listed[0].state, OperationState::Partial);
}

#[test]
fn a_stored_copy_that_no_longer_matches_its_digest_is_refused() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"before\":true}\n");
    let files = [fixture.file("settings.json", "{\"after\":true}\n")];
    let backups = fixture.backups();
    let result = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &files,
        &backups,
    )
    .unwrap();
    let record = operation(&fixture.storage, &result.operation_id)
        .unwrap()
        .unwrap();
    let step = &record.steps[0];
    let backup = step.backup.as_ref().unwrap();
    let digest = step.applied_digest.as_deref().unwrap();

    // A missing copy and a tampered copy are both refusals, not silent restores.
    let stored = fs::read(&backup.path).unwrap();
    fs::remove_file(&backup.path).unwrap();
    assert_eq!(
        restore_file(&fixture.path("settings.json"), digest, backup, &backups).unwrap_err(),
        WriteError::BackupUnavailable
    );
    fs::write(&backup.path, "{\"tampered\":true}\n").unwrap();
    assert_eq!(
        restore_file(&fixture.path("settings.json"), digest, backup, &backups).unwrap_err(),
        WriteError::BackupUnavailable
    );
    fs::write(&backup.path, &stored).unwrap();
    assert!(restore_file(&fixture.path("settings.json"), digest, backup, &backups).is_ok());
    assert_eq!(fixture.content("settings.json"), "{\"before\":true}\n");
}

#[test]
fn an_empty_step_or_a_settled_operation_is_refused() {
    let fixture = Fixture::new();
    let empty: [PlannedFile; 0] = [];
    assert_eq!(
        apply_files(
            &fixture.storage,
            OperationKind::ClaudeCodeSettings,
            None,
            &empty,
            &fixture.backups()
        )
        .unwrap_err(),
        WriteError::InvalidRequest
    );

    fixture.write("settings.json", "{\"before\":true}\n");
    let files = [fixture.file("settings.json", "{\"after\":true}\n")];
    let backups = fixture.backups();
    let settled = apply_files(
        &fixture.storage,
        OperationKind::ClaudeCodeSettings,
        None,
        &files,
        &backups,
    )
    .unwrap();
    assert_eq!(
        compensate_operation(&fixture.storage, &settled.operation_id, &backups).unwrap_err(),
        WriteError::InvalidRequest
    );
    assert_eq!(
        compensate_operation(&fixture.storage, &"0".repeat(32), &backups).unwrap_err(),
        WriteError::InvalidRequest
    );
    // The settled operation was not disturbed by the refusal.
    assert_eq!(fixture.content("settings.json"), "{\"after\":true}\n");
    assert_eq!(
        operation(&fixture.storage, &settled.operation_id)
            .unwrap()
            .unwrap()
            .state,
        OperationState::Applied
    );
}

#[test]
fn removing_a_created_file_refuses_content_this_module_did_not_write() {
    let fixture = Fixture::new();
    fixture.write("settings.json", "{\"mine\":true}\n");
    let digest = digest_hex(b"{\"mine\":true}\n");
    // The file still holds what the module wrote.
    assert_eq!(
        remove_created_file(&fixture.path("settings.json"), &digest),
        Ok(())
    );
    assert!(!fixture.path("settings.json").exists());
    // Already gone counts as removed.
    assert_eq!(
        remove_created_file(&fixture.path("settings.json"), &digest),
        Ok(())
    );

    fixture.write("settings.json", "{\"theirs\":true}\n");
    assert_eq!(
        remove_created_file(&fixture.path("settings.json"), &digest),
        Err(WriteError::TargetChanged)
    );
    assert!(fixture.path("settings.json").exists());
}
