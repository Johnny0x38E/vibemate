//! Isolated Grok Build tests: never touch real user configuration or OS credentials.
use super::*;
use crate::model_catalog::{
    CapabilityEvidence, CapabilityOverrides, CapabilitySource, CapabilityState, ModelCapabilities,
};
use crate::model_identity::RouteSupport;
use crate::models::{ModelSource, UpstreamState};
use crate::providers::{ProviderId, ProviderKind};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

const ID: &str = "0123456789abcdef0123456789abcdef";
const MODEL: &str = "deepseek-flash";
/// A value that belongs to the user's file and must never reach a plan.
const PRIVATE: &str = "user-private-key";

/// One reference row per field the plan writes, as the installed manual spells it.
fn reference_text() -> String {
    let mut rows = String::from("# Configuration reference\n\n");
    for row in CONTRACT_ROWS {
        rows.push_str(row);
        rows.push('\n');
    }
    rows
}

/// The refusal a plan must produce, so tests read as one expectation.
fn plan_error(
    fixture_root: &Path,
    storage: &Storage,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
) -> PlanError {
    plan_grok_settings(storage, fixture_root, provider, snapshot).expect_err("the plan must refuse")
}

fn snapshot(model: ProviderModel) -> ModelSnapshot {
    ModelSnapshot {
        content_digest: "b".repeat(64),
        model,
    }
}

fn model(mutate: impl FnOnce(&mut ProviderModel)) -> ModelSnapshot {
    let mut model = ProviderModel {
        provider_id: ID.into(),
        model_id: MODEL.into(),
        source: ModelSource::Manual,
        selected: true,
        alias: Some("My flash".into()),
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
    };
    mutate(&mut model);
    snapshot(model)
}

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
            "vibemate-grok-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open_in_directory(&root.join("db")).unwrap();
        let provider = ProviderRecord {
            id: ProviderId::parse(ID).unwrap(),
            kind: ProviderKind::DeepSeek,
            display_name: "Grok test".into(),
            base_url: "https://api.deepseek.com".into(),
            protocol: ProviderProtocol::ChatCompletions,
            extensions: Default::default(),
            revision: 1,
            created_at_ms: 1,
            updated_at_ms: 1,
            selected_model_count: 1,
        };
        Self {
            root,
            storage,
            provider,
            model: model(|_| {}),
        }
    }

    fn write(&self, file: &str, text: &str) {
        if let Some(parent) = self.root.join(file).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(self.root.join(file), text).unwrap();
    }

    fn text(&self, file: &str) -> String {
        fs::read_to_string(self.root.join(file)).unwrap()
    }

    /// A configuration root whose launch-extracted artifacts match a versioned
    /// download, so `configuration_root` has a contract to check.
    fn installed(&self) -> DiscoveryEnvironment {
        fs::create_dir_all(self.root.join("downloads")).unwrap();
        fs::write(
            self.root.join("downloads/grok-1.0.60-macos-aarch64"),
            "not a real binary",
        )
        .unwrap();
        self.write(".metadata_version", "1.0.60");
        self.write("docs/user-guide/26-config-reference.md", &reference_text());
        DiscoveryEnvironment {
            home: self.root.clone(),
            grok_directory: Some(self.root.as_os_str().to_os_string()),
            ..DiscoveryEnvironment::default()
        }
    }

    fn entry(&self, name: &str) -> Option<String> {
        Some(self.root.join(name).to_string_lossy().into_owned())
    }

    fn plan(&self) -> GrokPlan {
        self.plan_with(&self.provider.clone(), &self.model)
    }

    fn plan_with(&self, provider: &ProviderRecord, model: &ModelSnapshot) -> GrokPlan {
        plan_grok_settings(&self.storage, &self.root, provider, model).unwrap()
    }

    fn error(&self, provider: &ProviderRecord, model: &ModelSnapshot) -> PlanError {
        plan_error(&self.root, &self.storage, provider, model)
    }

    /// Record an alias as written by vibemate for the selected Provider and model.
    fn bind(&self, alias: &str) {
        self.storage
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO grok_model_binding(config_root,model_alias,provider_id,model_id) VALUES(?1,?2,?3,?4)",
                rusqlite::params![self.root.to_str().unwrap(), alias, ID, MODEL],
            )
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The user's own document: another model, an MCP server, UI settings, a comment
/// and a private key that must never appear in a plan.
const USER_CONFIG: &str = r#"# my Grok settings
[models]
default = "my-model"

[model.my-model]
model = "someone-elses"
base_url = "https://gateway.example/v1"
name = "Mine"
api_backend = "responses"
env_key = "MY_GATEWAY_KEY"
api_key = "user-private-key"

[[model.my-model.reasoning_efforts]]
value = "high"

[mcp_servers.codegraph]
command = "must-not-run"
args = ["serve", "--mcp"]

[ui]
theme = "dark"
"#;

#[test]
fn full_model_preview_is_bound_to_native_plan_and_preserves_unowned_toml() {
    use crate::configuration_preview::{PreviewError, PreviewRequest, read_preview};
    let f = Fixture::new();
    let environment = f.installed();
    f.write(
        "config.toml",
        "# retained layout\n[other]\npassword = \"synthetic-unowned\"\nlabel = \"keep\"\n",
    );
    let plan = f.plan();
    let mut request = PreviewRequest {
        agent: AgentId::GrokBuild,
        provider_id: ID.into(),
        model_id: MODEL.into(),
        entry_path: f.entry("downloads/grok-1.0.60-macos-aarch64"),
        reviewed: serde_json::to_value(&plan).unwrap(),
        file_index: None,
    };
    let summaries =
        read_preview(&f.storage, &environment, &f.provider, &f.model, &request).unwrap();
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].text.is_none());
    request.file_index = Some(0);
    let details = read_preview(&f.storage, &environment, &f.provider, &f.model, &request).unwrap();
    let encoded = serde_json::to_string(&details).unwrap();
    assert!(!encoded.contains("synthetic-unowned"));
    let text = details[0].text.as_ref().unwrap();
    let crate::config_apply::VersionTextSide::Present { text: after, .. } = &text.target else {
        panic!("planned text");
    };
    assert!(after.contains("# retained layout"));
    let document = after.parse::<DocumentMut>().unwrap();
    // The default model belongs to the user and Grok's picker; the plan does
    // not create a [models] table merely to select the written alias.
    assert!(document.get("models").is_none());
    assert_eq!(document["other"]["label"].as_str(), Some("keep"));
    let mut changed_provider = f.provider.clone();
    changed_provider.revision += 1;
    assert!(matches!(
        read_preview(
            &f.storage,
            &environment,
            &changed_provider,
            &f.model,
            &request
        ),
        Err(PreviewError::PlanStale)
    ));
    f.write("config.toml", "# external edit\n");
    assert!(matches!(
        read_preview(&f.storage, &environment, &f.provider, &f.model, &request),
        Err(PreviewError::PlanStale)
    ));
}

#[test]
fn preview_describes_only_its_own_fields_and_leaves_the_file_untouched() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let before = f.text("config.toml");
    let plan = f.plan();
    let paths: Vec<String> = plan
        .file
        .changes
        .iter()
        .map(|change| change.key_path.clone())
        .collect();
    let owned = format!("model.{}.", plan.model_alias);
    assert!(
        paths.iter().all(|path| path.starts_with(&owned)),
        "unexpected keys: {paths:?}"
    );
    // `[models].default` is user-owned and must never appear in this plan.
    assert!(!paths.iter().any(|path| path == "models.default"));
    for field in ["model", "base_url", "name", "api_backend", "api_key"] {
        assert!(
            paths.contains(&format!("{owned}{field}")),
            "missing {field} in {paths:?}"
        );
    }
    assert!(paths.contains(&format!("{owned}context_window")));
    // Request parameters are not part of this plan: they need a verified mapping.
    assert!(!paths.contains(&format!("{owned}max_completion_tokens")));
    assert!(!paths.contains(&format!("{owned}temperature")));

    let rendered = serde_json::to_string(&plan).unwrap();
    assert!(!rendered.contains(PRIVATE));
    assert!(
        rendered.contains(SECRET_PLACEHOLDER),
        "the planned key must stay masked"
    );
    // A preview never writes, backs up or creates the backups directory.
    assert_eq!(f.text("config.toml"), before);
    assert!(!f.root.join("backups").exists());
    assert_eq!(plan.agent, AgentId::GrokBuild);
    assert!(plan.restart_required);
    assert!(!plan.existing_alias);
}

#[test]
fn context_summary_matches_the_native_write_and_serialized_contract() {
    let f = Fixture::new();
    let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
    let field = format!("model.{}.context_window", plan.model_alias);
    let change = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == field)
        .unwrap();
    assert_eq!(plan.context_window.source, GrokContextSource::Library);
    assert_eq!(plan.context_window.state, GrokContextState::Written);
    assert_eq!(
        plan.context_window.value.as_deref(),
        Some(change.after.as_str())
    );
    assert_eq!(
        document["model"][&plan.model_alias]["context_window"].as_integer(),
        Some(128000)
    );
    assert_eq!(
        serde_json::to_value(&plan).unwrap()["contextWindow"],
        serde_json::json!({
            "source": "library", "state": "written", "value": "128000"
        })
    );
}

#[test]
fn undeclared_capacity_preserves_native_fields_and_reports_the_native_value() {
    let mut f = Fixture::new();
    let alias = f.plan().model_alias;
    f.bind(&alias);
    f.model.model.context_window = None;
    let text = format!(
        "[models]\ndefault = \"mine\"\n[model.{alias}]\ncontext_window = 65536\nsupports_reasoning_effort = false\nreasoning_effort = \"high\"\ntemperature = 0.7\ntop_p = 0.9\nmax_completion_tokens = 1024\n[[model.{alias}.reasoning_efforts]]\nvalue = \"low\"\ndefault = true\n"
    );
    f.write("config.toml", &text);
    let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
    assert_eq!(plan.context_window.source, GrokContextSource::NotStated);
    assert_eq!(plan.context_window.state, GrokContextState::Unchanged);
    assert_eq!(plan.context_window.value.as_deref(), Some("65536"));
    assert!(
        !plan
            .file
            .changes
            .iter()
            .any(|change| change.key_path.ends_with(".context_window"))
    );
    let before = text.parse::<DocumentMut>().unwrap();
    for field in [
        "context_window",
        "supports_reasoning_effort",
        "reasoning_effort",
        "reasoning_efforts",
        "temperature",
        "top_p",
        "max_completion_tokens",
    ] {
        assert_eq!(
            document["model"][&alias][field].to_string(),
            before["model"][&alias][field].to_string()
        );
    }
    assert_eq!(document["models"].to_string(), before["models"].to_string());
    assert_eq!(f.text("config.toml"), text);
}

#[test]
fn undeclared_capacity_does_not_invent_a_default_or_expose_unknown_native_text() {
    let mut f = Fixture::new();
    f.model.model.context_window = None;
    let plan = f.plan();
    assert_eq!(plan.context_window.state, GrokContextState::NotStated);
    assert_eq!(plan.context_window.value, None);
    f.bind(&plan.model_alias);
    for native in [
        format!("\"{PRIVATE}\""),
        format!("{{ password = \"{PRIVATE}\" }}"),
    ] {
        f.write(
            "config.toml",
            &format!("[model.{}]\ncontext_window = {native}\n", plan.model_alias),
        );
        let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
        assert_eq!(plan.context_window.state, GrokContextState::Unchanged);
        assert_eq!(plan.context_window.value, None);
        assert!(!serde_json::to_string(&plan).unwrap().contains(PRIVATE));
        assert!(document.to_string().contains(PRIVATE));
    }
}

#[test]
fn preview_rejects_forged_context_summary_before_details_are_returned() {
    use crate::configuration_preview::{PreviewError, PreviewRequest, read_preview};
    let f = Fixture::new();
    let environment = f.installed();
    let mut reviewed = serde_json::to_value(f.plan()).unwrap();
    reviewed["contextWindow"]["value"] = serde_json::json!("999999");
    let request = PreviewRequest {
        agent: AgentId::GrokBuild,
        provider_id: ID.into(),
        model_id: MODEL.into(),
        entry_path: f.entry("downloads/grok-1.0.60-macos-aarch64"),
        reviewed,
        file_index: Some(0),
    };
    assert!(matches!(
        read_preview(&f.storage, &environment, &f.provider, &f.model, &request),
        Err(PreviewError::PlanStale)
    ));
    assert!(!f.root.join("config.toml").exists());
}

#[test]
fn image_capability_is_injected_as_vision_and_reported_from_its_source() {
    let mut f = Fixture::new();
    f.model.model.capabilities.image_input = CapabilityEvidence {
        state: CapabilityState::Supported,
        source: Some(CapabilitySource::DeepseekModelInputModalities),
    };
    let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
    let alias = &plan.model_alias;
    let field = format!("model.{alias}.vision");
    assert_eq!(plan.image_input.source, GrokCapabilitySource::Catalog);
    assert_eq!(plan.image_input.state, GrokCapabilityState::Written);
    assert_eq!(plan.image_input.value, Some(true));
    assert!(plan.file.changes.iter().any(|change| {
        change.key_path == field && change.before.is_none() && change.after == "true"
    }));
    assert!(document["model"][alias]["vision"].as_bool().unwrap());

    f.model.model.capabilities.image_input.state = CapabilityState::Unknown;
    f.model.model.capabilities.image_input.source = None;
    f.model.model.capability_overrides.image_input =
        Some(crate::model_catalog::CapabilityOverride::Unsupported);
    let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
    assert_eq!(
        plan.image_input.source,
        GrokCapabilitySource::UserDeclaration
    );
    assert_eq!(plan.image_input.value, Some(false));
    assert!(
        !document["model"][&plan.model_alias]["vision"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn unknown_image_capability_preserves_native_vision_without_exposing_other_shapes() {
    let f = Fixture::new();
    let alias = f.plan().model_alias;
    f.bind(&alias);
    f.write("config.toml", &format!("[model.{alias}]\nvision = true\n"));
    let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
    assert_eq!(plan.image_input.source, GrokCapabilitySource::NotStated);
    assert_eq!(plan.image_input.state, GrokCapabilityState::Unchanged);
    assert_eq!(plan.image_input.value, Some(true));
    assert!(
        !plan
            .file
            .changes
            .iter()
            .any(|change| change.key_path.ends_with(".vision"))
    );
    assert!(document["model"][&alias]["vision"].as_bool().unwrap());

    f.write(
        "config.toml",
        &format!("[model.{alias}]\nvision = \"{PRIVATE}\"\n"),
    );
    let (plan, document) = build(&f.storage, &f.root, &f.provider, &f.model).unwrap();
    assert_eq!(plan.image_input.state, GrokCapabilityState::Unchanged);
    assert_eq!(plan.image_input.value, None);
    assert!(!serde_json::to_string(&plan).unwrap().contains(PRIVATE));
    assert!(document.to_string().contains(PRIVATE));
}

#[test]
fn malformed_native_vision_is_not_silently_overwritten() {
    let mut f = Fixture::new();
    let alias = f.plan().model_alias;
    f.bind(&alias);
    f.write(
        "config.toml",
        &format!("[model.{alias}]\nvision = \"unknown\"\n"),
    );
    f.model.model.capability_overrides.image_input =
        Some(crate::model_catalog::CapabilityOverride::Supported);
    assert_eq!(f.error(&f.provider, &f.model), PlanError::TargetConflict);
}

#[test]
fn preview_creates_the_tables_a_new_document_needs() {
    let f = Fixture::new();
    let plan = f.plan();
    // No file existed, so the plan reports it as absent and still names its keys.
    assert!(!plan.file.target.exists);
    assert!(plan.model_alias.starts_with("vibemate-"));
    assert!(
        plan.notices
            .iter()
            .any(|notice| notice.code == NoticeCode::CredentialFromStore)
    );
    assert_eq!(plan.precedence.len(), PRECEDENCE_FILES.len());
    assert!(
        plan.precedence
            .iter()
            .all(|file| file.state == GrokLayerState::Absent)
    );
}

#[test]
fn preview_reports_a_policy_layer_it_did_not_read() {
    let f = Fixture::new();
    f.write(
        "requirements.toml",
        "[models]\ndefault = \"pinned-value\"\n",
    );
    let plan = f.plan();
    let requirements = plan
        .precedence
        .iter()
        .find(|file| file.name == "requirements.toml")
        .expect("requirements layer listed");
    assert_eq!(requirements.state, GrokLayerState::Present);
    assert!(
        plan.notices
            .iter()
            .any(|notice| notice.code == NoticeCode::OverridesNotChecked)
    );
    // Existence is the whole claim: the file's content stays on disk.
    let rendered = serde_json::to_string(&plan).unwrap();
    assert!(!rendered.contains("pinned-value"));
}

#[test]
fn preview_never_adopts_an_alias_the_user_already_wrote() {
    let f = Fixture::new();
    let taken = format!("vibemate-grok-test-{MODEL}");
    f.write(
        "config.toml",
        &format!(
            "[model.{taken}]\nmodel = \"theirs\"\napi_key = \"{PRIVATE}\"\n\n[models]\ndefault = \"{taken}\"\n"
        ),
    );
    let plan = f.plan();
    assert_ne!(plan.model_alias, taken);
    assert!(plan.model_alias.ends_with(&ID[..8]));
    let paths: Vec<String> = plan
        .file
        .changes
        .iter()
        .map(|change| change.key_path.clone())
        .collect();
    assert!(
        !paths
            .iter()
            .any(|path| path.starts_with(&format!("model.{taken}."))),
        "the user's entry must not change: {paths:?}"
    );
    let rendered = serde_json::to_string(&plan).unwrap();
    assert!(!rendered.contains(PRIVATE));
}

#[test]
fn preview_shows_the_provider_marker_and_model_id_in_the_native_name() {
    let f = Fixture::new();
    let plan = f.plan();
    let name = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == format!("model.{}.name", plan.model_alias))
        .expect("name planned");
    // Readable text, not a slug: the fixed marker, then the model ID untouched.
    assert_eq!(name.after, "(vibemate-Grok-test) deepseek-flash");
    // A slashed, organization-prefixed ID keeps its separators; neither the
    // Provider name nor the user's alias replaces the native name.
    let slashed = model(|m| {
        m.model_id = "deepseek/deepseek-v4.1-flash".into();
        m.alias = Some("Pretty label".into());
        m.upstream_name = Some("Upstream Pretty".into());
    });
    let provider = ProviderRecord {
        display_name: "Command Code GOAT".into(),
        ..f.provider.clone()
    };
    let plan = f.plan_with(&provider, &slashed);
    let name = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == format!("model.{}.name", plan.model_alias))
        .expect("name planned");
    assert_eq!(
        name.after,
        "(vibemate-Command-Code-GOAT) deepseek/deepseek-v4.1-flash"
    );
}

#[test]
fn preview_refuses_when_model_id_matches_an_existing_alias_table() {
    let f = Fixture::new();
    // Handwritten `[model.deepseek-flash]` would collide with `model = "deepseek-flash"`.
    f.write(
        "config.toml",
        &format!(
            "[model.{MODEL}]\nmodel = \"someone-elses\"\nname = \"handwritten\"\napi_key = \"{PRIVATE}\"\n"
        ),
    );
    assert_eq!(
        f.error(&f.provider.clone(), &f.model),
        PlanError::TargetConflict
    );
    // The file is untouched and the private key never leaves the disk content.
    assert!(f.text("config.toml").contains(PRIVATE));
}

#[test]
fn preview_writes_when_model_id_differs_from_existing_alias_names() {
    let f = Fixture::new();
    f.write(
        "config.toml",
        "[model.my-flash]\nmodel = \"other\"\nname = \"Mine\"\n\n[models]\ndefault = \"my-flash\"\n",
    );
    let plan = f.plan();
    assert_ne!(plan.model_alias, "my-flash");
    assert_ne!(plan.model_alias, MODEL);
    let model_field = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == format!("model.{}.model", plan.model_alias))
        .expect("model planned");
    assert_eq!(model_field.after, MODEL);
    let name = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == format!("model.{}.name", plan.model_alias))
        .expect("name planned");
    assert_eq!(name.after, "(vibemate-Grok-test) deepseek-flash");
}

#[test]
fn preview_updates_an_owned_binding_to_the_current_name_format() {
    let f = Fixture::new();
    let alias = f.plan().model_alias;
    f.write(
        "config.toml",
        &format!(
            "[model.{alias}]\nmodel = \"{MODEL}\"\nname = \"My flash\"\napi_key = \"{PRIVATE}\"\n"
        ),
    );
    f.bind(&alias);
    let plan = f.plan();
    assert_eq!(plan.model_alias, alias);
    assert!(plan.existing_alias);
    let name = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == format!("model.{alias}.name"))
        .expect("name planned");
    assert_eq!(name.before.as_deref(), Some("My flash"));
    assert_eq!(name.after, "(vibemate-Grok-test) deepseek-flash");
}

#[test]
fn preview_refuses_a_disabled_or_unlisted_alias() {
    let f = Fixture::new();
    let alias = f.plan().model_alias;
    f.write(
        "config.toml",
        &format!("[models]\ndisabled_models = [\"{alias}\"]\n"),
    );
    assert_eq!(
        f.error(&f.provider.clone(), &f.model),
        PlanError::TargetConflict
    );
    // An allowlist naming neither the alias nor the model id cannot select it.
    f.write("config.toml", "[models]\nallowed_models = [\"other\"]\n");
    assert_eq!(
        f.error(&f.provider.clone(), &f.model),
        PlanError::TargetConflict
    );
}

#[test]
fn preview_reports_the_env_key_it_would_outrank_and_keeps_it() {
    let f = Fixture::new();
    let alias = f.plan().model_alias;
    f.write(
        "config.toml",
        &format!(
            "[model.{alias}]\nmodel = \"{MODEL}\"\nenv_key = \"DEEPSEEK_KEY\"\napi_key = \"{PRIVATE}\"\n"
        ),
    );
    f.bind(&alias);
    let plan = f.plan();
    assert_eq!(plan.model_alias, alias);
    assert!(plan.existing_alias);
    let override_notice = plan
        .notices
        .iter()
        .find(|notice| notice.code == NoticeCode::EnvKeyOverridden)
        .expect("taking over an existing env_key must be reported");
    assert_eq!(override_notice.detail.as_deref(), Some(alias.as_str()));
    let paths: Vec<&str> = plan
        .file
        .changes
        .iter()
        .map(|change| change.key_path.as_str())
        .collect();
    assert!(
        !paths.contains(&&format!("model.{alias}.env_key")[..]),
        "env_key is not this plan's field to rewrite: {paths:?}"
    );
    // The stored key it replaces is masked on both sides of the diff.
    let key = plan
        .file
        .changes
        .iter()
        .find(|change| change.key_path == format!("model.{alias}.api_key"))
        .expect("api_key planned");
    assert_eq!(key.before.as_deref(), Some(SECRET_PLACEHOLDER));
    assert_eq!(key.after, SECRET_PLACEHOLDER);
    assert!(key.secret);
}

#[test]
fn preview_refuses_a_key_that_holds_another_shape() {
    let f = Fixture::new();
    let alias = f.plan().model_alias;
    // `model` as a number is the user's data, so the plan stops rather than
    // replacing it with text.
    f.write(
        "config.toml",
        &format!("[model.{alias}]\nmodel = 12\n[models]\ndefault = 7\n"),
    );
    f.bind(&alias);
    assert_eq!(
        f.error(&f.provider.clone(), &f.model),
        PlanError::TargetConflict
    );
}

#[test]
fn preview_refuses_a_document_that_does_not_parse() {
    let f = Fixture::new();
    f.write("config.toml", "[models\ndefault = \n");
    assert_eq!(
        f.error(&f.provider.clone(), &f.model),
        PlanError::TargetUnreadable
    );
}

#[test]
fn preview_refuses_an_anthropic_backend_whose_credential_header_is_unverified() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let messages =
        model(|model| model.resolved_protocol = Some(ProviderProtocol::AnthropicMessages));
    assert_eq!(
        f.error(&f.provider.clone(), &messages),
        PlanError::ProtocolUnsupported
    );
}

#[test]
fn preview_refuses_a_protocol_switch_on_a_non_default_address() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    // The instance answers a different protocol from the model's route, and its
    // address is not the vendor's documented one for either, so there is no
    // verified base URL to write.
    let mut provider = f.provider.clone();
    provider.protocol = ProviderProtocol::Responses;
    provider.base_url = "https://gateway.example/v1".into();
    assert_eq!(f.error(&provider, &f.model), PlanError::BaseUrlUnverified);
}

#[test]
fn plan_rejects_a_model_that_is_not_part_of_the_selected_provider() {
    let f = Fixture::new();
    let other = model(|model| model.provider_id = "f".repeat(32));
    assert_eq!(
        f.error(&f.provider.clone(), &other),
        PlanError::InvalidRequest
    );
    let deselected = model(|model| model.selected = false);
    assert_eq!(
        f.error(&f.provider.clone(), &deselected),
        PlanError::InvalidRequest
    );
    let unrouted = model(|model| model.resolved_protocol = None);
    assert_eq!(
        f.error(&f.provider.clone(), &unrouted),
        PlanError::ProtocolUnsupported
    );
}

#[test]
fn two_previews_of_one_selection_agree_on_the_alias() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    // The apply step re-derives the plan, so an unstable alias would always look
    // stale.
    assert_eq!(f.plan().model_alias, f.plan().model_alias);
    assert_eq!(
        f.plan().file.target.content_digest,
        f.plan().file.target.content_digest
    );
}

#[test]
fn root_accepts_only_the_installed_version_it_can_verify() {
    let f = Fixture::new();
    let environment = f.installed();
    let entry = f.entry("downloads/grok-1.0.60-macos-aarch64");
    assert_eq!(
        configuration_root(&environment, entry.clone()),
        Ok(f.root.clone())
    );
    // A marker written by a launch of another build is not this contract.
    f.write(".metadata_version", "1.0.52");
    assert_eq!(
        configuration_root(&environment, entry.clone()),
        Err(PlanError::AgentVersionUnverified)
    );
    f.write(".metadata_version", "1.0.60");
    // The reference must still document the field the plan writes.
    f.write(
        "docs/user-guide/26-config-reference.md",
        "# Configuration reference\nonly old rows here\n",
    );
    assert_eq!(
        configuration_root(&environment, entry),
        Err(PlanError::AgentVersionUnverified)
    );
}

#[test]
fn root_refuses_an_entry_with_no_version_claim() {
    let f = Fixture::new();
    let environment = f.installed();
    // An unversioned download name carries no version evidence.
    fs::write(f.root.join("downloads/grok-macos-aarch64"), "binary").unwrap();
    assert_eq!(
        configuration_root(&environment, f.entry("downloads/grok-macos-aarch64")),
        Err(PlanError::AgentVersionUnverified)
    );
    // Neither does a 1.0.52 build, which predates the verified reference.
    fs::write(f.root.join("downloads/grok-1.0.52-macos-aarch64"), "binary").unwrap();
    assert_eq!(
        configuration_root(&environment, f.entry("downloads/grok-1.0.52-macos-aarch64")),
        Err(PlanError::AgentVersionUnverified)
    );
}

#[cfg(unix)]
#[test]
fn root_refuses_a_symbolic_link_in_place_of_the_version_marker() {
    let f = Fixture::new();
    let environment = f.installed();
    let entry = f.entry("downloads/grok-1.0.60-macos-aarch64");
    // The marker must come from the launch itself; a link the adapter did not
    // create is not that claim.
    fs::remove_file(f.root.join(".metadata_version")).unwrap();
    fs::write(f.root.join("other-version"), "1.0.60").unwrap();
    std::os::unix::fs::symlink(
        f.root.join("other-version"),
        f.root.join(".metadata_version"),
    )
    .unwrap();
    assert_eq!(
        configuration_root(&environment, entry),
        Err(PlanError::AgentVersionUnverified)
    );
}

#[cfg(unix)]
#[test]
fn preview_refuses_a_symbolic_linked_settings_file() {
    let f = Fixture::new();
    f.write("real.toml", USER_CONFIG);
    std::os::unix::fs::symlink(f.root.join("real.toml"), f.root.join("config.toml")).unwrap();
    assert_eq!(
        f.error(&f.provider.clone(), &f.model),
        PlanError::TargetConflict
    );
}

/// The synthetic key an apply step reads from the fake OS store.
const KEY: &str = "synthetic-grok-key";

impl Fixture {
    /// Every token one preview showed, ready to send back to the apply step.
    fn reviewed(&self) -> ReviewedGrokPlan {
        let plan = self.plan();
        ReviewedGrokPlan {
            model_alias: plan.model_alias,
            file: ReviewedGrokFile {
                path: plan.file.target.path,
                content_digest: plan.file.target.content_digest,
            },
            provider_revision: plan.source.provider_revision,
            model_content_digest: plan.source.model_content_digest,
        }
    }

    fn store(&self) -> crate::credentials::fake::FakeStore {
        crate::credentials::fake::FakeStore::holding(&format!("provider-{ID}"), KEY)
    }

    fn apply_with(
        &self,
        reviewed: &ReviewedGrokPlan,
        store: &crate::credentials::fake::FakeStore,
    ) -> Result<ConfigApplied, ConfigApplyError> {
        apply_grok_settings(
            &self.storage,
            store,
            &self.root,
            &self.provider,
            &self.model,
            reviewed,
            self.root.join("backups"),
        )
    }

    fn apply(&self, reviewed: &ReviewedGrokPlan) -> Result<ConfigApplied, ConfigApplyError> {
        let store = self.store();
        self.apply_with(reviewed, &store)
    }

    /// How many aliases this configuration root owns.
    fn bindings(&self) -> Vec<(String, String, String)> {
        self.storage
            .lock()
            .unwrap()
            .prepare("SELECT model_alias, provider_id, model_id FROM grok_model_binding")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }
}

#[test]
fn apply_writes_the_key_and_keeps_every_field_the_plan_does_not_own() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let original = f.text("config.toml");
    let applied = f.apply(&f.reviewed()).expect("apply");
    assert_eq!(applied.state, OperationState::Applied);
    assert_eq!(applied.steps.len(), 1);
    assert!(applied.restart_required);
    let written = f.text("config.toml");
    // The backup holds exactly what the write replaced, including the user's key.
    assert_eq!(
        f.text(applied.steps[0].backup_path.as_ref().unwrap()),
        original
    );
    // Unowned content survives: the comment, another model with its key, the
    // reasoning-effort rows, an MCP server and the UI table.
    for keep in [
        "# my Grok settings",
        "default = \"my-model\"",
        "[model.my-model]",
        "api_key = \"user-private-key\"",
        "[[model.my-model.reasoning_efforts]]",
        "[mcp_servers.codegraph]",
        "[ui]",
    ] {
        assert!(written.contains(keep), "lost {keep}");
    }
    let alias = f.plan().model_alias;
    assert!(written.contains(&format!("[model.{alias}]")));
    assert!(written.contains(&format!("api_key = \"{KEY}\"")));
    // The picker shows readable text; the table name stays the alias.
    assert!(written.contains("name = \"(vibemate-Grok-test) deepseek-flash\""));
    assert!(written.contains("api_backend = \"chat_completions\""));
    assert!(written.contains("context_window = 128000"));
    // The user's default model is preserved, not repointed at the new alias.
    assert!(!written.contains(&format!("default = \"{alias}\"")));
    // The answer carries paths and digests, never the file it wrote or the key.
    let rendered = serde_json::to_string(&applied).unwrap();
    assert!(!rendered.contains(KEY));
    assert!(!rendered.contains(PRIVATE));
    // Ownership is recorded so the next preview reuses this alias instead of
    // leaving an orphan entry behind.
    assert_eq!(
        f.bindings(),
        vec![(alias.clone(), ID.to_owned(), MODEL.to_owned())]
    );
    // The record also says when this alias was written; the history view uses
    // that timestamp to show vibemate's last write instead of Grok's default.
    let written_at: i64 = f
        .storage
        .lock()
        .unwrap()
        .query_row(
            "SELECT last_written_at FROM grok_model_binding WHERE model_alias=?1",
            [&alias],
            |row| row.get(0),
        )
        .unwrap();
    assert!(written_at > 0);
}

#[cfg(unix)]
#[test]
fn apply_keeps_the_settings_file_private_when_it_holds_a_key() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    // Grok writes this file itself with owner-only bits on this machine, and a
    // plain-text key must not become world-readable through vibemate.
    f.write("config.toml", "x = 1");
    fs::set_permissions(
        f.root.join("config.toml"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let applied = f.apply(&f.reviewed()).expect("apply");
    assert_eq!(applied.state, OperationState::Applied);
    let mode = fs::metadata(f.root.join("config.toml"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let backup = applied.steps[0].backup_path.as_ref().unwrap();
    assert_eq!(
        fs::metadata(backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn apply_refuses_every_token_that_moved_after_the_preview() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let reviewed = f.reviewed();
    // A concurrent edit by the user or another tool.
    f.write("config.toml", &format!("{USER_CONFIG}\n[extra]\n"));
    assert_eq!(f.apply(&reviewed).unwrap_err(), ConfigApplyError::PlanStale);
    // The file itself is unchanged again, but each stored identity moved.
    f.write("config.toml", USER_CONFIG);
    let mut wrong_alias = f.reviewed();
    wrong_alias.model_alias = "vibemate-someone-else".to_owned();
    let mut wrong_revision = f.reviewed();
    wrong_revision.provider_revision += 1;
    let mut wrong_model = f.reviewed();
    wrong_model.model_content_digest = "c".repeat(64);
    let mut wrong_digest = f.reviewed();
    wrong_digest.file.content_digest = Some("d".repeat(64));
    for stale in [wrong_alias, wrong_revision, wrong_model, wrong_digest] {
        assert_eq!(f.apply(&stale).unwrap_err(), ConfigApplyError::PlanStale);
    }
    assert_eq!(f.text("config.toml"), USER_CONFIG);
    assert!(!f.root.join("backups").exists());
    assert!(f.bindings().is_empty());
}

#[test]
fn apply_reads_the_key_only_from_the_credential_store() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let reviewed = f.reviewed();
    let empty = crate::credentials::fake::FakeStore::default();
    assert_eq!(
        f.apply_with(&reviewed, &empty).unwrap_err(),
        ConfigApplyError::CredentialMissing
    );
    let failing = crate::credentials::fake::FakeStore::default();
    failing.fail_loads(Some(crate::credentials::CredentialError::Unavailable));
    assert_eq!(
        f.apply_with(&reviewed, &failing).unwrap_err(),
        ConfigApplyError::CredentialUnavailable
    );
    // Neither refusal wrote anything or claimed the alias.
    assert_eq!(f.text("config.toml"), USER_CONFIG);
    assert!(f.bindings().is_empty());
}

#[test]
fn a_grok_write_can_be_restored_byte_for_byte_and_stays_out_of_other_histories() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let original = f.text("config.toml");
    let applied = f.apply(&f.reviewed()).expect("apply");
    // Another adapter must not be able to undo this write.
    assert_eq!(
        crate::agents::plan_claude_code_restore(&f.storage, &applied.operation_id).unwrap_err(),
        crate::agents::ClaudeCodeRestoreError::OperationNotFound
    );
    let plan = crate::agents::plan_config_restore(
        &f.storage,
        &applied.operation_id,
        OperationKind::GrokSettings,
    )
    .expect("restore plan");
    assert_eq!(plan.steps.len(), 1);
    let request = crate::agents::ApplyClaudeCodeRestoreRequest {
        operation_id: applied.operation_id.clone(),
        reviewed: plan
            .steps
            .iter()
            .map(|step| crate::agents::ReviewedRestoreStepRequest {
                ordinal: step.ordinal,
                current_digest: step.current_digest.clone(),
            })
            .collect(),
    };
    let restored = crate::agents::apply_config_restore(
        &f.storage,
        &request,
        &f.root.join("backups"),
        OperationKind::GrokSettings,
    )
    .expect("restore");
    assert_eq!(restored.state, OperationState::Compensated);
    // The user's file comes back exactly as it was, key and comments included.
    assert_eq!(f.text("config.toml"), original);
    // The recorded write and its ownership stay: the alias was written once and
    // the history still describes what happened.
    assert_eq!(f.bindings().len(), 1);
    // The paginated history keeps the entry as a record of what happened.
    let page = crate::config_apply::configuration_history(
        &f.storage,
        &crate::config_apply::HistoryScope::new(
            OperationKind::GrokSettings,
            &f.root,
            &["config.toml"],
        )
        .unwrap(),
        None,
        50,
        &BackupStore::new(f.root.join("backups")),
    )
    .unwrap();
    // The apply and the manual restore each occupy their own entry, newest
    // first: the restore was the last thing that happened.
    assert_eq!(page.entries.len(), 2);
    assert_eq!(
        page.entries[0].intent,
        crate::config_apply::HistoryIntent::Restore
    );
    let apply = page
        .entries
        .iter()
        .find(|entry| entry.intent == crate::config_apply::HistoryIntent::Apply)
        .expect("apply entry");
    assert_eq!(
        apply.source_steps[0].target_path,
        f.root.join("config.toml").to_string_lossy()
    );
    // Another Agent's scope never sees this write.
    assert!(
        crate::config_apply::configuration_history(
            &f.storage,
            &crate::config_apply::HistoryScope::new(
                OperationKind::ClaudeCodeSettings,
                &f.root,
                &["settings.json"],
            )
            .unwrap(),
            None,
            50,
            &BackupStore::new(f.root.join("backups")),
        )
        .unwrap()
        .entries
        .is_empty()
    );
}

#[test]
fn applying_the_same_pair_twice_keeps_one_alias_claim() {
    let f = Fixture::new();
    f.write("config.toml", USER_CONFIG);
    let first = f.apply(&f.reviewed()).expect("first apply");
    // The second application re-derives the plan, finds the alias it already owns
    // and updates that one row instead of claiming a second name for one model.
    let second = f.apply(&f.reviewed()).expect("second apply");
    assert_eq!(second.state, OperationState::Applied);
    assert_eq!(f.plan().model_alias, f.plan().model_alias);
    let bindings = f.bindings();
    assert_eq!(bindings.len(), 1);
    assert_eq!(first.steps[0].target_path, second.steps[0].target_path);
}
