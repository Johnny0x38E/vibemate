//! Coordinate MCP metadata commits, fresh credential writes, and retryable cleanup.

use super::repository::{
    SaveSnapshot, acknowledge_cleanup, cleanup_complete, commit_definition, get_definition,
    pending_cleanup_references, prepare_definition_save,
};
use super::types::*;
use super::validation::{identity, validate};
use crate::credentials::{CredentialError, CredentialStore};
use crate::storage::Storage;
use std::collections::BTreeMap;

/// Delete only recorded obsolete references, retryably. Current references are
/// checked before every deletion. Call under the app's credential mutation lock.
/// Returns whether all recorded deletions for this definition finished.
///
/// # Errors
/// Reports invalid identity and storage/read/write failures. Failed OS deletion
/// remains queued and yields `false`, so the caller can offer a retry.
pub fn cleanup_credentials(
    storage: &Storage,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<bool, McpError> {
    identity(id)?;
    let refs = pending_cleanup_references(storage, id)?;
    for reference in refs {
        // Only references from this module's private namespace may reach the store.
        if !reference.starts_with(&format!("mcp-{id}-"))
            || reference.len() != 69
            || identity(&reference[37..]).is_err()
        {
            return Err(McpError::InvalidStoredDefinition);
        }
        if store.delete(&reference).is_ok() {
            acknowledge_cleanup(storage, reference)?;
        }
    }
    cleanup_complete(storage, id)
}

/// Save metadata and literal values, never starting the described server.
///
/// New values use fresh references. A known failed DB transaction deletes only
/// those new entries; uncertain commits retain them so a saved row stays usable.
/// Old entries are queued for retryable cleanup in the same metadata transaction.
/// A process crash between OS and DB writes may leave an orphan; no cross-store
/// transaction is claimed. Call under the app's credential mutation lock.
/// `now_ms` is a nonnegative Unix-millisecond timestamp used for safe metadata.
///
/// # Errors
/// Rejects unsupported or invalid fields before touching credentials. Reports
/// conflicts, unavailable credential storage and known database failures safely.
/// `OutcomeUnknown` requires reloading before another mutation.
pub fn save_definition(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: SaveMcpRequest,
    now_ms: i64,
) -> Result<McpRecord, McpError> {
    let (existing_id, revision, enabled, valid) = validate(request)?;
    if now_ms < 0 {
        return Err(McpError::InvalidRequest);
    }
    let SaveSnapshot {
        id,
        old_refs,
        new_ids,
    } = prepare_definition_save(storage, existing_id.as_deref(), revision, &valid)?;
    let mut next_refs = BTreeMap::new();
    let mut writes = Vec::new();
    for (field, new_id) in valid.fields.iter().zip(new_ids) {
        let key = (valid.kind.to_string(), field.name.clone());
        let reference = if let Some(random) = new_id {
            format!("mcp-{id}-{random}")
        } else {
            old_refs
                .get(&key)
                .cloned()
                .ok_or(McpError::SecretRequired)?
        };
        if let Some(value) = &field.value {
            writes.push((reference.clone(), value));
        }
        next_refs.insert(key, reference);
    }
    let mut written = Vec::new();
    for (reference, value) in writes {
        if let Err(error) = store.save(&reference, value) {
            // Backend initialization/access failures cannot have created this
            // fresh entry. Other platform failures may have an uncertain effect,
            // so include their reference in compensation as well.
            if !matches!(
                error,
                CredentialError::Unavailable | CredentialError::AccessDenied
            ) {
                written.push(reference);
            }
            return Err(if compensate(store, &written) {
                error.into()
            } else {
                McpError::OutcomeUnknown
            });
        }
        written.push(reference);
    }
    let commit = commit_definition(storage, &id, revision, enabled, &valid, &next_refs, now_ms);
    match commit {
        Ok(()) => {
            // Cleanup failure does not negate a confirmed metadata save. The UI
            // receives cleanupPending and a dedicated retry action.
            let _ = cleanup_credentials(storage, store, &id);
            get_definition(storage, &id).map_err(|_| McpError::OutcomeUnknown)
        }
        Err((error, uncertain)) => Err(if uncertain || !compensate(store, &written) {
            McpError::OutcomeUnknown
        } else {
            error
        }),
    }
}

fn compensate(store: &dyn CredentialStore, refs: &[String]) -> bool {
    let mut clean = true;
    for reference in refs {
        if store.delete(reference).is_err() {
            clean = false;
        }
    }
    clean
}
