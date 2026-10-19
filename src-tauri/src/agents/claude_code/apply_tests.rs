//! Apply-step tests for the Claude Code adapter. Every fixture is a temporary
//! directory removed when the test ends; no real Agent file and no OS credential
//! store is touched.
use super::*;
use crate::config_apply::{StepState, operation, unfinished_operations};
use crate::config_plan::SECRET_PLACEHOLDER;
use crate::credentials::CredentialError;
use crate::credentials::fake::FakeStore;
use crate::providers::ProviderKind;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

const DEEPSEEK_BASE: &str = "https://api.deepseek.com/anthropic";
const COMMAND_CODE_BASE: &str = "https://api.commandcode.ai/provider/v1";
/// A stand-in model content digest; the value only has to be a SHA-256 token.
const MODEL_DIGEST: &str = "0f4f87db4567232a7f1756aa1534ec1314777b39c3bf5209f87cf9739321cddc";
const PROVIDER_ID: &str = "0123456789abcdef0123456789abcdef";
/// A key that never leaves the fake store except through the written file.
const KEY: &str = "synthetic-key-value";

struct Fixture {
    root: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-claude-code-apply-{}-{}",
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

    fn write_settings(&self, content: &str) {
        fs::write(self.settings(), content).unwrap();
    }

    fn content(&self) -> String {
        fs::read_to_string(self.settings()).unwrap()
    }

    fn plan_input(&self) -> ClaudeCodePlanInput {
        ClaudeCodePlanInput {
            config_root: self.root.clone(),
            provider: ProviderFacts {
                id: PROVIDER_ID.to_owned(),
                kind: ProviderKind::DeepSeek,
                base_url: DEEPSEEK_BASE.to_owned(),
                revision: 3,
            },
            model: ModelFacts {
                id: "deepseek-flash".to_owned(),
                content_digest: MODEL_DIGEST.to_owned(),
            },
        }
    }

    /// Tokens a preview would have produced for the current state.
    fn reviewed(&self) -> ReviewedPlan {
        let plan = plan_claude_code_settings(&self.plan_input()).unwrap();
        ReviewedPlan {
            target_path: plan.target.path,
            target_digest: plan.target.content_digest,
            provider_revision: plan.source.provider_revision,
            model_content_digest: plan.source.model_content_digest,
        }
    }

    fn input(&self) -> ClaudeCodeApplyInput {
        ClaudeCodeApplyInput {
            plan: self.plan_input(),
            reviewed: self.reviewed(),
            backups: self.root.join("backups"),
        }
    }

    fn store(&self) -> FakeStore {
        FakeStore::holding(&format!("provider-{PROVIDER_ID}"), KEY)
    }

    /// How many copies the private backup directory holds.
    fn backups(&self) -> usize {
        fs::read_dir(self.root.join("backups"))
            .map(|entries| entries.count())
            .unwrap_or(0)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn applying_a_plan_writes_the_key_and_keeps_unrelated_settings() {
    let fixture = Fixture::new();
    fixture.write_settings(
        "{\"theme\":\"dark\",\"permissions\":{\"defaultMode\":\"acceptEdits\"},\"env\":{\"EDITOR\":\"vim\"}}\n",
    );
    let applied =
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &fixture.input()).unwrap();

    assert_eq!(applied.state, OperationState::Applied);
    assert!(applied.restart_required);
    assert_eq!(applied.steps.len(), 1);
    assert_eq!(applied.steps[0].state, StepState::Applied);
    assert_eq!(applied.steps[0].error_code, None);
    assert_eq!(
        applied.steps[0].target_path,
        fixture.settings().to_str().unwrap()
    );

    let written: serde_json::Value = serde_json::from_str(&fixture.content()).unwrap();
    assert_eq!(written["theme"], "dark");
    assert_eq!(written["permissions"]["defaultMode"], "acceptEdits");
    assert_eq!(written["env"]["EDITOR"], "vim");
    assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], KEY);
    assert_eq!(written["env"]["ANTHROPIC_BASE_URL"], DEEPSEEK_BASE);
    assert_eq!(written["env"]["ANTHROPIC_MODEL"], "deepseek-flash");
    assert_eq!(
        written["env"]["CLAUDE_CODE_SUBAGENT_MODEL"],
        "deepseek-flash"
    );
    assert!(!fixture.content().contains(SECRET_PLACEHOLDER));

    // The copy of the previous content is stored outside the settings folder, and
    // the operation is recorded as applied.
    let backup_path = applied.backup_path.as_ref().unwrap();
    let backup = fs::read_to_string(backup_path).unwrap();
    assert!(backup.contains("\"EDITOR\":\"vim\""));
    assert!(!backup.contains(KEY));
    let record = operation(&fixture.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.kind, OperationKind::ClaudeCodeSettings);
    assert_eq!(record.state, OperationState::Applied);
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
}

#[test]
fn the_applied_result_never_carries_the_key() {
    let fixture = Fixture::new();
    fixture.write_settings("{}\n");
    let applied =
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &fixture.input()).unwrap();

    let payload = serde_json::to_string(&applied).unwrap();
    assert!(!payload.contains(KEY));
    assert!(payload.contains("applied"));
}

#[test]
fn a_changed_token_requires_a_new_preview() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"theme\":\"dark\"}\n");

    // The settings file changed after the user reviewed the plan.
    let mut input = fixture.input();
    fixture.write_settings("{\"theme\":\"light\"}\n");
    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &input).unwrap_err(),
        ClaudeCodeApplyError::PlanStale
    );
    assert_eq!(fixture.content(), "{\"theme\":\"light\"}\n");

    // The Provider was edited after the preview.
    input = fixture.input();
    input.reviewed.provider_revision += 1;
    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &input).unwrap_err(),
        ClaudeCodeApplyError::PlanStale
    );

    // The model record changed.
    input = fixture.input();
    input.reviewed.model_content_digest = "1".repeat(64);
    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &input).unwrap_err(),
        ClaudeCodeApplyError::PlanStale
    );

    // The reviewed plan pointed at another file.
    input = fixture.input();
    input.reviewed.target_path = fixture.root.join("other.json").to_str().unwrap().to_owned();
    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &input).unwrap_err(),
        ClaudeCodeApplyError::PlanStale
    );

    assert_eq!(fixture.content(), "{\"theme\":\"light\"}\n");
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
    assert_eq!(fixture.backups(), 0);
}

#[test]
fn a_missing_key_stops_before_any_write() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"theme\":\"dark\"}\n");
    let store = FakeStore::default();

    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &store, &fixture.input()).unwrap_err(),
        ClaudeCodeApplyError::CredentialMissing
    );
    assert_eq!(fixture.content(), "{\"theme\":\"dark\"}\n");
    assert_eq!(fixture.backups(), 0);
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
}

#[test]
fn an_unreadable_credential_store_is_reported() {
    let fixture = Fixture::new();
    fixture.write_settings("{}\n");
    let store = FakeStore::holding(&format!("provider-{PROVIDER_ID}"), KEY);
    store.fail_loads(Some(CredentialError::AccessDenied));

    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &store, &fixture.input()).unwrap_err(),
        ClaudeCodeApplyError::CredentialUnavailable
    );
    assert_eq!(fixture.content(), "{}\n");
    assert_eq!(fixture.backups(), 0);
}

#[test]
fn a_provider_without_official_instructions_is_refused() {
    let fixture = Fixture::new();
    fixture.write_settings("{}\n");
    let mut input = fixture.input();
    input.plan.provider.kind = ProviderKind::CommandCode;
    input.plan.provider.base_url = COMMAND_CODE_BASE.to_owned();
    input.reviewed = fixture.reviewed();

    assert_eq!(
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &input).unwrap_err(),
        ClaudeCodeApplyError::GatewayUnverified
    );
    assert_eq!(fixture.content(), "{}\n");
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_target_fails_with_its_step_code() {
    let fixture = Fixture::new();
    let real = fixture.root.join("real-settings.json");
    fs::write(&real, "{\"theme\":\"dark\"}\n").unwrap();
    std::os::unix::fs::symlink(&real, fixture.settings()).unwrap();

    let applied =
        apply_claude_code_settings(&fixture.storage, &fixture.store(), &fixture.input()).unwrap();

    assert_eq!(applied.state, OperationState::Failed);
    assert_eq!(applied.steps[0].state, StepState::Failed);
    assert_eq!(
        applied.steps[0].error_code.as_deref(),
        Some("symlink_refused")
    );
    assert_eq!(applied.backup_path, None);
    // Nothing was written, so no new session is needed for unchanged settings.
    assert!(!applied.restart_required);
    assert_eq!(fs::read_to_string(&real).unwrap(), "{\"theme\":\"dark\"}\n");
    // The refusal is recorded and settled, so the recovery entry does not list it.
    let record = operation(&fixture.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.state, OperationState::Failed);
    assert!(unfinished_operations(&fixture.storage).unwrap().is_empty());
}

#[test]
fn applying_the_same_plan_again_touches_the_file_once() {
    let fixture = Fixture::new();
    fixture.write_settings("{\"theme\":\"dark\"}\n");
    let store = fixture.store();
    let first = apply_claude_code_settings(&fixture.storage, &store, &fixture.input()).unwrap();
    let written = fixture.content();

    // A fresh preview of the file as it is now; the same values would be written.
    let second = apply_claude_code_settings(&fixture.storage, &store, &fixture.input()).unwrap();

    assert_eq!(second.state, OperationState::Applied);
    assert_eq!(second.steps[0].state, StepState::Applied);
    assert_eq!(second.steps[0].backup_path, None);
    assert_eq!(fixture.content(), written);
    assert_ne!(first.operation_id, second.operation_id);
    // The file is replaced once: one recovery copy. Each recorded apply also
    // owns a full version copy, including the unchanged second apply.
    assert_eq!(fixture.backups(), 3);
}
