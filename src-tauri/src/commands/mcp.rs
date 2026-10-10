//! Central MCP metadata and credential IPC commands; no server is executed or contacted.

use super::state::{CredentialLock, with_mcp_storage};
use crate::credentials::OsCredentialStore;
use crate::providers;
use tauri::Manager;

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
            let now = providers::current_unix_millis()
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
