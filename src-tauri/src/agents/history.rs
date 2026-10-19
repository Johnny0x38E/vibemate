//! Read-only history IPC domain boundary for the three verified write adapters.
//!
//! Root selection shares discovery's environment precedence, not installation
//! verification. Removed installations and missing roots must not hide journals.
//! No frontend path, file contents, backup paths or recovery approval cross here.

use serde::{Deserialize, Serialize};

use super::{
    AgentId, ClaudeCodeRestoreError, DiscoveryEnvironment, discovery, recovery::ConfigurationAgent,
};
use crate::{
    config_apply::{self, HistoryCursor, HistoryEntry, HistoryScope},
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

/// One enriched history page. It still carries no backup paths or file bytes.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationHistoryPage {
    /// Newest first; every multi-file batch stays inside one entry.
    pub entries: Vec<ConfigurationHistoryEntry>,
    /// Seek position for more entries, absent when this read reached the end.
    pub next_cursor: Option<HistoryCursor>,
}

/// Read retained apply/restore/compensation entries in the adapter's active root.
///
/// The environment is a trusted desktop snapshot, never frontend input. Resolves
/// the root without launching/validating an installed CLI, then queries only the
/// exact files owned by that adapter. Reads journal and Provider metadata only;
/// does not open target/backup files, infer availability/current state or
/// authorize recovery.
///
/// # Errors
/// Returns safe request errors for invalid pagination, invalid/wrong-root cursors
/// or an unusable environment root. Storage failures reveal no raw diagnostics.
pub fn configuration_history_page(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: &ConfigurationHistoryRequest,
) -> Result<ConfigurationHistoryPage, ClaudeCodeRestoreError> {
    let (agent, files): (AgentId, &[&str]) = match request.agent {
        ConfigurationAgent::ClaudeCode => (AgentId::ClaudeCode, &["settings.json"]),
        ConfigurationAgent::Pi => (AgentId::Pi, &["models.json", "settings.json"]),
        ConfigurationAgent::GrokBuild => (AgentId::GrokBuild, &["config.toml"]),
    };
    let (root, _) = discovery::configuration_directory(agent, environment);
    // An invalid explicit override must not silently expose the default root.
    let root = root.ok_or(ClaudeCodeRestoreError::InvalidRequest)?;
    let scope = HistoryScope::new(request.agent.kind(), &root, files)?;
    let page = config_apply::configuration_history(
        storage,
        &scope,
        request.after.as_ref(),
        request.limit,
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
    Ok(ConfigurationHistoryPage {
        entries,
        next_cursor: page.next_cursor,
    })
}
