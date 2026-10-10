//! Tauri managed-state lookups and credential coordination shared by commands.
//! Business modules never depend on this desktop-only boundary.

use crate::models::ModelError;
use crate::providers::ProviderError;
use crate::storage::{Storage, StorageStatus};
use std::sync::{Mutex, PoisonError};
use tauri::Manager;

/// Serializes this app's writes to the OS credential store.
///
/// A replacement reads the old key as a backup, writes the new one, and may put
/// the old one back. Two such sequences for the same provider must not interleave.
/// This is a separate lock from the database connection: the credential store may
/// wait for a system prompt, and database reads must not wait with it.
///
/// The mutex only excludes other threads of this process. It does not stop a
/// second vibemate process from writing the same entry at the same time; the
/// app does not prevent a second instance yet.
#[derive(Default)]
pub(crate) struct CredentialLock(pub(super) Mutex<()>);

/// Return Unix milliseconds in the existing model error namespace.
pub(super) fn model_timestamp() -> Result<i64, ModelError> {
    crate::shared::current_unix_millis().map_err(|_| ModelError::OperationFailed)
}

/// Resolve startup storage while keeping failures in the model error namespace.
pub(super) fn with_model_storage<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&Storage) -> Result<T, ModelError>,
) -> Result<T, ModelError> {
    let status = app
        .try_state::<StorageStatus>()
        .ok_or(ModelError::StorageUnavailable)?;
    let storage = status
        .storage()
        .map_err(|_| ModelError::StorageUnavailable)?;
    operation(storage)
}

/// Run `operation` while holding the app-wide `CredentialLock`.
///
/// The lock guards no data, only ordering, so a panic elsewhere ("poisoning")
/// leaves nothing inconsistent to protect and the lock is used as usual.
/// It gives mutual exclusion within this process only (see `CredentialLock`).
///
/// A missing lock means the app was wired up wrongly, not that storage failed,
/// so it is reported as the generic internal `OperationFailed`, before anything
/// was written.
pub(super) fn with_credential_writes<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce() -> Result<T, ProviderError>,
) -> Result<T, ProviderError> {
    let lock = app
        .try_state::<CredentialLock>()
        .ok_or(ProviderError::OperationFailed)?;
    let _credential_writes = lock.0.lock().unwrap_or_else(PoisonError::into_inner);
    operation()
}

/// Run one provider operation against the storage opened at startup.
///
/// Every provider command needs the same two lookups, and both failures mean
/// `StorageUnavailable`. Keeping them here leaves each command one line of logic.
pub(super) fn with_provider_storage<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&Storage) -> Result<T, ProviderError>,
) -> Result<T, ProviderError> {
    let status = app
        .try_state::<StorageStatus>()
        .ok_or(ProviderError::StorageUnavailable)?;
    let storage = status
        .storage()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    operation(storage)
}

/// Resolve startup storage while preserving safe MCP error codes.
pub(super) fn with_mcp_storage<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&Storage) -> Result<T, crate::mcp::McpError>,
) -> Result<T, crate::mcp::McpError> {
    let status = app
        .try_state::<StorageStatus>()
        .ok_or(crate::mcp::McpError::StorageUnavailable)?;
    let storage = status
        .storage()
        .map_err(|_| crate::mcp::McpError::StorageUnavailable)?;
    operation(storage)
}
