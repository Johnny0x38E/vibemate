//! Isolated Pi tests: never touch real user configuration or OS credentials.
use super::*;
use crate::agents::{
    ApplyClaudeCodeRestoreRequest, ClaudeCodeRestoreError, ReviewedRestoreStepRequest,
};
use crate::config_apply::{StepState, operation};
use crate::credentials::fake::FakeStore;
use crate::model_catalog::{
    CapabilityEvidence, CapabilityOverride, CapabilityOverrides, CapabilitySource, CapabilityState,
    EffortLevel, ModelCapabilities, ThinkingEffort,
};
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
        self.plan_with(None).unwrap()
    }
    /// Build a preview for an explicit thinking-level choice (or none).
    fn plan_with(&self, level: Option<PiThinkingLevel>) -> Result<PiPlan, PlanError> {
        plan_pi_settings(
            &self.storage,
            &self.root,
            &self.provider,
            &self.model,
            level,
        )
    }
    fn reviewed(&self) -> ReviewedPiPlan {
        reviewed_from(self.plan())
    }
    fn reviewed_with(&self, level: Option<PiThinkingLevel>) -> ReviewedPiPlan {
        reviewed_from(self.plan_with(level).unwrap())
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
/// The approval tokens a reviewed preview would send back to apply.
fn reviewed_from(p: PiPlan) -> ReviewedPiPlan {
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
        selected_thinking_level: p.selected_thinking_level,
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
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model, None).unwrap_err(),
        PlanError::CredentialConflict
    );
    fs::remove_file(f.root.join("auth.json")).unwrap();
    f.write(
        "models.json",
        &format!(r#"{{"providers":{{"vibemate-{ID}":{{"apiKey":"mine"}}}}}}"#),
    );
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model, None).unwrap_err(),
        PlanError::TargetConflict
    );
    fs::remove_file(f.root.join("models.json")).unwrap();
    f.model.model.resolved_protocol = None;
    assert_eq!(
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model, None).unwrap_err(),
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
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model, None).unwrap_err(),
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
        plan_pi_settings(&f.storage, &f.root, &f.provider, &f.model, None).unwrap_err(),
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
fn model_names_carry_the_marker_and_model_id_not_the_user_alias() {
    let mut f = Fixture::new();
    f.provider.display_name = "Command Code GOAT".into();
    f.provider.revision += 1;
    f.model.model.model_id = "deepseek/deepseek-v4.1-flash".into();
    f.model.model.alias = Some("Pretty label".into());
    f.model.model.upstream_name = Some("Upstream Pretty".into());
    f.model.content_digest = "c".repeat(64);
    let plan = serde_json::to_string(&f.plan()).unwrap();
    assert!(
        plan.contains("(vibemate-Command-Code-GOAT) deepseek/deepseek-v4.1-flash"),
        "the planned name carries the marker and the model ID: {plan}"
    );
    assert!(!plan.contains("Pretty label"));
    f.apply(&f.reviewed()).unwrap();
    let document: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    let models = document["providers"]["vibemate-Command-Code-GOAT"]["models"]
        .as_array()
        .unwrap();
    let written = models
        .iter()
        .find(|model| model["id"] == "deepseek/deepseek-v4.1-flash")
        .expect("selected model written");
    // The model id stays the identity; only the display name is combined.
    assert_eq!(
        written["name"],
        "(vibemate-Command-Code-GOAT) deepseek/deepseek-v4.1-flash"
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

/// The catalog-style effort statement the model dialog stores for one model.
fn listed(levels: &[EffortLevel]) -> ThinkingEffort {
    ThinkingEffort::Listed {
        levels: levels.to_vec(),
        default_level: None,
    }
}

/// Declare reasoning support and an effort list, as a user would in the model dialog.
fn declare_thinking(f: &mut Fixture, levels: &[EffortLevel]) {
    f.model.model.capability_overrides.thinking = Some(CapabilityOverride::Supported);
    f.model.model.thinking_effort = Some(listed(levels));
}

/// Rewrite the owned model definition, as a user editing `models.json` by hand would.
fn edit_owned_model(f: &Fixture, edit: impl FnOnce(&mut Value)) {
    let mut models: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    edit(&mut models["providers"]["vibemate-Pi-test"]["models"][0]);
    f.write("models.json", &models.to_string());
}

#[test]
fn a_declared_level_is_saved_under_its_own_model_key_and_other_entries_survive() {
    let mut f = Fixture::new();
    declare_thinking(&mut f, &[EffortLevel::Low, EffortLevel::High]);
    f.write(
        "models.json",
        r#"{"providers":{"mine":{"models":[{"id":"deepseek-flash"}]}}}"#,
    );
    f.write(
        "settings.json",
        r#"{"defaultThinkingLevel":"medium","modelThinkingLevels":{"mine/deepseek-flash":"off","vibemate-Pi-test/other":"low"}}"#,
    );
    let plan = f.plan_with(Some(PiThinkingLevel::High)).unwrap();
    assert_eq!(plan.selected_thinking_level, Some(PiThinkingLevel::High));
    let change = plan.files[1]
        .changes
        .iter()
        .find(|c| c.key_path == "modelThinkingLevels.vibemate-Pi-test/deepseek-flash")
        .expect("the settings change names the per-model key");
    assert_eq!(change.before, None);
    assert_eq!(change.after, "high");
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::High)))
        .unwrap();
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    assert_eq!(
        settings["modelThinkingLevels"]["vibemate-Pi-test/deepseek-flash"],
        "high"
    );
    assert_eq!(
        settings["modelThinkingLevels"]["mine/deepseek-flash"],
        "off"
    );
    assert_eq!(
        settings["modelThinkingLevels"]["vibemate-Pi-test/other"],
        "low"
    );
    assert_eq!(settings["defaultThinkingLevel"], "medium");
}

#[test]
fn do_not_overwrite_keeps_an_existing_saved_level_without_a_level_change() {
    let mut f = Fixture::new();
    declare_thinking(&mut f, &[EffortLevel::Low]);
    f.write(
        "settings.json",
        r#"{"modelThinkingLevels":{"vibemate-Pi-test/deepseek-flash":"low"}}"#,
    );
    let plan = f.plan_with(None).unwrap();
    assert_eq!(plan.selected_thinking_level, None);
    assert!(
        plan.files[1]
            .changes
            .iter()
            .all(|c| !c.key_path.starts_with("modelThinkingLevels."))
    );
    f.apply(&f.reviewed()).unwrap();
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    assert_eq!(
        settings["modelThinkingLevels"]["vibemate-Pi-test/deepseek-flash"],
        "low"
    );
}

#[test]
fn undeclared_or_unaccepted_levels_are_refused_and_apply_cannot_forge_them() {
    type Setup = fn(&mut Fixture);
    let cases: [(&str, Setup, PiThinkingLevel); 3] = [
        (
            "reasoning not declared",
            |f| f.model.model.thinking_effort = Some(listed(&[EffortLevel::High])),
            PiThinkingLevel::High,
        ),
        (
            "declared support without any listed level",
            |f| f.model.model.capability_overrides.thinking = Some(CapabilityOverride::Supported),
            PiThinkingLevel::High,
        ),
        (
            "listed max still needs a Pi mapping",
            |f| declare_thinking(f, &[EffortLevel::Max]),
            PiThinkingLevel::Max,
        ),
    ];
    for (name, setup, level) in cases {
        let mut f = Fixture::new();
        setup(&mut f);
        assert_eq!(
            f.plan_with(Some(level)).unwrap_err(),
            PlanError::ThinkingLevelUnavailable,
            "{name}"
        );
        // A hand-edited approval with the same level must hit the same rebuild check.
        let mut forged = f.reviewed();
        forged.selected_thinking_level = Some(level);
        assert_eq!(
            f.apply(&forged).unwrap_err(),
            ConfigApplyError::ThinkingLevelUnavailable,
            "{name}"
        );
        assert!(!f.root.join("backups").exists(), "{name}");
    }
}

#[test]
fn a_user_null_map_entry_removes_the_level_and_a_user_string_map_enables_xhigh() {
    let mut f = Fixture::new();
    declare_thinking(&mut f, &[EffortLevel::High]);
    f.apply(&f.reviewed()).unwrap();
    edit_owned_model(&f, |model| {
        model["thinkingLevelMap"] = json!({"high": null})
    });
    assert_eq!(
        f.plan_with(Some(PiThinkingLevel::High)).unwrap_err(),
        PlanError::ThinkingLevelUnavailable
    );

    let mut f = Fixture::new();
    declare_thinking(&mut f, &[EffortLevel::XHigh]);
    f.apply(&f.reviewed()).unwrap();
    edit_owned_model(&f, |model| {
        model["thinkingLevelMap"] = json!({"xhigh": "max"})
    });
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::XHigh)))
        .unwrap();
    let models: Value = serde_json::from_str(&f.text("models.json")).unwrap();
    assert_eq!(
        models["providers"]["vibemate-Pi-test"]["models"][0]["thinkingLevelMap"],
        json!({"xhigh": "max"}),
        "the user's map is preserved, never rewritten by vibemate"
    );
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    assert_eq!(
        settings["modelThinkingLevels"]["vibemate-Pi-test/deepseek-flash"],
        "xhigh"
    );
}

#[test]
fn a_renamed_provider_takes_the_chosen_level_under_its_new_key_only() {
    let f = Fixture::new();
    let legacy = format!("vibemate-{ID}");
    let document = json!({"providers":{&legacy:{"models":[{"id":"deepseek-flash"}]}}});
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
        &json!({"defaultProvider": &legacy, "defaultModel": "deepseek-flash",
        "modelThinkingLevels": {format!("{legacy}/deepseek-flash"): "high"}})
        .to_string(),
    );
    let mut f = f;
    declare_thinking(&mut f, &[EffortLevel::Low, EffortLevel::High]);
    let plan = f.plan_with(Some(PiThinkingLevel::Low)).unwrap();
    assert_eq!(plan.previous_provider_key.as_deref(), Some(legacy.as_str()));
    f.apply(&f.reviewed_with(Some(PiThinkingLevel::Low)))
        .unwrap();
    let settings: Value = serde_json::from_str(&f.text("settings.json")).unwrap();
    let levels = settings["modelThinkingLevels"].as_object().unwrap();
    assert_eq!(levels.get(&format!("{legacy}/deepseek-flash")), None);
    assert_eq!(
        levels.get("vibemate-Pi-test/deepseek-flash"),
        Some(&json!("low"))
    );
}

#[test]
fn the_reviewed_plan_carries_its_level_for_detailed_preview_and_apply() {
    let mut f = Fixture::new();
    declare_thinking(&mut f, &[EffortLevel::Low]);
    let chosen = serde_json::to_value(f.plan_with(Some(PiThinkingLevel::Low)).unwrap()).unwrap();
    assert_eq!(chosen["selectedThinkingLevel"], "low");
    assert_eq!(reviewed_level(&chosen).unwrap(), Some(PiThinkingLevel::Low));
    let inherited = serde_json::to_value(f.plan_with(None).unwrap()).unwrap();
    assert!(inherited["selectedThinkingLevel"].is_null());
    assert_eq!(reviewed_level(&inherited).unwrap(), None);
    assert_ne!(
        chosen, inherited,
        "a changed level must not look like the reviewed plan"
    );
}

#[test]
fn the_frontend_approval_shape_deserializes_with_its_thinking_token() {
    // The exact keys the desktop bridge sends back; any extra or unknown token fails.
    let shape = |level: Value| {
        json!({
            "providerKey": "vibemate-Pi-test",
            "previousProviderKey": null,
            "files": [
                {"path": "/x/models.json", "contentDigest": null},
                {"path": "/x/settings.json", "contentDigest": null}
            ],
            "authTarget": {"path": "/x/auth.json", "contentDigest": null},
            "providerRevision": 1,
            "modelContentDigest": "a".repeat(64),
            "selectedThinkingLevel": level
        })
    };
    let high: ReviewedPiPlan = serde_json::from_value(shape(json!("high"))).unwrap();
    assert_eq!(high.selected_thinking_level, Some(PiThinkingLevel::High));
    let inherited: ReviewedPiPlan = serde_json::from_value(shape(Value::Null)).unwrap();
    assert_eq!(inherited.selected_thinking_level, None);
    assert!(serde_json::from_value::<ReviewedPiPlan>(shape(json!("none"))).is_err());
    assert!(serde_json::from_value::<ReviewedPiPlan>(shape(json!(7))).is_err());
}

#[test]
fn plan_facts_name_each_source_and_what_the_plan_writes() {
    let mut f = Fixture::new();
    f.model.model.capabilities.thinking = CapabilityEvidence {
        state: CapabilityState::Supported,
        source: Some(CapabilitySource::OpenrouterModelReasoning),
    };
    f.model.model.capability_overrides.image_input = Some(CapabilityOverride::Unsupported);
    assert_eq!(
        f.plan().model_facts,
        vec![
            PiModelFact {
                item: PiFactItem::ContextWindow,
                source: PiFactSource::Library,
                state: PiFactState::Written,
                target: "contextWindow",
                value: Some("128000".into()),
            },
            PiModelFact {
                item: PiFactItem::MaxOutputTokens,
                source: PiFactSource::Library,
                state: PiFactState::Written,
                target: "maxTokens",
                value: Some("64000".into()),
            },
            PiModelFact {
                item: PiFactItem::Reasoning,
                source: PiFactSource::Catalog,
                state: PiFactState::Written,
                target: "reasoning",
                value: Some("true".into()),
            },
            PiModelFact {
                item: PiFactItem::ImageInput,
                source: PiFactSource::UserDeclaration,
                state: PiFactState::Written,
                target: "input",
                value: Some("text".into()),
            },
            PiModelFact {
                item: PiFactItem::ThinkingLevel,
                source: PiFactSource::Choice,
                state: PiFactState::NotStated,
                target: "modelThinkingLevels",
                value: None,
            },
        ]
    );
}

#[test]
fn values_the_plan_does_not_write_are_kept_or_reported_as_not_stated() {
    let mut f = Fixture::new();
    // The first application binds the owned namespace; the edits below stand in for
    // values a user or an earlier application already holds in the native files.
    f.apply(&f.reviewed()).unwrap();
    edit_owned_model(&f, |model| {
        model["contextWindow"] = json!(32000);
        model["input"] = json!(["text", "image"]);
    });
    f.write(
        "settings.json",
        r#"{"modelThinkingLevels":{"vibemate-Pi-test/deepseek-flash":"low"}}"#,
    );
    f.model.model.context_window = None;
    f.model.model.max_output_tokens = None;
    let facts = f.plan().model_facts;
    assert_eq!(
        facts,
        vec![
            PiModelFact {
                item: PiFactItem::ContextWindow,
                source: PiFactSource::NotStated,
                state: PiFactState::Unchanged,
                target: "contextWindow",
                value: Some("32000".into()),
            },
            PiModelFact {
                item: PiFactItem::MaxOutputTokens,
                source: PiFactSource::NotStated,
                state: PiFactState::Unchanged,
                target: "maxTokens",
                value: Some("64000".into()),
            },
            PiModelFact {
                item: PiFactItem::Reasoning,
                source: PiFactSource::NotStated,
                state: PiFactState::NotStated,
                target: "reasoning",
                value: None,
            },
            PiModelFact {
                item: PiFactItem::ImageInput,
                source: PiFactSource::NotStated,
                state: PiFactState::Unchanged,
                target: "input",
                value: Some("text, image".into()),
            },
            PiModelFact {
                item: PiFactItem::ThinkingLevel,
                source: PiFactSource::Choice,
                state: PiFactState::Unchanged,
                target: "modelThinkingLevels",
                value: Some("low".into()),
            },
        ]
    );
}

#[test]
fn facts_serialize_with_the_tokens_the_desktop_bridge_validates() {
    let mut f = Fixture::new();
    declare_thinking(&mut f, &[EffortLevel::High]);
    let value = serde_json::to_value(f.plan_with(Some(PiThinkingLevel::High)).unwrap()).unwrap();
    let facts = value["modelFacts"].as_array().unwrap();
    assert_eq!(facts.len(), 5);
    assert_eq!(
        facts[0],
        json!({"item": "contextWindow", "source": "library", "state": "written", "target": "contextWindow", "value": "128000"})
    );
    assert_eq!(
        facts[2],
        json!({"item": "reasoning", "source": "userDeclaration", "state": "written", "target": "reasoning", "value": "true"})
    );
    assert_eq!(
        facts[3],
        json!({"item": "imageInput", "source": "notStated", "state": "notStated", "target": "input", "value": null})
    );
    assert_eq!(
        facts[4],
        json!({"item": "thinkingLevel", "source": "choice", "state": "written", "target": "modelThinkingLevels", "value": "high"})
    );
}
