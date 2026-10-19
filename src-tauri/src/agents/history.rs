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

/// One enriched history page, including backup availability and the current pin.
/// Still carries no backup paths or file bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationHistoryPage {
    /// Live configuration pin; identical for every page of the same scope.
    pub current: HistoryCurrentConfig,
    /// Newest first; every multi-file batch stays inside one entry.
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

/// Read retained apply/restore/compensation entries in the adapter's active root.
///
/// The environment is a trusted desktop snapshot, never frontend input. Resolves
/// the root without launching/validating an installed CLI, then queries only the
/// exact files owned by that adapter. Opens private backup files only to report
/// availability; never returns paths or bytes, and does not authorize recovery.
///
/// When `current.status` is `matched`, also attaches the matched Provider's
/// user-custom display name and the model alias-or-id read from the live config
/// files (and the saved model row when an alias still exists). Unmatched,
/// partial, and unreadable pins leave those fields null.
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
    let page = config_apply::configuration_history(
        storage,
        &scope,
        request.after.as_ref(),
        request.limit,
        backups,
    )?;
    let entries = page
        .entries
        .into_iter()
        .map(|entry| {
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
        })
        .collect::<Result<Vec<_>, ClaudeCodeRestoreError>>()?;
    let current = enrich_current(storage, request.agent, page.current, &entries);
    Ok(ConfigurationHistoryPage {
        current,
        entries,
        next_cursor: page.next_cursor,
    })
}

/// Attach provider/model display fields only for a fully matched live pin.
fn enrich_current(
    storage: &Storage,
    agent: ConfigurationAgent,
    mut current: HistoryCurrentConfig,
    entries: &[ConfigurationHistoryEntry],
) -> HistoryCurrentConfig {
    if current.status != HistoryCurrentStatus::Matched {
        return current;
    }
    let Some(matched_key) = current.matched_entry_key.as_deref() else {
        return current;
    };
    let matched = entries
        .iter()
        .find(|entry| entry.entry.entry_key == matched_key);
    // Provider label is the saved custom display name; never invent one from the
    // raw owner_ref when the Provider row is gone.
    current.provider_label = matched.and_then(|entry| entry.owner_label.clone());
    let owner_ref = matched.and_then(|entry| entry.entry.owner_ref.as_deref());
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
        ConfigurationAgent::GrokBuild => {
            let path = owned_path(files, "config.toml")?;
            grok_default_model_id(&path)
        }
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

/// Grok's default alias table carries `model = "<id>"` for the written entry.
fn grok_default_model_id(path: &Path) -> Option<String> {
    let bytes = crate::config_plan::read_target_bytes(path).ok()?.1;
    let text = std::str::from_utf8(&bytes).ok()?;
    let document = if text.chars().all(char::is_whitespace) {
        DocumentMut::new()
    } else {
        text.parse::<DocumentMut>().ok()?
    };
    let alias = document
        .get("models")?
        .get("default")?
        .as_str()
        .filter(|value| !value.is_empty())?;
    let model_id = document
        .get("model")?
        .get(alias)?
        .get("model")?
        .as_str()
        .filter(|value| is_valid_model_id(value))?;
    Some(model_id.to_owned())
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
