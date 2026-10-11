//! Fetch snapshot/status queries. This boundary owns its lock and error mapping;
//! it does not borrow private helpers from the local model repository.

use super::registry::ModelFetchRegistry;
use super::types::{LastModelFetch, ModelFetchStatus, ProviderSnapshot};
use crate::model_error::{ModelError, parse_provider_id};
use crate::providers::{ProviderId, ProviderKind, ProviderProtocol};
use crate::storage::Storage;
use rusqlite::OptionalExtension;

/// Lock fetch storage, preserving the existing safe startup/poison error code.
pub(super) fn lock(
    storage: &Storage,
) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, ModelError> {
    storage.lock().map_err(|_| ModelError::StorageUnavailable)
}

/// Read the provider snapshot and credential reference, releasing the lock on return.
/// The caller may then prompt through the OS credential store without holding SQLite.
pub(super) fn read_provider_snapshot(
    storage: &Storage,
    provider_id: &ProviderId,
) -> Result<(ProviderSnapshot, Option<String>), ModelError> {
    let connection = lock(storage)?;
    let row: Option<(String, String, String, i64, Option<String>)> = connection
        .query_row(
            "SELECT p.kind, p.protocol, p.base_url, p.revision, c.credential_ref
                 FROM provider_instance p
                 LEFT JOIN provider_credential c ON c.provider_id = p.id
                 WHERE p.id = ?1",
            [provider_id.as_str()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    let (kind, protocol, base_url, revision, reference) = row.ok_or(ModelError::NotFound)?;
    let snapshot = ProviderSnapshot {
        provider_id: provider_id.clone(),
        kind: ProviderKind::from_value(&kind).ok_or(ModelError::InvalidStoredProvider)?,
        protocol: ProviderProtocol::from_value(&protocol)
            .ok_or(ModelError::InvalidStoredProvider)?,
        base_url,
        revision,
    };
    Ok((snapshot, reference))
    // The connection guard is dropped here, before the credential store runs.
}

/// Report whether a fetch is running and the last saved fetch.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `NotFound`, or a storage code.
pub fn fetch_status(
    storage: &Storage,
    registry: &ModelFetchRegistry,
    provider_id: &str,
) -> Result<ModelFetchStatus, ModelError> {
    let provider_id = parse_provider_id(provider_id)?;
    let connection = lock(storage)?;
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM provider_instance WHERE id = ?1",
            [provider_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    exists.ok_or(ModelError::NotFound)?;
    let last_fetch = connection
        .query_row(
            "SELECT fetched_at, complete, listed_count FROM provider_model_fetch
             WHERE provider_id = ?1",
            [provider_id.as_str()],
            |row| {
                Ok(LastModelFetch {
                    fetched_at_ms: row.get(0)?,
                    complete: row.get::<_, i64>(1)? == 1,
                    listed_count: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|_| ModelError::ReadFailed)?;
    Ok(ModelFetchStatus {
        running: registry.is_running(&provider_id),
        last_fetch,
    })
}
