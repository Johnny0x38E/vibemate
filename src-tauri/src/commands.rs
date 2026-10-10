//! Desktop IPC composition. Each feature owns its command module; managed-state
//! coordination is shared here, while domain behavior remains independent of Tauri.

pub(crate) mod app;
pub(crate) mod models;
pub(crate) mod providers;
mod state;

pub(crate) use state::CredentialLock;

use crate::appearance::{self, AppearancePreference};
use crate::credentials::OsCredentialStore;
use crate::settings::{self, LocalePreference, SettingsError};
use crate::storage::StorageStatus;
use state::with_mcp_storage;
use tauri::Manager;

/// Read the saved language choice without writing defaults. Returns safe error
/// codes if startup storage or the read fails; never exposes SQLite diagnostics.
#[tauri::command]
pub(crate) async fn get_locale_preference(
    app: tauri::AppHandle,
) -> Result<LocalePreference, SettingsError> {
    // SQLite can wait for another process's lock. Keep that blocking wait off both
    // the window thread and async executor; the owned handle lives until it ends.
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::load_locale_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("get_locale_preference", result)
}

/// Save a validated language choice, returning it after the single-row commit.
/// Storage/write failures are safe codes; a task failure has an unknown outcome
/// and callers should reload before claiming which preference is persisted.
#[tauri::command]
pub(crate) async fn save_locale_preference(
    app: tauri::AppHandle,
    preference: LocalePreference,
) -> Result<LocalePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        settings::save_locale_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("save_locale_preference", result)
}

/// Read the validated appearance/theme pair without writing a default row.
/// Database work uses the blocking pool; only safe settings error codes escape.
#[tauri::command]
pub(crate) async fn get_appearance_preference(
    app: tauri::AppHandle,
) -> Result<AppearancePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        appearance::load_appearance_preference(storage)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("get_appearance_preference", result)
}

/// Save a typed pair atomically, returning the committed choice. A task failure
/// has an unknown outcome and requires a read before another save is attempted.
#[tauri::command]
pub(crate) async fn save_appearance_preference(
    app: tauri::AppHandle,
    preference: AppearancePreference,
) -> Result<AppearancePreference, SettingsError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let status = app
            .try_state::<StorageStatus>()
            .ok_or(SettingsError::StorageUnavailable)?;
        let storage = status
            .storage()
            .map_err(|_| SettingsError::StorageUnavailable)?;
        appearance::save_appearance_preference(storage, preference)
    })
    .await
    .map_err(|_| SettingsError::OperationFailed)
    .and_then(|result| result);
    crate::logging::settings_outcome("save_appearance_preference", result)
}

/// Read central MCP metadata, without executing servers or opening credentials.
#[tauri::command]
pub(crate) async fn list_mcp_definitions(
    app: tauri::AppHandle,
    after: Option<String>,
    limit: u32,
) -> Result<crate::mcp::McpPage, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| {
            crate::mcp::list_definitions(storage, after.as_deref(), limit)
        })
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}

/// Read one non-secret central definition for editing or result reconciliation.
#[tauri::command]
pub(crate) async fn get_mcp_definition(
    app: tauri::AppHandle,
    id: String,
) -> Result<crate::mcp::McpRecord, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| crate::mcp::get_definition(storage, &id))
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}

/// Save only central metadata and OS credential references. No process or HTTP
/// API is involved; the credential lock excludes in-process secret mutations.
#[tauri::command]
pub(crate) async fn save_mcp_definition(
    app: tauri::AppHandle,
    request: crate::mcp::SaveMcpRequest,
) -> Result<crate::mcp::McpRecord, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| {
            let credential_lock = app
                .try_state::<CredentialLock>()
                .ok_or(crate::mcp::McpError::OperationFailed)?;
            let _guard = credential_lock
                .0
                .lock()
                .map_err(|_| crate::mcp::McpError::OperationFailed)?;
            let now = crate::providers::current_unix_millis()
                .map_err(|_| crate::mcp::McpError::OperationFailed)?;
            crate::mcp::save_definition(storage, &OsCredentialStore, request, now)
        })
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}

/// Retry deletion of recorded obsolete credential entries and return safe status.
#[tauri::command]
pub(crate) async fn cleanup_mcp_credentials(
    app: tauri::AppHandle,
    id: String,
) -> Result<crate::mcp::McpRecord, crate::mcp::McpError> {
    tauri::async_runtime::spawn_blocking(move || {
        with_mcp_storage(&app, |storage| {
            let credential_lock = app
                .try_state::<CredentialLock>()
                .ok_or(crate::mcp::McpError::OperationFailed)?;
            let _guard = credential_lock
                .0
                .lock()
                .map_err(|_| crate::mcp::McpError::OperationFailed)?;
            crate::mcp::get_definition(storage, &id)?;
            crate::mcp::cleanup_credentials(storage, &OsCredentialStore, &id)?;
            crate::mcp::get_definition(storage, &id)
        })
    })
    .await
    .map_err(|_| crate::mcp::McpError::OperationFailed)?
}
