//! Read-only history IPC domain boundary for the three verified write adapters.
//!
//! Root selection shares discovery's environment precedence, not installation
//! verification. Removed installations and missing roots must not hide journals.
//! No frontend path, file contents, backup paths or recovery approval cross here.

use serde::Deserialize;

use super::{
    AgentId, ClaudeCodeRestoreError, DiscoveryEnvironment, discovery, recovery::ConfigurationAgent,
};
use crate::{
    config_apply::{self, HistoryCursor, HistoryPage, HistoryScope},
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

/// Read retained apply/restore/compensation entries in the adapter's active root.
///
/// The environment is a trusted desktop snapshot, never frontend input. Resolves
/// the root without launching/validating an installed CLI, then queries only the
/// exact files owned by that adapter. Reads journal metadata only; does not open
/// target/backup files, infer availability/current state or authorize recovery.
///
/// # Errors
/// Returns safe request errors for invalid pagination, invalid/wrong-root cursors
/// or an unusable environment root. Storage failures reveal no raw diagnostics.
pub fn configuration_history_page(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: &ConfigurationHistoryRequest,
) -> Result<HistoryPage, ClaudeCodeRestoreError> {
    let (agent, files): (AgentId, &[&str]) = match request.agent {
        ConfigurationAgent::ClaudeCode => (AgentId::ClaudeCode, &["settings.json"]),
        ConfigurationAgent::Pi => (AgentId::Pi, &["models.json", "settings.json"]),
        ConfigurationAgent::GrokBuild => (AgentId::GrokBuild, &["config.toml"]),
    };
    let (root, _) = discovery::configuration_directory(agent, environment);
    // An invalid explicit override must not silently expose the default root.
    let root = root.ok_or(ClaudeCodeRestoreError::InvalidRequest)?;
    let scope = HistoryScope::new(request.agent.kind(), &root, files)?;
    Ok(config_apply::configuration_history(
        storage,
        &scope,
        request.after.as_ref(),
        request.limit,
    )?)
}
