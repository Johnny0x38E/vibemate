//! Isolated Claude Code preview tests. The fixture root replaces a real
//! `~/.claude` directory and is removed when the test ends.
use super::*;
use crate::config_plan::SECRET_PLACEHOLDER;
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

/// SHA-256 of [`SAMPLE_SETTINGS`], computed with `shasum -a 256`.
const SETTINGS_DIGEST: &str = "c2e7c5c20563a37b7b8d8e378fad52a60fa0a2c6e30b71f4b46521dfa7280409";
const SAMPLE_SETTINGS: &str = "{\"theme\":\"dark\",\"permissions\":{\"defaultMode\":\"acceptEdits\"},\"env\":{\"EDITOR\":\"vim\"}}\n";

/// A stand-in model content digest; the value only has to be a SHA-256 token.
const MODEL_DIGEST: &str = "0f4f87db4567232a7f1756aa1534ec1314777b39c3bf5209f87cf9739321cddc";
const DEEPSEEK_BASE: &str = "https://api.deepseek.com/anthropic";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
const COMMAND_CODE_BASE: &str = "https://api.commandcode.ai/provider/v1";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vibemate-claude-code-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn settings(&self, content: &str) {
        fs::write(self.0.join("settings.json"), content).unwrap();
    }

    fn input(&self, kind: ProviderKind, base_url: &str) -> ClaudeCodePlanInput {
        ClaudeCodePlanInput {
            config_root: self.0.clone(),
            provider: ProviderFacts {
                id: "provider-1".to_owned(),
                kind,
                base_url: base_url.to_owned(),
                revision: 3,
            },
            model: ModelFacts {
                id: "deepseek-flash".to_owned(),
                content_digest: MODEL_DIGEST.to_owned(),
            },
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_deepseek_plan_writes_the_verified_gateway_keys() {
    let fixture = Fixture::new();
    let plan =
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    assert_eq!(plan.agent, AgentId::ClaudeCode);
    assert_eq!(plan.changes.len(), 7);
    assert_eq!(plan.changes[0].key_path, "env.ANTHROPIC_BASE_URL");
    assert_eq!(plan.changes[0].after, DEEPSEEK_BASE);
    assert!(!plan.changes[0].secret);
    assert_eq!(plan.changes[1].key_path, "env.ANTHROPIC_AUTH_TOKEN");
    assert_eq!(plan.changes[1].after, SECRET_PLACEHOLDER);
    assert!(plan.changes[1].secret);
    assert_eq!(plan.changes[2].key_path, "env.ANTHROPIC_MODEL");
    assert_eq!(plan.changes[2].after, "deepseek-flash");
    let class_keys: Vec<&str> = plan.changes[3..]
        .iter()
        .map(|change| change.key_path.as_str())
        .collect();
    assert_eq!(
        class_keys,
        [
            "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
            "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
            "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "env.CLAUDE_CODE_SUBAGENT_MODEL",
        ]
    );
    assert!(plan.restart_required);
    assert!(
        plan.notices
            .iter()
            .any(|notice| notice.code == NoticeCode::CredentialFromStore)
    );
    assert!(
        plan.notices
            .iter()
            .any(|notice| notice.code == NoticeCode::OverridesNotChecked)
    );
}

#[test]
fn every_class_model_key_carries_the_selected_model() {
    let fixture = Fixture::new();
    for kind in [ProviderKind::DeepSeek, ProviderKind::OpenRouter] {
        let base = if kind == ProviderKind::DeepSeek {
            DEEPSEEK_BASE
        } else {
            OPENROUTER_BASE
        };
        let plan = plan_claude_code_settings(&fixture.input(kind, base)).unwrap();
        for key in CLASS_MODEL_KEYS {
            let change = plan
                .changes
                .iter()
                .find(|change| change.key_path == *key)
                .unwrap();
            assert_eq!(change.after, "deepseek-flash");
            assert!(!change.secret);
        }
    }
}

#[test]
fn previous_class_model_values_are_reported_and_replaced() {
    let fixture = Fixture::new();
    // The values a working DeepSeek setup carries today: the vendor recipe
    // writes the 1M variant to the Opus/Sonnet classes.
    fixture.settings(
        "{\"env\":{\"ANTHROPIC_DEFAULT_OPUS_MODEL\":\"deepseek-flash[1m]\",\"ANTHROPIC_DEFAULT_HAIKU_MODEL\":\"deepseek-flash\"}}",
    );
    let (plan, document) = build(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    let opus = plan
        .changes
        .iter()
        .find(|change| change.key_path == "env.ANTHROPIC_DEFAULT_OPUS_MODEL")
        .unwrap();
    assert_eq!(opus.before.as_deref(), Some("deepseek-flash[1m]"));
    assert_eq!(opus.after, "deepseek-flash");
    // The plan never invents a context-variant suffix, in either direction.
    assert_eq!(
        document["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"],
        json!("deepseek-flash")
    );
    assert_eq!(plan.changes.len(), 7);
}

#[test]
fn a_stored_model_id_is_written_verbatim_to_the_class_keys() {
    let fixture = Fixture::new();
    // A manually added ID may itself carry the suffix; the plan copies whatever
    // the user stored instead of rewriting it.
    let mut input = fixture.input(ProviderKind::OpenRouter, OPENROUTER_BASE);
    input.model.id = "anthropic/claude-sonnet-4-5[1m]".to_owned();
    let plan = plan_claude_code_settings(&input).unwrap();
    for key in CLASS_MODEL_KEYS {
        let change = plan
            .changes
            .iter()
            .find(|change| change.key_path == *key)
            .unwrap();
        assert_eq!(change.after, "anthropic/claude-sonnet-4-5[1m]");
    }
}

#[test]
fn a_deepseek_openai_base_is_still_a_vendor_address() {
    let fixture = Fixture::new();
    let plan = plan_claude_code_settings(
        &fixture.input(ProviderKind::DeepSeek, "https://api.deepseek.com"),
    )
    .unwrap();
    assert_eq!(plan.changes[0].after, DEEPSEEK_BASE);
}

#[test]
fn an_openrouter_plan_uses_the_gateway_base_and_clears_the_direct_api_key() {
    let fixture = Fixture::new();
    let plan = plan_claude_code_settings(&fixture.input(ProviderKind::OpenRouter, OPENROUTER_BASE))
        .unwrap();
    assert_eq!(plan.changes[0].key_path, "env.ANTHROPIC_BASE_URL");
    assert_eq!(plan.changes[0].after, "https://openrouter.ai/api");
    let cleared = plan
        .changes
        .iter()
        .find(|change| change.key_path == "env.ANTHROPIC_API_KEY")
        .unwrap();
    assert_eq!(cleared.after, "");
    assert!(!cleared.secret);
    assert!(
        plan.notices
            .iter()
            .any(|notice| notice.code == NoticeCode::ProviderLimited)
    );
}

#[test]
fn command_code_is_refused_because_no_official_instructions_exist() {
    let fixture = Fixture::new();
    assert_eq!(
        gateway_status(ProviderKind::CommandCode),
        GatewayStatus::Unverified
    );
    assert_eq!(
        gateway_status(ProviderKind::DeepSeek),
        GatewayStatus::Verified
    );
    assert_eq!(
        gateway_status(ProviderKind::OpenRouter),
        GatewayStatus::VerifiedWithLimits
    );
    assert_eq!(
        plan_claude_code_settings(&fixture.input(ProviderKind::CommandCode, COMMAND_CODE_BASE))
            .unwrap_err(),
        PlanError::GatewayUnverified
    );
}

#[test]
fn an_address_the_vendor_does_not_document_is_refused() {
    let fixture = Fixture::new();
    let mirror = fixture.input(
        ProviderKind::DeepSeek,
        "https://mirror.example.com/anthropic",
    );
    assert_eq!(
        plan_claude_code_settings(&mirror).unwrap_err(),
        PlanError::BaseUrlUnverified
    );
    // OpenRouter's request base is not the Claude Code gateway base.
    let wrong_base = fixture.input(ProviderKind::OpenRouter, "https://openrouter.ai/api");
    assert_eq!(
        plan_claude_code_settings(&wrong_base).unwrap_err(),
        PlanError::BaseUrlUnverified
    );
}

#[test]
fn unrelated_settings_keys_survive_the_preview() {
    let fixture = Fixture::new();
    fixture.settings(SAMPLE_SETTINGS);
    let (plan, document) = build(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    assert_eq!(plan.target.content_digest.as_deref(), Some(SETTINGS_DIGEST));
    assert_eq!(document["theme"], json!("dark"));
    assert_eq!(document["permissions"]["defaultMode"], json!("acceptEdits"));
    assert_eq!(document["env"]["EDITOR"], json!("vim"));
    assert_eq!(document["env"]["ANTHROPIC_MODEL"], json!("deepseek-flash"));
    assert_eq!(
        document["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"],
        json!("deepseek-flash")
    );
    assert_eq!(
        document["env"]["CLAUDE_CODE_SUBAGENT_MODEL"],
        json!("deepseek-flash")
    );
    assert_eq!(
        document["env"]["ANTHROPIC_AUTH_TOKEN"],
        json!(SECRET_PLACEHOLDER)
    );
}

#[test]
fn a_missing_settings_file_is_planned_as_a_new_file() {
    let fixture = Fixture::new();
    let plan =
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    assert!(!plan.target.exists);
    assert_eq!(plan.target.content_digest, None);
    assert_eq!(
        plan.target.path,
        fixture.root().join("settings.json").to_str().unwrap()
    );
    assert_eq!(plan.changes[0].before, None);
}

#[test]
fn the_plan_binds_the_settings_provider_and_model_tokens() {
    let fixture = Fixture::new();
    fixture.settings(SAMPLE_SETTINGS);
    let plan =
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    assert!(plan.target.exists);
    assert_eq!(plan.target.content_digest.as_deref(), Some(SETTINGS_DIGEST));
    assert_eq!(plan.source.provider_id, "provider-1");
    assert_eq!(plan.source.provider_revision, 3);
    assert_eq!(plan.source.model_id, "deepseek-flash");
    assert_eq!(plan.source.model_content_digest, MODEL_DIGEST);
}

#[test]
fn an_existing_model_key_is_reported_as_overridden_and_left_alone() {
    let fixture = Fixture::new();
    fixture.settings("{\"model\":\"opus\"}");
    let (plan, document) = build(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    let notice = plan
        .notices
        .iter()
        .find(|notice| notice.code == NoticeCode::ModelKeyOverridden)
        .unwrap();
    assert_eq!(notice.detail.as_deref(), Some("opus"));
    assert_eq!(document["model"], json!("opus"));
}

#[test]
fn a_credential_is_previewed_as_a_placeholder_only() {
    let fixture = Fixture::new();
    let (plan, document) = build(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    let change = plan
        .changes
        .iter()
        .find(|change| change.key_path == "env.ANTHROPIC_AUTH_TOKEN")
        .unwrap();
    assert!(change.secret);
    assert_eq!(change.after, SECRET_PLACEHOLDER);
    assert_eq!(
        document["env"]["ANTHROPIC_AUTH_TOKEN"],
        json!(SECRET_PLACEHOLDER)
    );
}

#[test]
fn the_plan_serializes_the_shape_the_interface_validates() {
    let fixture = Fixture::new();
    fixture.settings(SAMPLE_SETTINGS);
    let plan =
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    // The interface rejects an unexpected key or code as `invalid_response`, so
    // this pins the exact wire shape instead of only the Rust fields.
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        json!({
            "agent": "claudeCode",
            "target": {
                "path": fixture.root().join("settings.json").to_str().unwrap(),
                "exists": true,
                "contentDigest": SETTINGS_DIGEST,
                "symlink": false,
            },
            "source": {
                "providerId": "provider-1",
                "providerRevision": 3,
                "modelId": "deepseek-flash",
                "modelContentDigest": MODEL_DIGEST,
            },
            "changes": [
                {
                    "keyPath": "env.ANTHROPIC_BASE_URL",
                    "before": null,
                    "after": DEEPSEEK_BASE,
                    "secret": false,
                },
                {
                    "keyPath": "env.ANTHROPIC_AUTH_TOKEN",
                    "before": null,
                    "after": SECRET_PLACEHOLDER,
                    "secret": true,
                },
                {
                    "keyPath": "env.ANTHROPIC_MODEL",
                    "before": null,
                    "after": "deepseek-flash",
                    "secret": false,
                },
                {
                    "keyPath": "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
                    "before": null,
                    "after": "deepseek-flash",
                    "secret": false,
                },
                {
                    "keyPath": "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
                    "before": null,
                    "after": "deepseek-flash",
                    "secret": false,
                },
                {
                    "keyPath": "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
                    "before": null,
                    "after": "deepseek-flash",
                    "secret": false,
                },
                {
                    "keyPath": "env.CLAUDE_CODE_SUBAGENT_MODEL",
                    "before": null,
                    "after": "deepseek-flash",
                    "secret": false,
                },
            ],
            "notices": [
                { "code": "credentialFromStore", "detail": null },
                { "code": "overridesNotChecked", "detail": null },
            ],
            "restartRequired": true,
        })
    );
}

#[test]
fn notice_codes_serialize_as_the_interface_lists_them() {
    // Mirrors `PLAN_NOTICE_CODES` in `src/lib/desktop/agents.ts`.
    assert_eq!(
        serde_json::to_value([
            NoticeCode::CredentialFromStore,
            NoticeCode::OverridesNotChecked,
            NoticeCode::ModelKeyOverridden,
            NoticeCode::ProviderLimited,
        ])
        .unwrap(),
        json!([
            "credentialFromStore",
            "overridesNotChecked",
            "modelKeyOverridden",
            "providerLimited",
        ])
    );
}

#[test]
fn a_malformed_revision_model_id_or_digest_is_refused() {
    let fixture = Fixture::new();
    let valid = |input: ClaudeCodePlanInput| input;
    let mut stale_revision = valid(fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE));
    stale_revision.provider.revision = 0;
    assert_eq!(
        plan_claude_code_settings(&stale_revision).unwrap_err(),
        PlanError::InvalidRequest
    );
    for model_id in ["", "with space"] {
        let mut input = valid(fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE));
        input.model.id = model_id.to_owned();
        assert_eq!(
            plan_claude_code_settings(&input).unwrap_err(),
            PlanError::InvalidRequest
        );
    }
    for digest in ["short", &MODEL_DIGEST.to_uppercase(), &"g".repeat(64)] {
        let mut input = valid(fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE));
        input.model.content_digest = digest.to_owned();
        assert_eq!(
            plan_claude_code_settings(&input).unwrap_err(),
            PlanError::InvalidRequest
        );
    }
}

#[test]
fn a_non_object_env_block_is_refused() {
    let fixture = Fixture::new();
    fixture.settings("{\"env\":[1,2]}");
    assert_eq!(
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE))
            .unwrap_err(),
        PlanError::TargetConflict
    );
}

#[test]
fn an_unreadable_settings_file_is_refused() {
    let fixture = Fixture::new();
    fixture.settings("not json");
    assert_eq!(
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE))
            .unwrap_err(),
        PlanError::TargetUnreadable
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_settings_file_is_marked() {
    let fixture = Fixture::new();
    let real = fixture.root().join("real-settings.json");
    fs::write(&real, SAMPLE_SETTINGS).unwrap();
    std::os::unix::fs::symlink(&real, fixture.root().join("settings.json")).unwrap();
    let plan =
        plan_claude_code_settings(&fixture.input(ProviderKind::DeepSeek, DEEPSEEK_BASE)).unwrap();
    assert!(plan.target.symlink);
    assert_eq!(plan.target.content_digest.as_deref(), Some(SETTINGS_DIGEST));
}
