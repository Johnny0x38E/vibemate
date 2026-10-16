//! Isolated Pi tests: never touch real user configuration or OS credentials.
use super::*;
use crate::agents::{
    ApplyClaudeCodeRestoreRequest, ClaudeCodeRestoreError, ReviewedRestoreStepRequest,
};
use crate::config_apply::{StepState, operation};
use crate::credentials::fake::FakeStore;
use crate::model_catalog::{CapabilityOverrides, ModelCapabilities};
use crate::model_identity::RouteSupport;
use crate::models::{ModelSource, UpstreamState};
use crate::providers::ProviderId;
use std::sync::atomic::{AtomicU64, Ordering};
const ID: &str = "0123456789abcdef0123456789abcdef";
const KEY: &str = "synthetic-pi-key";

struct Fixture {
    root: PathBuf,
    storage: Storage,
    provider: ProviderRecord,
    model: ModelSnapshot,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vibemate-pi-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open_in_directory(&root.join("db")).unwrap();
        let provider = ProviderRecord {
            id: ProviderId::parse(ID).unwrap(),
            kind: ProviderKind::DeepSeek,
            display_name: "Pi test".into(),
            base_url: "https://api.deepseek.com".into(),
            protocol: ProviderProtocol::ChatCompletions,
            extensions: Default::default(),
            revision: 1,
            created_at_ms: 1,
            updated_at_ms: 1,
            selected_model_count: 1,
        };
        let model = ModelSnapshot {
            content_digest: "a".repeat(64),
            model: ProviderModel {
                provider_id: ID.into(),
                model_id: "deepseek-flash".into(),
                source: ModelSource::Manual,
                selected: true,
                alias: Some("My model".into()),
                upstream_name: None,
                context_window: Some(128000),
                max_output_tokens: Some(64000),
                input_modalities: None,
                output_modalities: None,
                supported_endpoints: None,
                capabilities: ModelCapabilities::unknown(),
                capability_overrides: CapabilityOverrides::none(),
                thinking_effort: None,
                route_support: RouteSupport::NotApplicable,
                resolved_protocol: Some(ProviderProtocol::ChatCompletions),
                upstream_state: UpstreamState::NeverListed,
                last_seen_at_ms: None,
                missing_since_ms: None,
                created_at_ms: 1,
                updated_at_ms: 1,
            },
        };
        Self {
            root,
            storage,
            provider,
            model,
        }
    }
    fn write(&self, file: &str, text: &str) {
        fs::write(self.root.join(file), text).unwrap();
    }
    fn text(&self, file: &str) -> String {
        fs::read_to_string(self.root.join(file)).unwrap()
    }
    fn plan(&self) -> PiPlan {
        plan_pi_settings(&self.storage, &self.root, &self.provider, &self.model).unwrap()
    }
    fn reviewed(&self) -> ReviewedPiPlan {
        let p = self.plan();
        ReviewedPiPlan {
            provider_key: p.provider_key,
            previous_provider_key: p.previous_provider_key,
            files: p
                .files
                .into_iter()
                .map(|f| ReviewedPiFile {
                    path: f.target.path,
                    content_digest: f.target.content_digest,
                })
                .collect(),
            auth_target: ReviewedPiFile {
                path: p.auth_target.path,
                content_digest: p.auth_target.content_digest,
            },
            provider_revision: p.source.provider_revision,
            model_content_digest: p.source.model_content_digest,
        }
    }
    fn store(&self) -> FakeStore {
        FakeStore::holding(&format!("provider-{ID}"), KEY)
    }
    fn apply(&self, reviewed: &ReviewedPiPlan) -> Result<ConfigApplied, ConfigApplyError> {
        apply_pi_settings(
            &self.storage,
            &self.store(),
            &self.root,
            &self.provider,
            &self.model,
            reviewed,
            self.root.join("backups"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn preview_is_read_only_and_apply_backs_up_both_original_files() {
    let f = Fixture::new();
    let models = r#"{"providers":{"mine":{"apiKey":"existing-private-key","models":[{"id":"keep"}]}},"other":true}"#;
    let settings =
        r#"{"theme":"dark","defaultProvider":"mine","defaultModel":"keep","extensions":["local"]}"#;
    f.write("models.json", models);
    f.write("settings.json", settings);
    let plan = serde_json::to_string(&f.plan()).unwrap();
    assert!(!plan.contains("existing-private-key"));
    assert!(!plan.contains(KEY));
    assert!(!f.root.join("backups").exists());
    assert_eq!(f.text("models.json"), models);
    let applied = f.apply(&f.reviewed()).unwrap();
    assert_eq!(applied.state, OperationState::Applied);
    assert_eq!(applied.steps.len(), 2);
    assert_eq!(
        fs::read_to_string(applied.steps[0].backup_path.as_ref().unwrap()).unwrap(),
        models
    );
    assert_eq!(
        fs::read_to_string(applied.steps[1].backup_path.as_ref().unwrap()).unwrap(),
        settings
    );
    let result: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        result["providers"]["mine"]["apiKey"],
        "existing-private-key"
    );
    let own = &result["providers"]["vibemate-Pi-test"];
    assert_eq!(own["apiKey"], KEY);
    assert_eq!(own["api"], "openai-completions");
    assert_eq!(own["models"][0]["maxTokens"], 64000);
    assert_eq!(own["models"][0]["compat"]["thinkingFormat"], "deepseek");
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    assert_eq!(settings["theme"], "dark");
    assert_eq!(settings["defaultModel"], "deepseek-flash");
    let record = operation(&f.storage, &applied.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(record.kind, OperationKind::PiSettings);
    assert!(!serde_json::to_string(&record).unwrap().contains(KEY));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(applied.steps[0].backup_path.as_ref().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn every_preview_token_is_required_before_writing() {
    for changed in [
        "models", "settings", "auth", "provider", "model", "path", "missing",
    ] {
        let mut f = Fixture::new();
        let mut reviewed = f.reviewed();
        match changed {
            "models" => f.write("models.json", "{\"providers\":{},\"edit\":true}"),
            "settings" => f.write("settings.json", "{\"theme\":\"mine\"}"),
            "auth" => f.write(
                "auth.json",
                "{\"other\":{\"type\":\"api_key\",\"key\":\"unrelated\"}}",
            ),
            "provider" => f.provider.revision += 1,
            "model" => f.model.content_digest = "b".repeat(64),
            "path" => reviewed.files[0].path.push_str("-other"),
            "missing" => {
                reviewed.files.pop();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            f.apply(&reviewed).unwrap_err(),
            ConfigApplyError::PlanStale,
            "{changed}"
        );
        assert!(!f.root.join("backups").exists());
    }
}

#[test]
fn second_file_failure_rolls_back_the_first_file() {
    let f = Fixture::new();
    f.write("models.json", "{\"providers\":{}}");
    f.write("settings.json", "{}");
    let reviewed = f.reviewed();
    let mut permissions = fs::metadata(f.root.join("settings.json"))
        .unwrap()
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(f.root.join("settings.json"), permissions).unwrap();
    let result = f.apply(&reviewed).unwrap();
    assert_eq!(result.state, OperationState::Compensated);
    assert_eq!(result.steps[1].state, StepState::Failed);
    assert_eq!(f.text("models.json"), "{\"providers\":{}}");
    assert_eq!(f.text("settings.json"), "{}");
}

#[test]
fn refuses_higher_priority_credentials_unknown_routes_and_unowned_namespaces() {
    let mut f = Fixture::new();
    f.write(
        "auth.json",
        &format!(r#"{{"vibemate-{ID}":{{"type":"api_key","key":"!never-run"}}}}"#),
    );
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model).unwrap_err(),
        PlanError::CredentialConflict
    );
    fs::remove_file(f.root.join("auth.json")).unwrap();
    f.write(
        "models.json",
        &format!(r#"{{"providers":{{"vibemate-{ID}":{{"apiKey":"mine"}}}}}}"#),
    );
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model).unwrap_err(),
        PlanError::TargetConflict
    );
    fs::remove_file(f.root.join("models.json")).unwrap();
    f.model.model.resolved_protocol = None;
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model).unwrap_err(),
        PlanError::ProtocolUnsupported
    );
}

#[test]
fn repeated_applications_preserve_other_models_and_mask_the_existing_key() {
    let mut f = Fixture::new();
    f.apply(&f.reviewed()).unwrap();
    f.model.model.model_id = "another/model.v1".into();
    f.model.content_digest = "b".repeat(64);
    let plan = serde_json::to_string(&f.plan()).unwrap();
    assert!(!plan.contains(KEY));
    f.apply(&f.reviewed()).unwrap();
    let result: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        result["providers"]["vibemate-Pi-test"]["models"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn key_templates_are_escaped_and_missing_credentials_write_nothing() {
    let f = Fixture::new();
    let reviewed = f.reviewed();
    let store = FakeStore::holding(&format!("provider-{ID}"), "!echo ${KEY}$!");
    apply_pi_settings(
        &f.storage,
        &store,
        &f.root,
        &f.provider,
        &f.model,
        &reviewed,
        f.root.join("backups"),
    )
    .unwrap();
    let value: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        value["providers"]["vibemate-Pi-test"]["apiKey"],
        "$!echo $${KEY}$$!"
    );
    let other = Fixture::new();
    let error = apply_pi_settings(
        &other.storage,
        &FakeStore::default(),
        &other.root,
        &other.provider,
        &other.model,
        &other.reviewed(),
        other.root.join("backups"),
    )
    .unwrap_err();
    assert_eq!(error, ConfigApplyError::CredentialMissing);
    assert!(!other.root.join("models.json").exists());
}

#[test]
fn preserves_custom_base_and_uses_each_models_resolved_protocol() {
    for (protocol, api) in [
        (ProviderProtocol::Responses, "openai-responses"),
        (ProviderProtocol::AnthropicMessages, "anthropic-messages"),
    ] {
        let mut f = Fixture::new();
        f.model.model.resolved_protocol = Some(protocol);
        let plan = f.plan();
        assert!(
            plan.files[0]
                .changes
                .iter()
                .any(|c| c.key_path.ends_with(".api") && c.after == api)
        );
    }
    let mut f = Fixture::new();
    f.provider.base_url = "https://gateway.example/v1".into();
    assert!(
        f.plan().files[0]
            .changes
            .iter()
            .any(|c| c.key_path.ends_with(".baseUrl") && c.after == f.provider.base_url)
    );
    f.model.model.resolved_protocol = Some(ProviderProtocol::AnthropicMessages);
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model).unwrap_err(),
        PlanError::BaseUrlUnverified
    );
}

#[test]
fn pi_backups_restore_both_files_and_claude_cannot_access_their_operation() {
    let f = Fixture::new();
    f.write("models.json", "{\"providers\":{}}");
    f.write("settings.json", "{\"theme\":\"mine\"}");
    let result = f.apply(&f.reviewed()).unwrap();
    assert_eq!(
        crate::agents::plan_claude_code_restore(&f.storage, &result.operation_id).unwrap_err(),
        ClaudeCodeRestoreError::OperationNotFound
    );
    let plan = crate::agents::plan_config_restore(
        &f.storage,
        &result.operation_id,
        OperationKind::PiSettings,
    )
    .unwrap();
    let request = ApplyClaudeCodeRestoreRequest {
        operation_id: result.operation_id,
        reviewed: plan
            .steps
            .iter()
            .map(|s| ReviewedRestoreStepRequest {
                ordinal: s.ordinal,
                current_digest: s.current_digest.clone(),
            })
            .collect(),
    };
    assert_eq!(
        crate::agents::apply_claude_code_restore(&f.storage, &request, &f.root.join("backups"))
            .unwrap_err(),
        ClaudeCodeRestoreError::OperationNotFound
    );
    let restored = crate::agents::apply_config_restore(
        &f.storage,
        &request,
        &f.root.join("backups"),
        OperationKind::PiSettings,
    )
    .unwrap();
    assert_eq!(restored.state, OperationState::Compensated);
    assert_eq!(f.text("models.json"), "{\"providers\":{}}");
    assert_eq!(f.text("settings.json"), "{\"theme\":\"mine\"}");
}

#[test]
fn unknown_versions_and_invalid_directory_overrides_are_refused_without_launch() {
    let f = Fixture::new();
    let env = DiscoveryEnvironment {
        home: f.root.clone(),
        pi_directory: Some("relative".into()),
        ..Default::default()
    };
    assert_eq!(
        configuration_root(&env, None).unwrap_err(),
        PlanError::LocationUnavailable
    );
    let env = DiscoveryEnvironment {
        home: f.root.clone(),
        ..Default::default()
    };
    assert_eq!(
        configuration_root(&env, None).unwrap_err(),
        PlanError::AgentVersionUnverified
    );
}

#[cfg(unix)]
#[test]
fn symlink_targets_are_refused() {
    let f = Fixture::new();
    f.write("real.json", "{}");
    std::os::unix::fs::symlink(f.root.join("real.json"), f.root.join("models.json")).unwrap();
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model).unwrap_err(),
        PlanError::TargetConflict
    );
    assert_eq!(f.text("real.json"), "{}");
}

#[cfg(unix)]
#[test]
fn adding_a_key_makes_an_existing_public_catalog_private_even_when_content_matches() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.write("models.json", "{\"providers\":{}}");
    fs::set_permissions(
        f.root.join("models.json"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    f.apply(&f.reviewed()).unwrap();
    assert_eq!(
        fs::metadata(f.root.join("models.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    fs::set_permissions(
        f.root.join("models.json"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    f.apply(&f.reviewed()).unwrap();
    assert_eq!(
        fs::metadata(f.root.join("models.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn readable_provider_names_are_stable_and_separate_accounts_with_the_same_label() {
    let mut f = Fixture::new();
    f.provider.display_name = "OpenRouter".into();
    assert_eq!(f.plan().provider_key, "vibemate-OpenRouter");
    f.apply(&f.reviewed()).unwrap();
    let old = f.provider.id.clone();
    f.provider.id = ProviderId::parse("fedcba9876543210fedcba9876543210").unwrap();
    f.model.model.provider_id = f.provider.id.as_str().into();
    assert_eq!(f.plan().provider_key, "vibemate-OpenRouter-fedcba98");
    f.provider.id = old;
    f.model.model.provider_id = ID.into();
    assert_eq!(f.plan().provider_key, "vibemate-OpenRouter");
    f.provider.display_name = "OpenRouter Work".into();
    f.provider.revision += 1;
    assert_eq!(
        f.plan().previous_provider_key.as_deref(),
        Some("vibemate-OpenRouter")
    );
    f.apply(&f.reviewed()).unwrap();
    let document: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert!(document["providers"].get("vibemate-OpenRouter").is_none());
    assert!(
        document["providers"]
            .get("vibemate-OpenRouter-Work")
            .is_some()
    );
}

#[test]
fn applying_renames_a_recorded_uuid_provider_and_preserves_every_model() {
    let f = Fixture::new();
    let legacy = format!("vibemate-{ID}");
    let document = json!({"providers":{&legacy:{"apiKey":"synthetic-previous-key","models":[{"id":"deepseek-flash"},{"id":"keep-me","extra":"user"}],"extra":"keep"},"mine":{"models":[{"id":"untouched"}]}}});
    let path = f.root.join("models.json");
    crate::config_apply::apply_files(
        &f.storage,
        OperationKind::PiSettings,
        Some(ID),
        &[PlannedFile {
            target: WriteTarget::from_plan(&read_target_document(&path).unwrap().target),
            content: serde_json::to_vec_pretty(&document).unwrap(),
        }],
        &BackupStore::new(f.root.join("backups")),
    )
    .unwrap();
    f.write(
        "settings.json",
        &json!({"defaultProvider": &legacy, "defaultModel":"deepseek-flash",
        "modelThinkingLevels":{format!("{legacy}/deepseek-flash"):"high","mine/untouched":"low"},
        "enabledModels":[format!("{legacy}/*"),"mine/*"]})
        .to_string(),
    );
    let plan = f.plan();
    assert_eq!(plan.provider_key, "vibemate-Pi-test");
    assert_eq!(plan.previous_provider_key.as_deref(), Some(legacy.as_str()));
    assert!(
        !serde_json::to_string(&plan)
            .unwrap()
            .contains("synthetic-previous-key")
    );
    f.apply(&f.reviewed()).unwrap();
    let result: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert!(result["providers"].get(&legacy).is_none());
    assert_eq!(result["providers"]["vibemate-Pi-test"]["name"], "Pi test");
    assert_eq!(
        result["providers"]["vibemate-Pi-test"]["models"][1]["extra"],
        "user"
    );
    assert_eq!(result["providers"]["mine"], document["providers"]["mine"]);
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    assert_eq!(
        settings["modelThinkingLevels"]["vibemate-Pi-test/deepseek-flash"],
        "high"
    );
    assert_eq!(settings["modelThinkingLevels"]["mine/untouched"], "low");
    assert_eq!(
        settings["enabledModels"],
        json!(["vibemate-Pi-test/*", "mine/*"])
    );
}

#[test]
fn stable_pi_updates_use_the_same_schema_contract_instead_of_an_exact_version() {
    for version in ["1.2.0", "1.2.1", "1.3.0", "1.15.10"] {
        assert!(supported_version(version), "{version}");
    }
    for version in ["1.1.0", "2.0.0", "1.3.0-beta", "../1.3.0", "1.3", "1.3.x"] {
        assert!(!supported_version(version), "{version}");
    }
}
