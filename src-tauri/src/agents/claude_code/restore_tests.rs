//! Restore-surface tests for the Claude Code adapter: the listing, the Provider
//! labels and the request boundary. Every fixture is a temporary directory
//! removed when the test ends; no real Agent file is touched.
use super::*;
use crate::config_apply::{
    BackupStore, OperationKind, PlannedFile, RestoreAction, StepState, WriteTarget, apply_files,
};
use crate::config_plan::read_target_document;
use crate::credentials::Secret;
use crate::credentials::fake::FakeStore;
use crate::providers::{CreateProviderRequest, DeleteProviderRequest};
use crate::storage::Storage;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

/// A key that never leaves the fake store.
const KEY: &str = "synthetic-restore-key";

struct Fixture {
    root: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-claude-code-restore-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open_in_directory(&root.join("storage")).unwrap();
        Self { root, storage }
    }

    fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    fn backups_root(&self) -> PathBuf {
        self.root.join("backups")
    }

    fn write_settings(&self, content: &str) {
        fs::write(self.settings(), content).unwrap();
    }

    fn content(&self) -> String {
        fs::read_to_string(self.settings()).unwrap()
    }

    /// Create a saved Provider through the real boundary, so its display name is
    /// the one the listing resolves.
    fn create_provider(&self, name: &str) -> String {
        let record = crate::providers::create_provider(
            &self.storage,
            &FakeStore::default(),
            CreateProviderRequest {
                kind: "deepseek".to_owned(),
                display_name: name.to_owned(),
                base_url: "https://api.deepseek.com".to_owned(),
                protocol: "chat_completions".to_owned(),
                extensions: BTreeMap::new(),
                secret: Secret::new(KEY.to_owned()),
            },
            42,
        )
        .expect("provider");
        record.id.as_str().to_owned()
    }

    /// Write the settings file through the recorded write path, with an owner.
    fn apply(&self, content: &str, owner: &str) -> String {
        let target =
            WriteTarget::from_plan(&read_target_document(&self.settings()).unwrap().target);
        let files = [PlannedFile {
            target,
            content: content.as_bytes().to_vec(),
        }];
        apply_files(
            &self.storage,
            OperationKind::ClaudeCodeSettings,
            Some(owner),
            &files,
            &BackupStore::new(self.backups_root()),
        )
        .expect("recorded write")
        .operation_id
    }

    fn plan(&self, operation_id: &str) -> ClaudeCodeRestorePlan {
        plan_claude_code_restore(&self.storage, operation_id).expect("preview")
    }

    /// The tokens the interface would send back for a preview.
    fn reviewed(&self, plan: &ClaudeCodeRestorePlan) -> ApplyClaudeCodeRestoreRequest {
        ApplyClaudeCodeRestoreRequest {
            operation_id: plan.operation_id.clone(),
            reviewed: plan
                .steps
                .iter()
                .filter(|step| {
                    matches!(step.action, RestoreAction::Restore | RestoreAction::Remove)
                })
                .map(|step| ReviewedRestoreStepRequest {
                    ordinal: step.ordinal,
                    current_digest: step.current_digest.clone(),
                })
                .collect(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn the_listing_labels_the_provider_and_carries_no_backup_path() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"model\":\"one\"}\n");
    let provider_id = fixture.create_provider("DeepSeek Work");
    let operation_id = fixture.apply("{\"model\":\"two\"}\n", &provider_id);

    let listed = list_claude_code_operations(&fixture.storage).expect("listing");
    assert_eq!(listed.len(), 1);
    let row = &listed[0];
    assert_eq!(row.operation_id, operation_id);
    assert_eq!(row.state, OperationState::Applied);
    assert_eq!(row.provider_id.as_deref(), Some(provider_id.as_str()));
    assert_eq!(row.provider_label.as_deref(), Some("DeepSeek Work"));
    assert!(row.restorable);
    assert_eq!(row.steps.len(), 1);
    assert_eq!(row.steps[0].state, StepState::Applied);
    assert_eq!(row.steps[0].restore_state, None);
    assert!(row.steps[0].target_path.ends_with("settings.json"));
    // The listing names the file, never where the copy of its content sits.
    let encoded = serde_json::to_string(&listed).unwrap();
    assert!(!encoded.contains("backups"));
    assert!(!encoded.contains(KEY));
}

#[test]
fn a_deleted_provider_leaves_the_label_out_instead_of_a_stale_name() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"model\":\"one\"}\n");
    let provider_id = fixture.create_provider("Removed Later");
    let operation_id = fixture.apply("{\"model\":\"two\"}\n", &provider_id);
    crate::providers::delete_provider(
        &fixture.storage,
        &FakeStore::default(),
        DeleteProviderRequest {
            id: provider_id.clone(),
            expected_revision: 1,
        },
    )
    .expect("delete provider");

    let listed = list_claude_code_operations(&fixture.storage).expect("listing");
    assert_eq!(listed[0].provider_id.as_deref(), Some(provider_id.as_str()));
    assert_eq!(listed[0].provider_label, None);
    // The recorded write stays restorable: undoing it does not need the Provider.
    assert!(listed[0].restorable);
    assert_eq!(
        fixture.plan(&operation_id).provider_label,
        None,
        "the preview resolves the label the same way"
    );
}

#[test]
fn a_restore_names_the_provider_whose_key_comes_back() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"model\":\"hand-written\"}\n");
    let first = fixture.create_provider("First Key");
    let second = fixture.create_provider("Second Key");
    fixture.apply("{\"model\":\"one\"}\n", &first);
    let second_write = fixture.apply("{\"model\":\"two\"}\n", &second);

    let plan = fixture.plan(&second_write);
    assert_eq!(plan.provider_label.as_deref(), Some("Second Key"));
    let step = &plan.steps[0];
    assert_eq!(step.action, RestoreAction::Restore);
    assert_eq!(step.restored_provider_id.as_deref(), Some(first.as_str()));
    assert_eq!(step.restored_provider_label.as_deref(), Some("First Key"));

    let outcome = apply_claude_code_restore(
        &fixture.storage,
        &fixture.reviewed(&plan),
        &fixture.backups_root(),
    )
    .expect("restore");
    assert_eq!(outcome.state, OperationState::Compensated);
    assert_eq!(fixture.content(), "{\"model\":\"one\"}\n");
}

#[test]
fn an_unknown_operation_is_reported_instead_of_planned() {
    let fixture = Fixture::new();
    let missing = "0".repeat(32);
    assert_eq!(
        plan_claude_code_restore(&fixture.storage, &missing).unwrap_err(),
        ClaudeCodeRestoreError::OperationNotFound
    );
    // A malformed identity is a request error, not a missing operation.
    assert_eq!(
        plan_claude_code_restore(&fixture.storage, "not-an-id").unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
}

#[test]
fn the_reviewed_tokens_are_checked_before_anything_is_read() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"model\":\"one\"}\n");
    let provider_id = fixture.create_provider("Boundary");
    let operation_id = fixture.apply("{\"model\":\"two\"}\n", &provider_id);
    let plan = fixture.plan(&operation_id);

    let request = |reviewed: Vec<ReviewedRestoreStepRequest>| ApplyClaudeCodeRestoreRequest {
        operation_id: operation_id.clone(),
        reviewed,
    };
    // A digest that is not a SHA-256 token cannot come from a preview.
    assert_eq!(
        request(vec![ReviewedRestoreStepRequest {
            ordinal: 0,
            current_digest: Some("short".to_owned()),
        }])
        .reviewed_steps()
        .unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
    // One ordinal twice describes two different files at the same position.
    assert_eq!(
        request(vec![
            ReviewedRestoreStepRequest {
                ordinal: 0,
                current_digest: None,
            },
            ReviewedRestoreStepRequest {
                ordinal: 0,
                current_digest: None,
            },
        ])
        .reviewed_steps()
        .unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
    // An oversized list is refused before any file is read.
    let too_many: Vec<ReviewedRestoreStepRequest> = (0..17)
        .map(|ordinal| ReviewedRestoreStepRequest {
            ordinal,
            current_digest: None,
        })
        .collect();
    assert_eq!(
        request(too_many).reviewed_steps().unwrap_err(),
        ClaudeCodeRestoreError::InvalidRequest
    );
    // The tokens a preview produced are accepted as they are.
    assert_eq!(fixture.reviewed(&plan).reviewed_steps().unwrap().len(), 1);
}

#[test]
fn a_settings_edit_after_the_preview_refuses_the_approval() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"model\":\"one\"}\n");
    let provider_id = fixture.create_provider("Stale");
    let operation_id = fixture.apply("{\"model\":\"two\"}\n", &provider_id);
    let reviewed = fixture.reviewed(&fixture.plan(&operation_id));

    fixture.write_settings("{\"model\":\"two-plus\"}\n");
    assert_eq!(
        apply_claude_code_restore(&fixture.storage, &reviewed, &fixture.backups_root())
            .unwrap_err(),
        ClaudeCodeRestoreError::PlanStale
    );
    assert_eq!(fixture.content(), "{\"model\":\"two-plus\"}\n");
}

#[test]
fn every_restore_error_code_matches_its_serialized_form() {
    // Keep this list in step with the enum; `code()` is exhaustive, so a new
    // variant fails to compile until it names its code.
    let errors = [
        ClaudeCodeRestoreError::OperationNotFound,
        ClaudeCodeRestoreError::PlanStale,
        ClaudeCodeRestoreError::InvalidRequest,
        ClaudeCodeRestoreError::StorageUnavailable,
        ClaudeCodeRestoreError::OperationFailed,
    ];
    for error in errors {
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!(error.code())
        );
    }
}
