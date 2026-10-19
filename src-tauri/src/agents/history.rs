//! Read-only history IPC domain boundary for the three verified write adapters.
//!
//! Root selection shares discovery's environment precedence, not installation
//! verification. Removed installations and missing roots must not hide journals.
//! No frontend path, file contents, backup paths or recovery approval cross here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use toml_edit::DocumentMut;

use super::{
    AgentId, ClaudeCodeRestoreError, DiscoveryEnvironment, discovery, recovery::ConfigurationAgent,
};
use crate::{
    config_apply::{
        self, BackupStore, HistoryCurrentConfig, HistoryCurrentFile, HistoryCurrentStatus,
        HistoryCursor, HistoryEntry, HistoryScope,
    },
    config_plan::read_target_document,
    model_identity::is_valid_model_id,
    models::{GetModelSnapshotRequest, get_model_snapshot},
    shared::validate_display_name,
    storage::Storage,
};

/// Strict pagination input; the caller cannot select an arbitrary filesystem root.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationHistoryRequest {
    /// One of the three adapters with recorded configuration writes.
    pub agent: ConfigurationAgent,
    /// Last position returned by this exact Agent/root scope, or null for page one.
    pub after: Option<HistoryCursor>,
    /// Page size from 1 through 50; batches are never split across pages.
    pub limit: u32,
}

/// One journal entry plus adapter-level labels for the history screen.
///
/// `requires_resolution` repeats per operation because a page may surface the
/// original apply and a later restore batch separately. It reads journal
/// metadata only. `owner_label` is display text for a still-saved Provider and
/// never substitutes for the recorded `owner_ref`.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationHistoryEntry {
    /// Shared root-scoped journal metadata; serialized inline for the frontend.
    #[serde(flatten)]
    pub entry: HistoryEntry,
    /// Whether this entry's source operation still has interrupted mutations.
    pub requires_resolution: bool,
    /// Current Provider display name for `ownerRef`, when that row still exists.
    pub owner_label: Option<String>,
}

/// The model the Agent is currently configured to use, classified by owner.
///
/// This reads the live configuration, not history. A model that resolves to a
/// still-saved vibemate Provider/model pair is `Ours`; a readable configuration
/// that names any other model (or no model) is `Own`. `Unknown` means the file
/// could not be read or parsed safely, so the interface must not claim either.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ActiveAgentModel {
    /// Current model resolves to a saved vibemate Provider/model pair.
    Ours {
        /// Provider display name resolved from the saved Provider row.
        provider_label: String,
        /// Model id the Agent configuration names.
        model_id: String,
        /// Saved model alias, or the model id when no alias is saved.
        model_label: String,
    },
    /// The Agent has a readable configuration, but the current model is not ours.
    Own,
    /// The live files could not be read or parsed safely.
    Unknown,
}

/// One enriched history page, including backup availability and the current pin.
/// Still carries no backup paths or file bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationHistoryPage {
    /// Live configuration pin; identical for every page of the same scope.
    pub current: HistoryCurrentConfig,
    /// Model the Agent currently names in its live configuration.
    ///
    /// Independent of [`HistoryCurrentConfig::status`]: an external edit may
    /// stop the file matching a retained version while the current model is
    /// still one vibemate wrote. Classifies the model as ours, the user's own,
    /// or unreadable, for the list-card summary.
    pub active: ActiveAgentModel,
    /// Current written configuration, even when outside this chronological page.
    pub current_entry: Option<ConfigurationHistoryEntry>,
    /// Newest first; recovery batches are retained internally, not listed.
    pub entries: Vec<ConfigurationHistoryEntry>,
    /// Seek position for more entries, absent when this read reached the end.
    pub next_cursor: Option<HistoryCursor>,
}

/// Build the exact file scope for one supported Agent from the desktop environment.
///
/// Shared by history reads and version restore so both use the same root
/// precedence and owned-file list. The environment is trusted desktop state.
pub(super) fn history_scope(
    agent: ConfigurationAgent,
    environment: &DiscoveryEnvironment,
) -> Result<HistoryScope, ClaudeCodeRestoreError> {
    let (agent_id, files): (AgentId, &[&str]) = match agent {
        ConfigurationAgent::ClaudeCode => (AgentId::ClaudeCode, &["settings.json"]),
        ConfigurationAgent::Pi => (AgentId::Pi, &["models.json", "settings.json"]),
        ConfigurationAgent::GrokBuild => (AgentId::GrokBuild, &["config.toml"]),
    };
    let (root, _) = discovery::configuration_directory(agent_id, environment);
    // An invalid explicit override must not silently expose the default root.
    let root = root.ok_or(ClaudeCodeRestoreError::InvalidRequest)?;
    Ok(HistoryScope::new(agent.kind(), &root, files)?)
}

/// Read apply configuration versions in the adapter's active root.
///
/// Restore/compensation batches remain internal journal records. The current
/// apply is loaded separately so it stays available on every seek page.
///
/// The environment is a trusted desktop snapshot, never frontend input. Resolves
/// the root without launching/validating an installed CLI, then queries only the
/// exact files owned by that adapter. Opens private backup files only to report
/// availability; never returns paths or bytes, and does not authorize recovery.
///
/// When `current.status` is `matched`, also attaches the matched Provider's
/// user-custom display name and, for Claude Code and Pi, the model alias-or-id
/// read from the live config files. The separate `active` classification always
/// reads the live model settings, independent of history matching, so the list
/// can distinguish a current vibemate model from the user's own configuration.
///
/// # Errors
/// Returns safe request errors for invalid pagination, invalid/wrong-root cursors
/// or an unusable environment root. Storage failures reveal no raw diagnostics.
pub fn configuration_history_page(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: &ConfigurationHistoryRequest,
    backups: &BackupStore,
) -> Result<ConfigurationHistoryPage, ClaudeCodeRestoreError> {
    let scope = history_scope(request.agent, environment)?;
    let page = config_apply::configuration_versions(
        storage,
        &scope,
        request.after.as_ref(),
        request.limit,
        backups,
    )?;
    // Classify the live model before the entries are consumed. This reads the
    // Agent's own model setting, not a retained version.
    let active = active_agent_model(storage, request.agent, scope.root(), &page.current.files);
    let enrich_entry = |entry: HistoryEntry| {
        // The journal key is a bounded reference; only resolve it as a
        // Provider when the row still exists. A removed Provider keeps its
        // recorded reference and shows as removed in the interface.
        let owner_label = entry.owner_ref.as_deref().and_then(|provider_id| {
            crate::providers::get_provider(storage, provider_id)
                .ok()
                .map(|provider| provider.display_name)
        });
        let requires_resolution =
            config_apply::operation_requires_resolution(storage, &entry.operation_id)?;
        Ok(ConfigurationHistoryEntry {
            entry,
            requires_resolution,
            owner_label,
        })
    };
    let current_entry = page.current_entry.map(enrich_entry).transpose()?;
    let entries = page
        .entries
        .into_iter()
        .map(enrich_entry)
        .collect::<Result<Vec<_>, ClaudeCodeRestoreError>>()?;
    let current = enrich_current(storage, request.agent, page.current, current_entry.as_ref());
    Ok(ConfigurationHistoryPage {
        current,
        active,
        current_entry,
        entries,
        next_cursor: page.next_cursor,
    })
}

/// Attach provider/model display fields only for a fully matched live pin.
fn enrich_current(
    storage: &Storage,
    agent: ConfigurationAgent,
    mut current: HistoryCurrentConfig,
    current_entry: Option<&ConfigurationHistoryEntry>,
) -> HistoryCurrentConfig {
    if current.status != HistoryCurrentStatus::Matched {
        return current;
    }
    // This entry produced the complete live bytes. It is loaded independently
    // of the seek page, never inferred from a pre-write snapshot or restore's
    // source operation.
    current.provider_label = current_entry.and_then(|entry| entry.owner_label.clone());
    let owner_ref = current_entry.and_then(|entry| entry.entry.owner_ref.as_deref());
    let Some(model_id) = live_model_id(agent, &current.files) else {
        return current;
    };
    let model_label = owner_ref
        .and_then(|provider_id| saved_model_alias(storage, provider_id, &model_id))
        .unwrap_or_else(|| model_id.clone());
    current.model_id = Some(model_id);
    current.model_label = Some(model_label);
    current
}

/// Read the model id vibemate wrote into the matched live configuration files.
///
/// Returns none when a file is missing, unreadable, or does not carry a valid id.
/// Does not invent values from unrelated journal or catalog rows.
fn live_model_id(agent: ConfigurationAgent, files: &[HistoryCurrentFile]) -> Option<String> {
    if files.iter().any(|file| !file.readable) {
        return None;
    }
    match agent {
        ConfigurationAgent::ClaudeCode => {
            let path = owned_path(files, "settings.json")?;
            let document = read_target_document(&path).ok()?.document;
            json_string(document.get("env")?.get("ANTHROPIC_MODEL")?)
                .filter(|id| is_valid_model_id(id))
        }
        ConfigurationAgent::Pi => {
            let path = owned_path(files, "settings.json")?;
            let document = read_target_document(&path).ok()?.document;
            json_string(document.get("defaultModel")?).filter(|id| is_valid_model_id(id))
        }
        // Grok Build reports its last write separately in `written`, because
        // its model identity does not come from `[models].default`.
        ConfigurationAgent::GrokBuild => None,
    }
}

fn owned_path(files: &[HistoryCurrentFile], file_name: &str) -> Option<PathBuf> {
    files.iter().find_map(|file| {
        let path = Path::new(&file.target_path);
        (path.file_name()?.to_str()? == file_name).then(|| path.to_path_buf())
    })
}

fn json_string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// Classify the model named by the Agent's current live configuration.
fn active_agent_model(
    storage: &Storage,
    agent: ConfigurationAgent,
    root: &str,
    files: &[HistoryCurrentFile],
) -> ActiveAgentModel {
    if files.iter().any(|file| !file.readable) {
        return ActiveAgentModel::Unknown;
    }
    match agent {
        ConfigurationAgent::ClaudeCode => active_claude_code(storage, files),
        ConfigurationAgent::Pi => active_pi(storage, root, files),
        ConfigurationAgent::GrokBuild => active_grok(storage, root, files),
    }
}

/// Claude Code's session model: `env.ANTHROPIC_MODEL`, then file `model`.
fn active_claude_code(storage: &Storage, files: &[HistoryCurrentFile]) -> ActiveAgentModel {
    let Some(path) = owned_path(files, "settings.json") else {
        return ActiveAgentModel::Unknown;
    };
    let Ok(target) = read_target_document(&path) else {
        return ActiveAgentModel::Unknown;
    };
    let document = target.document;
    let Some(model_id) = document
        .get("env")
        .and_then(|env| env.get("ANTHROPIC_MODEL"))
        .or_else(|| document.get("model"))
        .and_then(Value::as_str)
        .filter(|id| is_valid_model_id(id))
    else {
        return ActiveAgentModel::Own;
    };
    let base_url = document
        .get("env")
        .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
        .and_then(Value::as_str);
    match claude_provider(storage, model_id, base_url) {
        Some(provider_id) => ours(storage, &provider_id, model_id),
        None => ActiveAgentModel::Own,
    }
}

/// Pi's active model: the newest session's recorded switch, else the defaults.
///
/// `/model` in Pi is session-scoped, so `settings.json` can still name the
/// startup model after the user has switched. The newest session's tail is the
/// only local record of the model actually in use; when no session metadata is
/// available, the settings defaults remain the best observation.
fn active_pi(storage: &Storage, root: &str, files: &[HistoryCurrentFile]) -> ActiveAgentModel {
    if let Some((provider_key, model_id)) = super::pi_session::latest_model(Path::new(root)) {
        if !is_valid_model_id(&model_id) {
            return ActiveAgentModel::Own;
        }
        return match pi_binding(storage, root, &provider_key) {
            Some(provider_id) => ours(storage, &provider_id, &model_id),
            None => ActiveAgentModel::Own,
        };
    }
    let Some(path) = owned_path(files, "settings.json") else {
        return ActiveAgentModel::Unknown;
    };
    let Ok(target) = read_target_document(&path) else {
        return ActiveAgentModel::Unknown;
    };
    let document = target.document;
    let Some(model_id) = document
        .get("defaultModel")
        .and_then(Value::as_str)
        .filter(|id| is_valid_model_id(id))
    else {
        return ActiveAgentModel::Own;
    };
    let provider_id = match document.get("defaultProvider").and_then(Value::as_str) {
        Some(key) => pi_binding(storage, root, key),
        None => provider_by_model(storage, model_id),
    };
    match provider_id {
        Some(provider_id) => ours(storage, &provider_id, model_id),
        None => ActiveAgentModel::Own,
    }
}

/// Grok's `[models].default` alias resolved through its alias binding.
fn active_grok(storage: &Storage, root: &str, files: &[HistoryCurrentFile]) -> ActiveAgentModel {
    let Some(path) = owned_path(files, "config.toml") else {
        return ActiveAgentModel::Unknown;
    };
    let Some(document) = read_grok_document(&path) else {
        return ActiveAgentModel::Unknown;
    };
    let Some(alias) = document
        .get("models")
        .and_then(|models| models.get("default"))
        .and_then(|value| value.as_str())
    else {
        return ActiveAgentModel::Own;
    };
    let Some(live_model) = document
        .get("model")
        .and_then(|models| models.get(alias))
        .and_then(|entry| entry.get("model"))
        .and_then(|value| value.as_str())
    else {
        return ActiveAgentModel::Own;
    };
    let Some((provider_id, bound_model)) = grok_binding(storage, root, alias) else {
        return ActiveAgentModel::Own;
    };
    if bound_model != live_model {
        return ActiveAgentModel::Own;
    }
    ours(storage, &provider_id, live_model)
}

/// Build the `Ours` variant only while the saved pair still exists.
fn ours(storage: &Storage, provider_id: &str, model_id: &str) -> ActiveAgentModel {
    let Ok(provider) = crate::providers::get_provider(storage, provider_id) else {
        return ActiveAgentModel::Own;
    };
    if get_model_snapshot(
        storage,
        &GetModelSnapshotRequest {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
        },
    )
    .is_err()
    {
        return ActiveAgentModel::Own;
    }
    let model_label =
        saved_model_alias(storage, provider_id, model_id).unwrap_or_else(|| model_id.to_owned());
    ActiveAgentModel::Ours {
        provider_label: provider.display_name,
        model_id: model_id.to_owned(),
        model_label,
    }
}

/// A Claude Code Provider by its written base URL, or by a unique saved model.
fn claude_provider(storage: &Storage, model_id: &str, base_url: Option<&str>) -> Option<String> {
    match base_url {
        Some(base_url) => {
            let kind = super::claude_code::gateway_kind_for_base_url(base_url)?;
            provider_by_kind(storage, kind.as_str())
        }
        None => provider_by_model(storage, model_id),
    }
}

fn provider_by_kind(storage: &Storage, kind: &str) -> Option<String> {
    let connection = storage.lock().ok()?;
    let mut statement = connection
        .prepare("SELECT id FROM provider_instance WHERE kind = ?1")
        .ok()?;
    let ids: Vec<String> = statement
        .query_map([kind], |row| row.get(0))
        .ok()?
        .filter_map(Result::ok)
        .collect();
    (ids.len() == 1).then(|| ids[0].clone())
}

fn provider_by_model(storage: &Storage, model_id: &str) -> Option<String> {
    let connection = storage.lock().ok()?;
    let mut statement = connection
        .prepare("SELECT DISTINCT provider_id FROM provider_model WHERE model_id = ?1")
        .ok()?;
    let ids: Vec<String> = statement
        .query_map([model_id], |row| row.get(0))
        .ok()?
        .filter_map(Result::ok)
        .collect();
    (ids.len() == 1).then(|| ids[0].clone())
}

fn pi_binding(storage: &Storage, root: &str, provider_key: &str) -> Option<String> {
    let connection = storage.lock().ok()?;
    connection
        .query_row(
            "SELECT provider_id FROM pi_provider_binding
             WHERE config_root = ?1 AND provider_key = ?2",
            [root, provider_key],
            |row| row.get(0),
        )
        .ok()
}

fn grok_binding(storage: &Storage, root: &str, alias: &str) -> Option<(String, String)> {
    let connection = storage.lock().ok()?;
    connection
        .query_row(
            "SELECT provider_id, model_id FROM grok_model_binding
             WHERE config_root = ?1 AND model_alias = ?2",
            [root, alias],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok()
}

/// Parse a live Grok TOML file; an empty file is an empty document.
fn read_grok_document(path: &Path) -> Option<DocumentMut> {
    let bytes = crate::config_plan::read_target_bytes(path).ok()?.1;
    let text = std::str::from_utf8(&bytes).ok()?;
    if text.chars().all(char::is_whitespace) {
        Some(DocumentMut::new())
    } else {
        text.parse::<DocumentMut>().ok()
    }
}

/// Prefer the user's saved model alias when that row still exists.
fn saved_model_alias(storage: &Storage, provider_id: &str, model_id: &str) -> Option<String> {
    let snapshot = get_model_snapshot(
        storage,
        &GetModelSnapshotRequest {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
        },
    )
    .ok()?;
    let alias = snapshot.model.alias.as_deref()?;
    validate_display_name(alias, 64).ok()
}
