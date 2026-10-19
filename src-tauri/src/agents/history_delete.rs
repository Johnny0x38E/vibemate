//! Strict IPC boundary for deleting one configuration history entry (H3).
//!
//! The frontend names one Agent and one history entry; Rust selects the active
//! configuration root and owned files from the trusted desktop environment. No
//! path, backup location or credential reference is accepted from the caller.
//! The domain revalidates protections, shared backup refs and durable delete
//! state; this module only bounds the payload and maps safe errors.

use serde::Deserialize;

use super::{
    ClaudeCodeRestoreError, DiscoveryEnvironment, history::history_scope,
    recovery::ConfigurationAgent,
};
use crate::{
    config_apply::{self, BackupStore, HistoryDeleteOutcome},
    storage::Storage,
};

/// Request deletion of one history entry and its exclusive private backups.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteHistoryRequest {
    /// Adapter whose history the user is viewing.
    pub agent: ConfigurationAgent,
    /// `operation:<id>` or `batch:<id>` from the history page.
    pub entry_key: String,
}

/// Delete one history entry for the named Agent, or refuse with a stable code.
///
/// # Errors
/// Returns safe request, scope, protection and storage errors. A retryable
/// leftover outcome is success with `state = failed`, not an error.
pub fn delete_history_entry(
    storage: &Storage,
    environment: &DiscoveryEnvironment,
    request: &DeleteHistoryRequest,
    backups: &BackupStore,
) -> Result<HistoryDeleteOutcome, ClaudeCodeRestoreError> {
    if request.entry_key.is_empty() || request.entry_key.len() > 160 {
        return Err(ClaudeCodeRestoreError::InvalidRequest);
    }
    let scope = history_scope(request.agent, environment)?;
    Ok(config_apply::delete_history_entry(
        storage,
        &scope,
        &request.entry_key,
        backups,
    )?)
}
