//! Pi model configuration: credential-blind preview and recoverable application.
//!
//! Owns a provider namespace per saved instance, preserves other providers/models,
//! and writes models before the default selection. It never runs Pi, an extension
//! or a credential command. The contract is verified against Pi 1.2.0, 1.3.0 and
//! 1.4.0, with installed schema checks.

use super::{AgentId, ConfigApplied, ConfigApplyError, DiscoveryEnvironment, DiscoveryInput};
use crate::config_apply::{
    BackupStore, OperationKind, OperationState, PlannedFile, WriteTarget, apply_private_files,
    has_recorded_target,
};
use crate::config_plan::{
    ConfigChange, PlanError, PlanSource, SECRET_PLACEHOLDER, TargetFile, read_target_document,
};
use crate::credentials::CredentialStore;
use crate::model_catalog::CapabilityState;
use crate::models::{ModelSnapshot, ProviderModel};
use crate::providers::{ProviderKind, ProviderProtocol, ProviderRecord, base_url_for_protocol};
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

mod facts;
mod thinking;
mod thinking_mapping;

use facts::capability_source;
pub use facts::{PiFactItem, PiFactSource, PiFactState, PiModelFact};
pub use thinking::PiThinkingLevel;
pub(crate) use thinking::reviewed_level;

/// The two files the user reviews, in application order.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiPlannedFile {
    /// File identity and exact-byte concurrency token.
    pub target: TargetFile,
    /// Field changes; credentials are always masked on both sides.
    pub changes: Vec<ConfigChange>,
}

/// A Pi preview contains no merged documents or credential values.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiPlan {
    /// Fixed adapter identity.
    pub agent: AgentId,
    /// Readable native provider key, distinct from the central identity.
    pub provider_key: String,
    /// Previous managed key moved by this plan, absent when nothing moves.
    pub previous_provider_key: Option<String>,
    /// Models first, default settings second.
    pub files: Vec<PiPlannedFile>,
    /// Read-only credential precedence dependency, checked again on apply.
    pub auth_target: TargetFile,
    /// Saved provider and model identity and change tokens.
    pub source: PlanSource,
    /// A fresh session reads the new defaults; resumed sessions can override them.
    pub restart_required: bool,
    /// Saved thinking level written for this model, or `None` when the saved value stays.
    pub selected_thinking_level: Option<PiThinkingLevel>,
    /// Where each model fact came from and what this plan does with it, in display order.
    pub model_facts: Vec<PiModelFact>,
}

/// A reviewed file token; arbitrary paths are compared, never used as targets.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedPiFile {
    /// Absolute path returned by the preview.
    pub path: String,
    /// SHA-256 of the file, absent exactly when the file was absent.
    pub content_digest: Option<String>,
}

/// Tokens from the Pi preview, including the credential precedence dependency.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedPiPlan {
    /// Reviewed readable key and optional migration source.
    pub provider_key: String,
    /// Managed key the preview will rename, if any.
    pub previous_provider_key: Option<String>,
    /// Both writable files, in the order shown.
    pub files: Vec<ReviewedPiFile>,
    /// Read-only auth.json token.
    pub auth_target: ReviewedPiFile,
    /// Stored Provider revision.
    pub provider_revision: i64,
    /// Stored model content digest.
    pub model_content_digest: String,
    /// The thinking level the reviewed preview writes, or `None` for "do not overwrite".
    pub selected_thinking_level: Option<PiThinkingLevel>,
}

/// Strict IPC request. The entry path selects an installed Pi, never an output path.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyPiSettingsRequest {
    /// Saved Provider identity.
    pub provider_id: String,
    /// Saved model identity.
    pub model_id: String,
    /// Same entry the detail page discovered; no program is launched.
    pub entry_path: Option<String>,
    /// Tokens of the complete preview.
    pub reviewed: ReviewedPiPlan,
}

/// Resolve Pi's user directory and verify the installed configuration contract.
///
/// Supports package entries and the official managed launcher. Metadata is
/// bounded and tied to the selected entry; launcher text is never evaluated.
/// Returns `AgentVersionUnverified` for unsupported formats or unrecognized entries,
/// and `LocationUnavailable` for invalid directory overrides. No files change.
pub fn configuration_root(
    environment: &DiscoveryEnvironment,
    entry: Option<String>,
) -> Result<PathBuf, PlanError> {
    let observations = super::discover_agents(
        environment,
        &DiscoveryInput {
            pi_path: entry,
            ..DiscoveryInput::default()
        },
    )
    .map_err(|_| PlanError::InvalidRequest)?;
    let observation = observations
        .iter()
        .find(|row| row.id == AgentId::Pi)
        .ok_or(PlanError::AgentVersionUnverified)?;
    let root = observation
        .config_directory
        .as_ref()
        .map(PathBuf::from)
        .ok_or(PlanError::LocationUnavailable)?;
    let resolved = observation
        .resolved_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or(PlanError::AgentVersionUnverified)?;
    if observation
        .version
        .as_deref()
        .is_some_and(supported_version)
    {
        verify_schema_contract(&resolved)?;
        return Ok(root);
    }
    // Managed installs keep a stable launcher outside the release package.
    let managed = environment.home.join(".pi/agent");
    let launcher = fs::canonicalize(managed.join("bin/pi")).ok();
    if launcher.as_ref().and_then(|p| p.to_str()) != observation.resolved_path.as_deref()
        || launcher.is_none()
    {
        return Err(PlanError::AgentVersionUnverified);
    }
    let current = managed.join("install/current-version");
    let metadata = fs::symlink_metadata(&current).map_err(|_| PlanError::AgentVersionUnverified)?;
    if !metadata.is_file() || metadata.len() > 64 {
        return Err(PlanError::AgentVersionUnverified);
    }
    let version = fs::read_to_string(current).map_err(|_| PlanError::AgentVersionUnverified)?;
    let version = version.trim();
    if !supported_version(version) {
        return Err(PlanError::AgentVersionUnverified);
    }
    let bin = managed
        .join("install/releases")
        .join(version)
        .join("node_modules/.bin/pi");
    let resolved = fs::canonicalize(bin).map_err(|_| PlanError::AgentVersionUnverified)?;
    if super::discovery::pi_version(&resolved).as_deref() != Some(version) {
        return Err(PlanError::AgentVersionUnverified);
    }
    verify_schema_contract(&resolved)?;
    Ok(root)
}

/// Accept stable Pi 1.x releases with published schemas (introduced in 1.2).
fn supported_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts[0] == "1"
        && parts[1].parse::<u32>().is_ok_and(|minor| minor >= 2)
        && parts[2].parse::<u32>().is_ok()
}

/// Inspect only bounded, regular JSON schema files from the selected package.
/// No JavaScript, CLI, extension or credential command is executed.
fn verify_schema_contract(entry: &Path) -> Result<(), PlanError> {
    for directory in entry
        .parent()
        .ok_or(PlanError::AgentVersionUnverified)?
        .ancestors()
        .take(8)
    {
        let models = directory.join("schemas/models.schema.json");
        if !models.exists() {
            continue;
        }
        let read = |path: &Path| -> Result<Value, PlanError> {
            let m = fs::symlink_metadata(path).map_err(|_| PlanError::AgentVersionUnverified)?;
            if !m.is_file() || m.len() > 1024 * 1024 {
                return Err(PlanError::AgentVersionUnverified);
            }
            serde_json::from_slice(&fs::read(path).map_err(|_| PlanError::AgentVersionUnverified)?)
                .map_err(|_| PlanError::AgentVersionUnverified)
        };
        let models = read(&models)?;
        let settings = read(&directory.join("schemas/settings.schema.json"))?;
        let provider =
            &models["properties"]["providers"]["patternProperties"]["^.*$"]["properties"];
        let model = &provider["models"]["items"]["properties"];
        let string_fields = [
            (&provider["name"], "string"),
            (&provider["baseUrl"], "string"),
            (&provider["apiKey"], "string"),
            (&provider["api"], "string"),
            (&provider["authHeader"], "boolean"),
            (&model["id"], "string"),
            (&model["name"], "string"),
            (&model["api"], "string"),
            (&model["baseUrl"], "string"),
            (&model["reasoning"], "boolean"),
            (&model["input"], "array"),
            (&model["contextWindow"], "number"),
            (&model["maxTokens"], "number"),
            (&settings["properties"]["defaultProvider"], "string"),
            (&settings["properties"]["defaultModel"], "string"),
        ];
        if string_fields
            .iter()
            .any(|(field, kind)| field["type"].as_str() != Some(kind))
        {
            return Err(PlanError::AgentVersionUnverified);
        }
        if model["thinkingLevelMap"]["$ref"].as_str() != Some("#/$defs/ThinkingLevelMap") {
            return Err(PlanError::AgentVersionUnverified);
        }
        let compat = &models["$defs"]["ProviderCompat"]["properties"];
        let accepts = |schema: &Value, expected: &str| {
            schema["anyOf"].as_array().is_some_and(|variants| {
                variants
                    .iter()
                    .any(|v| v["const"].as_str() == Some(expected))
            })
        };
        if !accepts(&compat["thinkingFormat"], "deepseek")
            || !accepts(&compat["thinkingFormat"], "openrouter")
            || !accepts(&compat["maxTokensField"], "max_tokens")
        {
            return Err(PlanError::AgentVersionUnverified);
        }
        return Ok(());
    }
    Err(PlanError::AgentVersionUnverified)
}

/// Build the full read-only preview for two files, binding auth precedence too.
///
/// Returns safe plan errors for malformed/conflicting files or an unresolved
/// model route. Does not read OS credentials or execute existing config values.
///
/// `selected_thinking_level` is the explicit choice, or `None` to leave the saved
/// level untouched. The choice must be declared for the model and accepted by Pi.
pub fn plan_pi_settings(
    storage: &Storage,
    root: &std::path::Path,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
    selected_thinking_level: Option<PiThinkingLevel>,
) -> Result<PiPlan, PlanError> {
    build(storage, root, provider, snapshot, selected_thinking_level).map(|(plan, _)| plan)
}

/// Keep merged documents private: unrelated content can contain credentials.
pub(crate) fn build(
    storage: &Storage,
    root: &std::path::Path,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
    selected_thinking_level: Option<PiThinkingLevel>,
) -> Result<(PiPlan, Vec<Map<String, Value>>), PlanError> {
    let model = &snapshot.model;
    if !model.selected || model.provider_id != provider.id.as_str() {
        return Err(PlanError::InvalidRequest);
    }
    let protocol = model
        .resolved_protocol
        .ok_or(PlanError::ProtocolUnsupported)?;
    let base = if protocol == provider.protocol {
        provider.base_url.clone()
    } else if provider.base_url == base_url_for_protocol(provider.kind, provider.protocol) {
        base_url_for_protocol(provider.kind, protocol).to_owned()
    } else {
        return Err(PlanError::BaseUrlUnverified);
    };
    if fs::symlink_metadata(root.join("settings.json.lock")).is_ok()
        || fs::symlink_metadata(root.join("auth.json.lock")).is_ok()
    {
        return Err(PlanError::TargetConflict);
    }
    let legacy_namespace = format!("vibemate-{}", provider.id.as_str());
    let mut models = read_target_document(&root.join("models.json"))?;
    let mut settings = read_target_document(&root.join("settings.json"))?;
    let auth = read_target_document(&root.join("auth.json"))?;
    if models.target.symlink || settings.target.symlink {
        return Err(PlanError::TargetConflict);
    }
    let providers = object_entry(&mut models.document, "providers")?;
    let (namespace, previous) =
        readable_namespace(storage, root, provider, providers, &models.target.path)?;
    // Native credentials take precedence under either name; never move or execute them.
    if auth.document.contains_key(&namespace)
        || auth.document.contains_key(&legacy_namespace)
        || previous
            .as_ref()
            .is_some_and(|key| auth.document.contains_key(key))
    {
        return Err(PlanError::CredentialConflict);
    }
    if let Some(old) = previous.as_ref() {
        let definition = providers.remove(old).ok_or(PlanError::TargetConflict)?;
        providers.insert(namespace.clone(), definition);
    }
    let definition = object_entry(providers, &namespace)?;
    let prefix = format!("providers[{namespace}]");
    let mut model_changes = Vec::new();
    if let Some(old) = previous.as_ref() {
        model_changes.push(ConfigChange {
            key_path: "providers".into(),
            before: Some(old.clone()),
            after: namespace.clone(),
            secret: false,
        });
    }
    set_value(
        definition,
        "name",
        json!(provider.display_name),
        &prefix,
        false,
        &mut model_changes,
    )?;
    set_value(
        definition,
        "baseUrl",
        json!(base),
        &prefix,
        false,
        &mut model_changes,
    )?;
    set_value(
        definition,
        "api",
        json!(match protocol {
            ProviderProtocol::ChatCompletions => "openai-completions",
            ProviderProtocol::Responses => "openai-responses",
            ProviderProtocol::AnthropicMessages => "anthropic-messages",
        }),
        &prefix,
        false,
        &mut model_changes,
    )?;
    set_value(
        definition,
        "apiKey",
        json!(SECRET_PLACEHOLDER),
        &prefix,
        true,
        &mut model_changes,
    )?;
    // All three saved vendors document Bearer authentication, including gateways.
    set_value(
        definition,
        "authHeader",
        json!(true),
        &prefix,
        false,
        &mut model_changes,
    )?;
    let entries = definition
        .entry("models")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or(PlanError::TargetConflict)?;
    if entries
        .iter()
        .any(|item| !item.is_object() || !item["id"].is_string())
    {
        return Err(PlanError::TargetConflict);
    }
    let matches: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, m)| m["id"].as_str() == Some(model.model_id.as_str()))
        .map(|(i, _)| i)
        .collect();
    if matches.len() > 1 {
        return Err(PlanError::TargetConflict);
    }
    let index = match matches.first() {
        Some(i) => *i,
        None => {
            entries.push(json!({}));
            entries.len() - 1
        }
    };
    let selected = entries[index]
        .as_object_mut()
        .ok_or(PlanError::TargetConflict)?;
    let prefix = format!("{prefix}.models[{}]", model.model_id);
    set_value(
        selected,
        "id",
        json!(model.model_id),
        &prefix,
        false,
        &mut model_changes,
    )?;
    // Pi shows this text in its model picker, so it reads as
    // `(vibemate-<Provider>) <model ID>`. The model `id` remains the identity;
    // `name` is display text only.
    let model_name = crate::shared::agent_model_display_name(
        &provider.display_name,
        &model.model_id,
        crate::shared::MAX_AGENT_MODEL_NAME_CHARS,
    );
    set_value(
        selected,
        "name",
        json!(model_name),
        &prefix,
        false,
        &mut model_changes,
    )?;
    // A user may have added model-level overrides since an earlier application.
    // Show their replacement explicitly so the selected model cannot secretly
    // use a different endpoint or API from the provider shown in this preview.
    set_value(
        selected,
        "baseUrl",
        json!(base),
        &prefix,
        false,
        &mut model_changes,
    )?;
    set_value(
        selected,
        "api",
        json!(match protocol {
            ProviderProtocol::ChatCompletions => "openai-completions",
            ProviderProtocol::Responses => "openai-responses",
            ProviderProtocol::AnthropicMessages => "anthropic-messages",
        }),
        &prefix,
        false,
        &mut model_changes,
    )?;
    let mut model_facts = Vec::new();
    map_metadata(
        selected,
        model,
        &prefix,
        &mut model_changes,
        &mut model_facts,
    )?;
    if protocol == ProviderProtocol::ChatCompletions {
        let format = match provider.kind {
            ProviderKind::DeepSeek if base == "https://api.deepseek.com" => Some("deepseek"),
            ProviderKind::OpenRouter if base == "https://openrouter.ai/api/v1" => {
                Some("openrouter")
            }
            _ => None,
        };
        if let Some(format) = format {
            let compat = object_entry(selected, "compat")?;
            let path = format!("{prefix}.compat");
            set_value(
                compat,
                "thinkingFormat",
                json!(format),
                &path,
                false,
                &mut model_changes,
            )?;
            if provider.kind == ProviderKind::DeepSeek {
                set_value(
                    compat,
                    "maxTokensField",
                    json!("max_tokens"),
                    &path,
                    false,
                    &mut model_changes,
                )?;
            }
        }
    }
    thinking_mapping::inject_declared_levels(
        selected,
        provider,
        model,
        protocol,
        &base,
        &prefix,
        &mut model_changes,
    )?;
    // Check the choice against the merged definition, including any thinking-level
    // map the user wrote, before this plan produces a settings change.
    if let Some(level) = selected_thinking_level {
        let accepted = thinking::accepted_levels(selected)?;
        thinking::validate_selection(model, level, &accepted)?;
    }
    let mut setting_changes = Vec::new();
    if let Some(old) = previous.as_ref() {
        migrate_settings_references(
            &mut settings.document,
            old,
            &namespace,
            &mut setting_changes,
        )?;
    }
    // Pi reads this per-model key before its global default, then clamps it to
    // the model. Other entries and the global default stay exactly as they were.
    // The fact reports what Pi will read after the migration above and this write.
    let level_key = format!("{namespace}/{}", model.model_id);
    if let Some(level) = selected_thinking_level {
        let levels = object_entry(&mut settings.document, "modelThinkingLevels")?;
        let value = json!(level.as_str());
        let fact = PiModelFact::written(
            PiFactItem::ThinkingLevel,
            PiFactSource::Choice,
            "modelThinkingLevels",
            &value,
        );
        set_value(
            levels,
            &level_key,
            value,
            "modelThinkingLevels",
            false,
            &mut setting_changes,
        )?;
        model_facts.push(fact);
    } else {
        let existing = settings
            .document
            .get("modelThinkingLevels")
            .and_then(Value::as_object)
            .and_then(|levels| levels.get(&level_key));
        model_facts.push(PiModelFact::kept_or_unstated(
            PiFactItem::ThinkingLevel,
            PiFactSource::Choice,
            "modelThinkingLevels",
            existing,
        ));
    }
    set_value(
        &mut settings.document,
        "defaultProvider",
        json!(namespace),
        "",
        false,
        &mut setting_changes,
    )?;
    set_value(
        &mut settings.document,
        "defaultModel",
        json!(model.model_id),
        "",
        false,
        &mut setting_changes,
    )?;
    Ok((
        PiPlan {
            agent: AgentId::Pi,
            provider_key: namespace,
            previous_provider_key: previous,
            files: vec![
                PiPlannedFile {
                    target: models.target,
                    changes: model_changes,
                },
                PiPlannedFile {
                    target: settings.target,
                    changes: setting_changes,
                },
            ],
            auth_target: auth.target,
            source: PlanSource {
                provider_id: provider.id.as_str().to_owned(),
                provider_revision: provider.revision,
                model_id: model.model_id.clone(),
                model_content_digest: snapshot.content_digest.clone(),
            },
            restart_required: true,
            selected_thinking_level,
            model_facts,
        },
        vec![models.document, settings.document],
    ))
}

/// Use the saved display name in Pi, which renders provider keys literally.
/// Historical aliases stay recorded so a restored file can be recognized safely.
fn readable_namespace(
    storage: &Storage,
    root: &Path,
    provider: &ProviderRecord,
    providers: &Map<String, Value>,
    target: &str,
) -> Result<(String, Option<String>), PlanError> {
    let root = root.to_str().ok_or(PlanError::TargetPathInvalid)?;
    let connection = storage.lock().map_err(|_| PlanError::StorageUnavailable)?;
    let mut statement = connection
        .prepare("SELECT provider_key, provider_id FROM pi_provider_binding WHERE config_root=?1")
        .map_err(|_| PlanError::StorageUnavailable)?;
    let bindings = statement
        .query_map([root], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| PlanError::StorageUnavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PlanError::StorageUnavailable)?;
    drop(statement);
    drop(connection);
    let legacy = format!("vibemate-{}", provider.id.as_str());
    let mut owned: Vec<String> = bindings
        .iter()
        .filter(|(key, id)| id == provider.id.as_str() && providers.contains_key(key))
        .map(|(key, _)| key.clone())
        .collect();
    if providers.contains_key(&legacy) {
        if bindings.iter().any(|(_, id)| id == provider.id.as_str())
            && !bindings
                .iter()
                .any(|(key, id)| key == &legacy && id == provider.id.as_str())
        {
            return Err(PlanError::TargetConflict);
        }
        if !has_recorded_target(
            storage,
            OperationKind::PiSettings,
            provider.id.as_str(),
            target,
        )
        .map_err(|_| PlanError::StorageUnavailable)?
        {
            return Err(PlanError::TargetConflict);
        }
        owned.push(legacy.clone());
    }
    owned.sort();
    owned.dedup();
    if owned.len() > 1 {
        return Err(PlanError::TargetConflict);
    }
    let mut label = String::new();
    for character in provider.display_name.chars() {
        if character.is_alphanumeric() || character == '_' {
            label.push(character);
        } else if !label.ends_with('-') {
            label.push('-');
        }
    }
    let label = label.trim_matches('-');
    let base = format!(
        "vibemate-{}",
        if label.is_empty() {
            provider.kind.as_str()
        } else {
            label
        }
    );
    let previous = owned.first();
    for candidate in [
        base.clone(),
        format!("{base}-{}", &provider.id.as_str()[..8]),
        format!("{base}-{}", provider.id.as_str()),
    ] {
        let owner = bindings
            .iter()
            .find(|(key, _)| key == &candidate)
            .map(|(_, id)| id.as_str());
        if owner.is_some_and(|id| id != provider.id.as_str()) {
            continue;
        }
        if providers.contains_key(&candidate) && previous != Some(&candidate) {
            continue;
        }
        let moved = previous.filter(|key| **key != candidate).cloned();
        return Ok((candidate, moved));
    }
    Err(PlanError::TargetConflict)
}

/// Remember an alias only after the writer recorded a successful models step.
/// No credential or document content is stored in this table.
fn record_namespace(
    storage: &Storage,
    root: &Path,
    provider: &ProviderRecord,
    key: &str,
) -> Result<(), ConfigApplyError> {
    let root = root.to_str().ok_or(ConfigApplyError::InvalidRequest)?;
    let changed = storage
        .lock()
        .map_err(|_| ConfigApplyError::StorageUnavailable)?
        .execute(
            "INSERT INTO pi_provider_binding(config_root,provider_key,provider_id) VALUES(?1,?2,?3)
         ON CONFLICT(config_root,provider_key) DO UPDATE SET provider_id=excluded.provider_id
         WHERE pi_provider_binding.provider_id=excluded.provider_id",
            params![root, key, provider.id.as_str()],
        )
        .map_err(|_| ConfigApplyError::OperationFailed)?;
    if changed != 1 {
        return Err(ConfigApplyError::OperationFailed);
    }
    Ok(())
}

/// Carry user-level model references with a renamed provider. Unrelated scopes
/// and reasoning choices stay intact; conflicting destination entries are refused.
fn migrate_settings_references(
    settings: &mut Map<String, Value>,
    old: &str,
    new: &str,
    changes: &mut Vec<ConfigChange>,
) -> Result<(), PlanError> {
    let prefix = format!("{old}/");
    if let Some(levels) = settings.get_mut("modelThinkingLevels") {
        let levels = levels.as_object_mut().ok_or(PlanError::TargetConflict)?;
        let moved = levels
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(&prefix)
                    .map(|suffix| (key.clone(), format!("{new}/{suffix}"), value.clone()))
            })
            .collect::<Vec<_>>();
        for (_, target, value) in &moved {
            if levels.get(target).is_some_and(|existing| existing != value) {
                return Err(PlanError::TargetConflict);
            }
        }
        if !moved.is_empty() {
            for (source, target, value) in moved {
                levels.remove(&source);
                levels.insert(target, value);
            }
            changes.push(ConfigChange {
                key_path: "modelThinkingLevels".into(),
                before: Some(old.into()),
                after: new.into(),
                secret: false,
            });
        }
    }
    if let Some(patterns) = settings.get_mut("enabledModels") {
        let patterns = patterns.as_array_mut().ok_or(PlanError::TargetConflict)?;
        let mut changed = false;
        for pattern in patterns {
            let text = pattern.as_str().ok_or(PlanError::TargetConflict)?;
            let replacement = if text == old {
                Some(new.to_owned())
            } else {
                text.strip_prefix(&prefix)
                    .map(|suffix| format!("{new}/{suffix}"))
            };
            if let Some(replacement) = replacement {
                *pattern = Value::String(replacement);
                changed = true;
            }
        }
        if changed {
            changes.push(ConfigChange {
                key_path: "enabledModels".into(),
                before: Some(old.into()),
                after: new.into(),
                secret: false,
            });
        }
    }
    Ok(())
}

/// Apply only explicitly declared metadata; Pi supplies defaults for unknowns.
///
/// Each mapped field appends its [`PiModelFact`] in the same branch that writes it,
/// so a row cannot describe a value the writer did not produce. A field without a
/// central value is left alone and reported as kept or not stated.
fn map_metadata(
    document: &mut Map<String, Value>,
    model: &ProviderModel,
    prefix: &str,
    changes: &mut Vec<ConfigChange>,
    model_facts: &mut Vec<PiModelFact>,
) -> Result<(), PlanError> {
    match model.context_window {
        Some(count) => {
            let value = json!(count);
            let fact = PiModelFact::written(
                PiFactItem::ContextWindow,
                PiFactSource::Library,
                "contextWindow",
                &value,
            );
            set_value(document, "contextWindow", value, prefix, false, changes)?;
            model_facts.push(fact);
        }
        None => model_facts.push(PiModelFact::kept_or_unstated(
            PiFactItem::ContextWindow,
            PiFactSource::NotStated,
            "contextWindow",
            document.get("contextWindow"),
        )),
    }
    match model.max_output_tokens {
        Some(count) => {
            let value = json!(count);
            let fact = PiModelFact::written(
                PiFactItem::MaxOutputTokens,
                PiFactSource::Library,
                "maxTokens",
                &value,
            );
            set_value(document, "maxTokens", value, prefix, false, changes)?;
            model_facts.push(fact);
        }
        None => model_facts.push(PiModelFact::kept_or_unstated(
            PiFactItem::MaxOutputTokens,
            PiFactSource::NotStated,
            "maxTokens",
            document.get("maxTokens"),
        )),
    }
    let thinking = model
        .capability_overrides
        .thinking
        .map(|v| v.as_state())
        .unwrap_or(model.capabilities.thinking.state);
    if thinking != CapabilityState::Unknown {
        let value = json!(thinking == CapabilityState::Supported);
        let source = capability_source(
            model.capability_overrides.thinking.is_some(),
            model.capabilities.thinking.state,
        );
        let fact = PiModelFact::written(PiFactItem::Reasoning, source, "reasoning", &value);
        set_value(document, "reasoning", value, prefix, false, changes)?;
        model_facts.push(fact);
    } else {
        model_facts.push(PiModelFact::kept_or_unstated(
            PiFactItem::Reasoning,
            PiFactSource::NotStated,
            "reasoning",
            document.get("reasoning"),
        ));
    }
    let image = model
        .capability_overrides
        .image_input
        .map(|v| v.as_state())
        .unwrap_or(model.capabilities.image_input.state);
    if image != CapabilityState::Unknown {
        let input = if image == CapabilityState::Supported {
            json!(["text", "image"])
        } else {
            json!(["text"])
        };
        let source = capability_source(
            model.capability_overrides.image_input.is_some(),
            model.capabilities.image_input.state,
        );
        let fact = PiModelFact::written(PiFactItem::ImageInput, source, "input", &input);
        set_value(document, "input", input, prefix, false, changes)?;
        model_facts.push(fact);
    } else {
        model_facts.push(PiModelFact::kept_or_unstated(
            PiFactItem::ImageInput,
            PiFactSource::NotStated,
            "input",
            document.get("input"),
        ));
    }
    Ok(())
}

/// Preserve unrelated fields and refuse an intermediate value of another type.
fn object_entry<'a>(
    document: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, PlanError> {
    document
        .entry(key)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(PlanError::TargetConflict)
}

/// Only bounded, typed fields enter the preview; a credential's old value is
/// masked as well as its replacement, even if it used to be a command string.
fn set_value(
    document: &mut Map<String, Value>,
    key: &str,
    value: Value,
    prefix: &str,
    secret: bool,
    changes: &mut Vec<ConfigChange>,
) -> Result<(), PlanError> {
    let before = document.get(key);
    if before.is_some_and(|old| {
        old.is_object()
            || old.is_null()
            || old.is_array() != value.is_array()
            || old.is_string() != value.is_string()
            || old.is_boolean() != value.is_boolean()
            || old.is_number() != value.is_number()
    }) {
        return Err(PlanError::TargetConflict);
    }
    let display = |value: &Value| {
        value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string())
    };
    changes.push(ConfigChange {
        key_path: if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        },
        before: before.map(|v| {
            if secret {
                SECRET_PLACEHOLDER.to_owned()
            } else {
                display(v)
            }
        }),
        after: if secret {
            SECRET_PLACEHOLDER.to_owned()
        } else {
            display(&value)
        },
        secret,
    });
    document.insert(key.to_owned(), value);
    Ok(())
}

/// Apply both reviewed files through private backups and multi-file compensation.
///
/// Rebuilds and compares every file/provider/model/auth token before reading the
/// OS key. A later failure rolls back earlier writes; no key enters the result.
/// Returns stale/credential/plan errors before writing, or safe log errors.
pub fn apply_pi_settings(
    storage: &Storage,
    credentials: &impl CredentialStore,
    root: &std::path::Path,
    provider: &ProviderRecord,
    snapshot: &ModelSnapshot,
    reviewed: &ReviewedPiPlan,
    backups: PathBuf,
) -> Result<ConfigApplied, ConfigApplyError> {
    let (plan, mut documents) = build(
        storage,
        root,
        provider,
        snapshot,
        reviewed.selected_thinking_level,
    )
    .map_err(ConfigApplyError::from_plan)?;
    if reviewed.provider_key != plan.provider_key
        || reviewed.previous_provider_key != plan.previous_provider_key
        || reviewed.files.len() != plan.files.len()
        || reviewed.files.iter().zip(&plan.files).any(|(old, file)| {
            old.path != file.target.path || old.content_digest != file.target.content_digest
        })
        || reviewed.auth_target.path != plan.auth_target.path
        || reviewed.auth_target.content_digest != plan.auth_target.content_digest
        || reviewed.provider_revision != plan.source.provider_revision
        || reviewed.model_content_digest != plan.source.model_content_digest
    {
        return Err(ConfigApplyError::PlanStale);
    }
    let reference = provider.id.credential_reference();
    let secret = credentials
        .load(&reference)
        .map_err(|_| ConfigApplyError::CredentialUnavailable)?
        .ok_or(ConfigApplyError::CredentialMissing)?;
    // Pi 1.2.0 interprets !command and $VAR even in API keys. Escape both so a
    // stored value is always literal and cannot turn into executable config.
    let mut literal = secret.expose().replace('$', "$$");
    if literal.starts_with('!') {
        literal.replace_range(..1, "$!");
    }
    let namespace = &plan.provider_key;
    documents[0]["providers"][namespace]["apiKey"] = json!(literal);
    let files = plan
        .files
        .iter()
        .zip(&documents)
        .map(|(file, document)| {
            let mut content = serde_json::to_vec_pretty(document)
                .map_err(|_| ConfigApplyError::OperationFailed)?;
            content.push(b'\n');
            Ok(PlannedFile {
                target: WriteTarget::from_plan(&file.target),
                content,
            })
        })
        .collect::<Result<Vec<_>, ConfigApplyError>>()?;
    let result = apply_private_files(
        storage,
        OperationKind::PiSettings,
        Some(provider.id.as_str()),
        &files,
        &BackupStore::new(backups),
    )
    .map_err(ConfigApplyError::from_write)?;
    if result
        .steps
        .first()
        .is_some_and(|step| step.state == crate::config_apply::StepState::Applied)
    {
        if let Some(previous) = &plan.previous_provider_key {
            record_namespace(storage, root, provider, previous)?;
        }
        record_namespace(storage, root, provider, &plan.provider_key)?;
    }
    Ok(ConfigApplied {
        restart_required: result.state == OperationState::Applied,
        operation_id: result.operation_id,
        state: result.state,
        backup_path: result.steps.iter().find_map(|s| s.backup_path.clone()),
        steps: result.steps,
    })
}

#[cfg(test)]
mod tests;
